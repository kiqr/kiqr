//! Publishing: channels (where content goes out), the approvals behind
//! `publish_asset`, `send_email` and `create_channel`, the user's own
//! approve / schedule / publish / unpublish on an asset, the scheduled
//! publishes the daemon does, and the built-in email sender. Every publish
//! asks the user first; this module does the sending once they said yes.

use crate::api::{ApiError, ApiResult};
use crate::App;
use anyhow::{anyhow, Context, Result};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use kiqr_core::assets::{self, Asset, Publication, Status};
use kiqr_core::channels::{self, Channel};
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{ChannelKind, Manifest, Transport};
use kiqr_core::project::Project;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/channels", get(list_channels).post(create_channel))
        .route("/api/projects/{slug}/channels/{id}", get(get_channel).put(put_channel).delete(delete_channel))
        .route("/api/projects/{slug}/assets/{kind}/{id}/publish", axum::routing::post(publish_route))
        .route("/api/projects/{slug}/assets/{kind}/{id}/approve", axum::routing::post(approve_route))
        .route("/api/projects/{slug}/assets/{kind}/{id}/schedule", axum::routing::post(schedule_route).delete(unschedule_route))
        .route("/api/projects/{slug}/assets/{kind}/{id}/unpublish", axum::routing::post(unpublish_route))
        .route("/api/projects/{slug}/email", axum::routing::post(email_route))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

// ---------------------------------------------------------------- channels

/// The channel kinds a project can use right now: ready connections that publish.
async fn kinds(app: &App, slug: &str) -> Vec<(Manifest, ChannelKind)> {
    let reg = app.registry.read().await;
    let off = crate::ext::project_off(app, slug);
    let mut out = Vec::new();
    for (_, m) in reg.connections(slug) {
        if off.contains(&m.id) {
            continue;
        }
        for k in &m.channels {
            out.push((m.clone(), k.clone()));
        }
    }
    out
}

/// A kind a new channel could use, with what the project still has to give
/// it (a mail account is set up inside the project, when the channel is made).
fn kind_view(app: &App, slug: &str, m: &Manifest, k: &ChannelKind) -> Value {
    let home = app.store.home();
    json!({
        "kind": k.kind,
        "label": k.label,
        "connection": m.id,
        "connection_name": m.name,
        "ready": crate::ext::ready_in(app, Some(slug), m),
        "destination": k.destination,
        "auth": m.auth.iter().map(|a| json!({ "env": a.env, "label": a.label, "help": a.help, "optional": a.optional, "set": kiqr_core::secrets::scope_of(home, Some(slug), &a.env) })).collect::<Vec<_>>(),
    })
}

async fn channel_view(app: &App, slug: &str, c: &Channel) -> Value {
    let (name, ready) = {
        let reg = app.registry.read().await;
        reg.connection(slug, &c.extension).map(|(_, m)| (m.name.clone(), crate::ext::ready_in(app, Some(slug), m))).unwrap_or_else(|| (c.extension.clone(), false))
    };
    let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
    v["connection_name"] = json!(name);
    v["ready"] = json!(ready);
    v["destination_text"] = json!(c.destination_text());
    v
}

/// Channels plus the kinds a new one could use.
async fn list_channels(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let mut list = Vec::new();
    for c in Channel::list(&app.store, &slug)? {
        list.push(channel_view(&app, &slug, &c).await);
    }
    let kinds: Vec<Value> = kinds(&app, &slug).await.iter().map(|(m, k)| kind_view(&app, &slug, m, k)).collect();
    Ok(Json(json!({ "channels": list, "kinds": kinds })))
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct NewChannel {
    pub label: String,
    pub kind: String,
    /// The connection to use when several offer the kind.
    pub connection: Option<String>,
    pub destination: serde_json::Map<String, Value>,
}

/// Check a channel proposal against the kinds available; returns what will be saved.
pub async fn channel_preview(app: &App, slug: &str, b: &NewChannel) -> Result<Value, ApiError> {
    let kind = b.kind.trim().to_ascii_lowercase();
    if kind.is_empty() {
        return Err(bad("a channel needs a kind (email, or one a connection declares)"));
    }
    let all = kinds(app, slug).await;
    let matching: Vec<&(Manifest, ChannelKind)> = all.iter().filter(|(m, k)| k.kind == kind && b.connection.as_deref().map(|c| c == m.id).unwrap_or(true)).collect();
    let Some((m, k)) = matching.iter().find(|(m, _)| crate::ext::ready_in(app, Some(slug), m)).or_else(|| matching.first()).map(|x| (&x.0, &x.1)) else {
        let have: Vec<String> = all.iter().map(|(_, k)| k.kind.clone()).collect();
        return Err(bad(if have.is_empty() { "nothing can publish yet; install Email under Settings → Abilities".to_string() } else { format!("nothing publishes '{kind}'; available: {}", have.join(", ")) }));
    };
    let mut destination = serde_json::Map::new();
    for (field, spec) in &k.destination {
        let required = !spec["optional"].as_bool().unwrap_or(false);
        match b.destination.get(field).and_then(|v| if v.is_string() { v.as_str().map(|s| s.trim().to_string()) } else { Some(v.to_string()) }).filter(|s| !s.is_empty()) {
            Some(v) => {
                destination.insert(field.clone(), Value::String(v));
            }
            None if required => return Err(bad(format!("the destination needs '{}' ({})", field, spec["label"].as_str().unwrap_or(field)))),
            None => {}
        }
    }
    let label = b.label.trim().to_string();
    let label = if label.is_empty() {
        let d = destination.values().find_map(|v| v.as_str()).unwrap_or("");
        if d.is_empty() { k.label.clone() } else { format!("{} to {d}", k.label) }
    } else {
        label
    };
    Ok(json!({
        "label": label,
        "kind": kind,
        "kind_label": k.label,
        "connection": m.id,
        "connection_name": m.name,
        "ready": crate::ext::ready_in(app, Some(slug), m),
        "destination": destination,
    }))
}

/// Save a channel from a checked preview.
pub async fn save_channel(app: &App, slug: &str, preview: &Value, by: &str) -> Result<Value, ApiError> {
    let label = preview["label"].as_str().unwrap_or("Channel").to_string();
    let c = Channel {
        id: channels::new_id(&app.store, slug, &label),
        label,
        extension: preview["connection"].as_str().unwrap_or("").to_string(),
        kind: preview["kind"].as_str().unwrap_or("").to_string(),
        destination: preview["destination"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default(),
        created_by: by.to_string(),
        created_at: Utc::now(),
        asks_first: true,
        publications: 0,
        last_published: None,
    };
    c.save(&app.store, slug)?;
    app.store.commit(&format!("channel({slug}): add {}", c.label)).await?;
    app.bus.emit(Some(slug), EventKind::ChannelSaved { channel: c.id.clone(), label: c.label.clone(), kind: c.kind.clone() });
    let mut v = channel_view(app, slug, &c).await;
    v["note"] = json!("The channel exists. Publishing to it still asks the user each time.");
    Ok(v)
}

async fn create_channel(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<NewChannel>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let _ = Project::load(&app.store, &slug)?;
    let preview = channel_preview(&app, &slug, &b).await?;
    let v = save_channel(&app, &slug, &preview, "user").await?;
    Ok((StatusCode::CREATED, Json(v)))
}

async fn get_channel(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let c = Channel::load(&app.store, &slug, &id)?;
    Ok(Json(channel_view(&app, &slug, &c).await))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ChannelPatch {
    label: Option<String>,
    destination: Option<serde_json::Map<String, Value>>,
}

async fn put_channel(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(p): Json<ChannelPatch>) -> ApiResult<Value> {
    let mut c = Channel::load(&app.store, &slug, &id)?;
    if let Some(l) = p.label.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        c.label = l;
    }
    if let Some(d) = p.destination {
        let checked = channel_preview(&app, &slug, &NewChannel { label: c.label.clone(), kind: c.kind.clone(), connection: Some(c.extension.clone()), destination: d }).await?;
        c.destination = checked["destination"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    }
    c.save(&app.store, &slug)?;
    app.store.commit(&format!("channel({slug}): {}", c.label)).await?;
    app.bus.emit(Some(&slug), EventKind::ChannelSaved { channel: c.id.clone(), label: c.label.clone(), kind: c.kind.clone() });
    Ok(Json(channel_view(&app, &slug, &c).await))
}

async fn delete_channel(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> Result<StatusCode, ApiError> {
    let c = Channel::load(&app.store, &slug, &id)?;
    Channel::delete(&app.store, &slug, &id)?;
    app.store.commit(&format!("channel({slug}): remove {}", c.label)).await?;
    app.bus.emit(Some(&slug), EventKind::ChannelDeleted { channel: id });
    Ok(StatusCode::NO_CONTENT)
}

// -------------------------------------------------------------- publishing

/// An asset's body as Markdown and its media files, for sending.
fn asset_parts(app: &App, slug: &str, a: &Asset) -> Result<(String, Vec<(String, Vec<u8>, String)>)> {
    let dir = app.store.home().assets_dir(slug).join(&a.kind).join(&a.id);
    let body = assets::body_file(a).map(|f| std::fs::read_to_string(dir.join(f)).unwrap_or_default()).unwrap_or_default();
    let mut files = Vec::new();
    for f in a.files.iter().filter(|f| f.role != "body") {
        if let Ok(bytes) = std::fs::read(dir.join(&f.path)) {
            if bytes.len() <= 20 * 1024 * 1024 {
                let mime = mime_guess::from_path(&f.path).first_or_octet_stream().to_string();
                files.push((f.path.clone(), bytes, mime));
            }
        }
    }
    Ok((body, files))
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PublishBody {
    pub asset: AssetRef,
    /// Empty means the channel the asset was approved for.
    pub channel: String,
    /// Set, the asset is scheduled for then instead of published now.
    pub at: Option<DateTime<Utc>>,
    pub note: String,
    pub by: Option<String>,
}

/// A channel of the project that can publish right now, by id. The error
/// says what exists when the id is unknown.
async fn ready_channel(app: &App, slug: &str, id: &str) -> Result<Channel, ApiError> {
    let c = Channel::load(&app.store, slug, id.trim()).map_err(|_| {
        let have: Vec<String> = Channel::list(&app.store, slug).unwrap_or_default().into_iter().map(|c| format!("{} ({})", c.id, c.label)).collect();
        bad(if have.is_empty() { "there is no channel yet; create one first (create_channel) or ask the user to add one under Seemo → Channels".to_string() } else { format!("no channel '{}'; channels: {}", id.trim(), have.join(", ")) })
    })?;
    let cv = channel_view(app, slug, &c).await;
    if cv["ready"].as_bool() != Some(true) {
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("the ability behind the channel ({}) is not set up for this project; add a channel of that kind under Channels to set the account up", cv["connection_name"].as_str().unwrap_or(""))));
    }
    Ok(c)
}

/// The channel a request means: the one named, else the asset's own.
fn channel_of<'a>(a: &'a Asset, named: &'a str) -> Result<&'a str, ApiError> {
    let named = named.trim();
    if !named.is_empty() {
        return Ok(named);
    }
    a.channel.as_deref().filter(|c| !c.is_empty()).ok_or_else(|| bad("name a channel; the asset is not approved for one yet"))
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct AssetRef {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
}

/// What the card shows before a yes: the asset, where it goes, the first lines.
pub async fn publish_preview(app: &App, slug: &str, b: &PublishBody) -> Result<Value, ApiError> {
    let a = assets::load(&app.store, slug, &b.asset.kind, &b.asset.id)?;
    if a.archived {
        return Err(bad("it is archived; bring it back first"));
    }
    let c = ready_channel(app, slug, channel_of(&a, &b.channel)?).await?;
    let cv = channel_view(app, slug, &c).await;
    if b.at.is_some_and(|t| t <= Utc::now()) {
        return Err(bad("that time has passed; leave `at` out to publish now"));
    }
    let (body, files) = asset_parts(app, slug, &a).map_err(|e| bad(e.to_string()))?;
    let subject = c.destination.get("subject").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()).map(str::to_string).unwrap_or_else(|| a.title.clone());
    Ok(json!({
        "asset": { "type": a.kind, "id": a.id, "title": a.title, "status": a.status },
        "channel": c.id,
        "channel_label": c.label,
        "kind": c.kind,
        "destination": c.destination,
        "destination_text": c.destination_text(),
        "connection_name": cv["connection_name"],
        "subject": subject,
        "excerpt": kiqr_core::profile::truncate_chars(body.trim(), 400),
        "words": body.split_whitespace().count(),
        "files": files.iter().map(|(n, b, m)| json!({ "name": n, "bytes": b.len(), "mime": m })).collect::<Vec<_>>(),
        "at": b.at,
        "note": b.note,
    }))
}

/// Send the asset through the channel and record it, or, when the preview
/// carries `at`, approve it for the channel and leave it to the scheduler.
/// After a yes.
pub async fn do_publish(app: &App, slug: &str, preview: &Value, by: &str) -> Result<Value, ApiError> {
    let kind = preview["asset"]["type"].as_str().unwrap_or("").to_string();
    let id = preview["asset"]["id"].as_str().unwrap_or("").to_string();
    let mut a = assets::load(&app.store, slug, &kind, &id)?;
    let mut c = Channel::load(&app.store, slug, preview["channel"].as_str().unwrap_or(""))?;
    if let Some(at) = preview["at"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Utc)) {
        let a = set_schedule(app, slug, a, &c, Some(at)).await?;
        return Ok(json!({
            "scheduled": true,
            "asset": { "type": a.kind, "id": a.id, "title": a.title, "status": a.status },
            "channel": c.id,
            "channel_label": c.label,
            "at": at,
            "note": "Scheduled. The daemon publishes it at that time; the asset is marked scheduled until then.",
        }));
    }
    let (body, files) = asset_parts(app, slug, &a).map_err(|e| bad(e.to_string()))?;
    let subject = preview["subject"].as_str().unwrap_or(&a.title).to_string();
    let m = {
        let reg = app.registry.read().await;
        reg.connection(slug, &c.extension).map(|(_, m)| m.clone()).ok_or_else(|| bad(format!("the connection '{}' is gone", c.extension)))?
    };
    let k = m.channels.iter().find(|k| k.kind == c.kind).cloned().ok_or_else(|| bad(format!("{} no longer publishes '{}'", m.name, c.kind)))?;
    let (url, account, detail) = if m.transport == Transport::Builtin && m.id == "kiqr-smtp" {
        let to = c.destination.get("to").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let r = smtp_send(&kiqr_core::secrets::env_fn(app.store.home(), Some(slug)), &to, &subject, &body, files).await.map_err(|e| bad(format!("{e:#}")))?;
        (None, Some(r.from), format!("sent to {to}"))
    } else {
        // The connection's publish tool, with the asset and the destination.
        let provide = m.tools.iter().find(|t| t.name == k.publish).map(|t| t.provide.clone()).or_else(|| m.provides.get(&k.publish).cloned()).ok_or_else(|| bad(format!("{} has no '{}'", m.name, k.publish)))?;
        let mut args = json!({
            "title": a.title,
            "text": body,
            "html": channels::markdown_to_html(&body),
            "summary": a.summary,
            "tags": a.tags,
            "asset": { "type": a.kind, "id": a.id },
            "note": preview["note"],
        });
        for (dk, dv) in &c.destination {
            args[dk] = dv.clone();
        }
        let out = crate::ext::execute(app, slug, &m, &provide, None, Some(&k.publish), &args).await.map_err(|e| bad(format!("{e:#}")))?;
        let url = out.text.clone().filter(|t| t.starts_with("http")).or_else(|| out.json.get("url").and_then(|u| u.as_str()).map(str::to_string)).or_else(|| out.json.get("link").and_then(|u| u.as_str()).map(str::to_string));
        let detail = out.text.clone().unwrap_or_else(|| kiqr_core::profile::truncate_chars(&out.json.to_string(), 200));
        (url, None, detail)
    };
    let now = Utc::now();
    a.publications.push(Publication { connector: c.id.clone(), account, url: url.clone(), at: now, run: if by.starts_with("task:") { Some(by.to_string()) } else { None } });
    a.channel = Some(c.id.clone());
    a.publish_at = None;
    a.publish_error = None;
    let a = assets::save(&app.store, slug, &a)?;
    crate::api::emit_saved(app, slug, &a, false);
    c.publications += 1;
    c.last_published = Some(now);
    let _ = c.save(&app.store, slug);
    app.store.commit(&format!("[auto] publish({slug}): {} → {}", a.title, c.label)).await?;
    app.bus.emit(Some(slug), EventKind::AssetPublished { asset_type: a.kind.clone(), id: a.id.clone(), channel: c.id.clone(), url: url.clone() });
    Ok(json!({
        "published": true,
        "asset": { "type": a.kind, "id": a.id, "title": a.title },
        "channel": c.id,
        "channel_label": c.label,
        "url": url,
        "detail": detail,
        "note": "Published. The asset is marked published and the channel's count went up.",
    }))
}

/// `POST /api/projects/{slug}/assets/{kind}/{id}/publish`: from the UI or CLI; the click is the yes.
async fn publish_route(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>, Json(b): Json<PublishBody>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let body = PublishBody { asset: AssetRef { kind, id }, ..b };
    let preview = publish_preview(&app, &slug, &body).await?;
    let by = body.by.clone().unwrap_or_else(|| "user".into());
    Ok(Json(do_publish(&app, &slug, &preview, &by).await?))
}

// ------------------------------------------- approve, schedule, unpublish
//
// Status is never set by hand. These change the facts it follows from: the
// channel the user approved the asset for, the time the daemon publishes it,
// and whether KIQR still counts it as published.

#[derive(Deserialize, Default)]
#[serde(default)]
struct ApproveBody {
    channel: String,
}

#[derive(Deserialize)]
struct ScheduleBody {
    at: DateTime<Utc>,
    #[serde(default)]
    channel: String,
}

/// Approve the asset for a channel, with or without a time. Ready, or Scheduled.
async fn set_schedule(app: &App, slug: &str, mut a: Asset, c: &Channel, at: Option<DateTime<Utc>>) -> Result<Asset, ApiError> {
    if a.archived {
        return Err(bad("it is archived; bring it back first"));
    }
    if !a.publications.is_empty() {
        return Err(bad("it is published already; unpublish it first"));
    }
    a.channel = Some(c.id.clone());
    a.publish_at = at;
    a.publish_error = None;
    let a = assets::save(&app.store, slug, &a)?;
    crate::api::emit_saved(app, slug, &a, false);
    app.bus.emit(Some(slug), EventKind::AssetApproved { asset_type: a.kind.clone(), id: a.id.clone(), title: a.title.clone(), channel: c.id.clone(), at });
    Ok(a)
}

/// `POST …/approve {channel}`: the user's yes to this asset, for that channel. It is Ready.
async fn approve_route(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>, Json(b): Json<ApproveBody>) -> ApiResult<Asset> {
    let _ = Project::load(&app.store, &slug)?;
    let a = assets::load(&app.store, &slug, &kind, &id)?;
    let c = ready_channel(&app, &slug, channel_of(&a, &b.channel)?).await?;
    let at = a.publish_at;
    Ok(Json(set_schedule(&app, &slug, a, &c, at).await?))
}

/// `POST …/schedule {at, channel?}`: publish it then, through its channel or the one named. Scheduled.
async fn schedule_route(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>, Json(b): Json<ScheduleBody>) -> ApiResult<Asset> {
    let _ = Project::load(&app.store, &slug)?;
    let a = assets::load(&app.store, &slug, &kind, &id)?;
    if b.at <= Utc::now() {
        return Err(bad("that time has passed; publish it now instead"));
    }
    let c = ready_channel(&app, &slug, channel_of(&a, &b.channel)?).await?;
    Ok(Json(set_schedule(&app, &slug, a, &c, Some(b.at)).await?))
}

/// `DELETE …/schedule`: drop the time; it stays approved for the channel (Ready).
async fn unschedule_route(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>) -> ApiResult<Asset> {
    let _ = Project::load(&app.store, &slug)?;
    let mut a = assets::load(&app.store, &slug, &kind, &id)?;
    a.publish_at = None;
    a.publish_error = None;
    let a = assets::save(&app.store, &slug, &a)?;
    crate::api::emit_saved(&app, &slug, &a, false);
    Ok(Json(a))
}

/// `POST …/unpublish`: KIQR stops counting it as published. Nothing is taken
/// down: channels have no take-back, and the record says so. The asset goes
/// back to Ready for its channel.
async fn unpublish_route(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>) -> ApiResult<Asset> {
    let _ = Project::load(&app.store, &slug)?;
    let mut a = assets::load(&app.store, &slug, &kind, &id)?;
    if a.publications.is_empty() {
        return Err(bad("it is not published"));
    }
    // Older assets were published before a channel was kept on them: remember the one it went through.
    if a.channel.is_none() {
        a.channel = a.publications.last().map(|p| p.connector.clone());
    }
    a.publications.clear();
    let a = assets::save(&app.store, &slug, &a)?;
    crate::api::emit_saved(&app, &slug, &a, false);
    app.bus.emit(Some(&slug), EventKind::AssetUnpublished { asset_type: a.kind.clone(), id: a.id.clone(), title: a.title.clone() });
    Ok(Json(a))
}

/// Publish what is due: Scheduled assets whose time has come. The scheduler
/// calls this every tick. A failure stays on the asset (it falls back to
/// Ready, with the reason) and Seemo says so.
pub async fn publish_due(app: &App, slug: &str, now: DateTime<Utc>) {
    let Ok(list) = assets::list(&app.store, slug) else { return };
    for a in list.into_iter().filter(|a| a.status == Status::Scheduled && a.publish_at.is_some_and(|t| t <= now)) {
        let key = format!("publish:{slug}/{}/{}", a.kind, a.id);
        if !app.jobs.write().await.insert(key.clone()) {
            continue;
        }
        let body = PublishBody { asset: AssetRef { kind: a.kind.clone(), id: a.id.clone() }, channel: a.channel.clone().unwrap_or_default(), ..Default::default() };
        let r = match publish_preview(app, slug, &body).await {
            Ok(p) => do_publish(app, slug, &p, "schedule").await,
            Err(e) => Err(e),
        };
        if let Err(ApiError(_, msg)) = r {
            tracing::warn!(slug, asset = %format!("{}/{}", a.kind, a.id), error = %msg, "a scheduled publish failed");
            if let Ok(mut x) = assets::load(&app.store, slug, &a.kind, &a.id) {
                x.publish_at = None;
                x.publish_error = Some(msg.clone());
                if let Ok(x) = assets::save(&app.store, slug, &x) {
                    crate::api::emit_saved(app, slug, &x, false);
                }
            }
            app.bus.emit(Some(slug), EventKind::SeemoSays { text: format!("I could not publish \"{}\" as scheduled: {msg}", a.title) });
        }
        app.jobs.write().await.remove(&key);
    }
}

// ------------------------------------------------------------------- email

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct EmailBody {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub asset: Option<AssetRef>,
}

pub async fn email_preview(app: &App, slug: &str, b: &EmailBody) -> Result<Value, ApiError> {
    let to = b.to.trim();
    if to.is_empty() || !to.contains('@') {
        return Err(bad("`to` needs an email address"));
    }
    if b.subject.trim().is_empty() {
        return Err(bad("an email needs a subject"));
    }
    let provider = {
        let cfg = app.config.read().await.clone();
        let reg = app.registry.read().await;
        reg.provider(slug, "email", cfg.abilities.get("email").map(String::as_str), |m| crate::ext::ready_in(app, Some(slug), m)).map(|(_, m)| m.name.clone())
    };
    let Some(provider) = provider else {
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, "no mail ability is set up for this project; install Email under Settings → Abilities and add an email channel".into()));
    };
    let files: Vec<Value> = match &b.asset {
        Some(r) if !r.id.is_empty() => {
            let a = assets::load(&app.store, slug, &r.kind, &r.id)?;
            let (_, files) = asset_parts(app, slug, &a).map_err(|e| bad(e.to_string()))?;
            files.iter().map(|(n, bytes, m)| json!({ "name": n, "bytes": bytes.len(), "mime": m })).collect()
        }
        _ => vec![],
    };
    Ok(json!({
        "to": to,
        "subject": b.subject.trim(),
        "text": b.text.trim(),
        "excerpt": kiqr_core::profile::truncate_chars(b.text.trim(), 400),
        "asset": b.asset.as_ref().filter(|r| !r.id.is_empty()).map(|r| json!({ "type": r.kind, "id": r.id })),
        "files": files,
        "connection_name": provider,
    }))
}

/// Send after a yes, through whichever connection serves `email`.
pub async fn do_email(app: &App, slug: &str, preview: &Value) -> Result<Value, ApiError> {
    let to = preview["to"].as_str().unwrap_or("").to_string();
    let subject = preview["subject"].as_str().unwrap_or("").to_string();
    let text = preview["text"].as_str().unwrap_or("").to_string();
    let files = match (preview["asset"]["type"].as_str(), preview["asset"]["id"].as_str()) {
        (Some(k), Some(i)) => {
            let a = assets::load(&app.store, slug, k, i)?;
            asset_parts(app, slug, &a).map_err(|e| bad(e.to_string()))?.1
        }
        _ => vec![],
    };
    let m = {
        let cfg = app.config.read().await.clone();
        let reg = app.registry.read().await;
        reg.provider(slug, "email", cfg.abilities.get("email").map(String::as_str), |m| crate::ext::ready_in(app, Some(slug), m)).map(|(_, m)| m.clone()).ok_or_else(|| bad("no mail ability is set up for this project; add an email channel under Channels"))?
    };
    let result = if m.transport == Transport::Builtin && m.id == "kiqr-smtp" {
        let r = smtp_send(&kiqr_core::secrets::env_fn(app.store.home(), Some(slug)), &to, &subject, &text, files).await.map_err(|e| bad(format!("{e:#}")))?;
        json!({ "sent": true, "to": to, "from": r.from, "subject": subject, "connection": m.name })
    } else {
        let provide = m.provides.get("email").cloned().unwrap_or_default();
        let args = json!({ "to": to, "subject": subject, "text": text, "html": channels::markdown_to_html(&text) });
        let out = crate::ext::execute(app, slug, &m, &provide, kiqr_core::abilities::find("email").as_ref(), None, &args).await.map_err(|e| bad(format!("{e:#}")))?;
        json!({ "sent": true, "to": to, "subject": subject, "connection": m.name, "response": out.json })
    };
    app.bus.emit(Some(slug), EventKind::SeemoSays { text: format!("I sent \"{subject}\" to {to}.") });
    Ok(result)
}

/// `POST /api/projects/{slug}/email`: from the UI or CLI; the click is the yes.
async fn email_route(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<EmailBody>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let preview = email_preview(&app, &slug, &b).await?;
    Ok(Json(do_email(&app, &slug, &preview).await?))
}

pub struct Sent {
    pub from: String,
}

/// Send one email over SMTP with the `.env` settings: Markdown text as both
/// plain text and HTML, files attached.
pub async fn smtp_send(env: &(impl Fn(&str) -> Option<String> + Sync), to: &str, subject: &str, text_md: &str, files: Vec<(String, Vec<u8>, String)>) -> Result<Sent> {
    use lettre::message::{header::ContentType, Attachment, Mailbox, MultiPart, SinglePart};
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
    let host = env("SMTP_HOST").ok_or_else(|| anyhow!("SMTP_HOST is not set"))?;
    let user = env("SMTP_USER").ok_or_else(|| anyhow!("SMTP_USER is not set"))?;
    // No password: a relay that needs none (on this machine or a private network).
    let pass = env("SMTP_PASSWORD");
    let port: u16 = env("SMTP_PORT").and_then(|p| p.parse().ok()).unwrap_or(587);
    let from_s = env("SMTP_FROM").unwrap_or_else(|| user.clone());
    let from: Mailbox = from_s.parse().with_context(|| format!("SMTP_FROM '{from_s}' is not an address"))?;
    let mut builder = Message::builder().from(from.clone()).subject(subject);
    for addr in to.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()) {
        let mb: Mailbox = addr.parse().with_context(|| format!("'{addr}' is not an email address"))?;
        builder = builder.to(mb);
    }
    let html = format!("<div style=\"font-family:Inter,Helvetica,Arial,sans-serif;font-size:15px;line-height:1.55;color:#111\">{}</div>", channels::markdown_to_html(text_md));
    let alternative = MultiPart::alternative_plain_html(text_md.to_string(), html);
    let msg = if files.is_empty() {
        builder.multipart(alternative)?
    } else {
        let mut mixed = MultiPart::mixed().multipart(alternative);
        for (name, bytes, mime) in files {
            let ct = ContentType::parse(&mime).unwrap_or(ContentType::parse("application/octet-stream").unwrap());
            let part: SinglePart = Attachment::new(name).body(bytes, ct);
            mixed = mixed.singlepart(part);
        }
        builder.multipart(mixed)?
    };
    let security = env("SMTP_SECURITY").map(|s| s.to_ascii_lowercase()).unwrap_or_else(|| if port == 465 { "tls".into() } else { "starttls".into() });
    let mut builder = match security.as_str() {
        "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&host)?.port(port),
        // Plain: only for a relay on this machine or a private network.
        "none" | "plain" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host).port(port),
        _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)?.port(port),
    };
    if let Some(pass) = pass {
        builder = builder.credentials(Credentials::new(user, pass));
    }
    let transport = builder.build();
    transport.send(msg).await.with_context(|| format!("sending through {host}:{port}"))?;
    Ok(Sent { from: from.to_string() })
}
