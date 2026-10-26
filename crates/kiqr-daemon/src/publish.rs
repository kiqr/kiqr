//! Publishing: channels (where content goes out), the approvals behind
//! `publish_asset`, `send_email` and `create_channel`, the user's own
//! approve / schedule / publish / unpublish on an asset, the scheduled
//! publishes the daemon does, and the built-in email sender. Every publish
//! asks the user first; this module does the sending once they said yes.

use crate::api::{ApiError, ApiResult};
use crate::App;
use anyhow::Result;
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
use kiqr_core::extensions::{ChannelKind, Manifest};
use kiqr_core::project::Project;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

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
pub(crate) async fn kinds(app: &App, slug: &str) -> Vec<(Manifest, ChannelKind)> {
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

/// What the publisher sets itself on every publish; never an option.
const PUBLISH_ARGS: &[&str] = &["title", "subject", "text", "html", "summary", "tags", "asset", "note", "files"];

/// The choices a kind's publish tool takes per publish: the properties of its
/// input beyond what the publisher fills in and the channel's destination.
pub(crate) fn publish_options(m: &Manifest, k: &ChannelKind) -> serde_json::Map<String, Value> {
    let input = m.tools.iter().find(|t| t.name == k.publish).map(|t| t.input.clone()).unwrap_or(Value::Null);
    input["properties"].as_object().map(|o| o.iter().filter(|(n, _)| !PUBLISH_ARGS.contains(&n.as_str()) && !k.destination.contains_key(*n) && n.as_str() != "connection").map(|(n, v)| (n.clone(), v.clone())).collect()).unwrap_or_default()
}

/// Keep the options a kind offers, with an allowed value; refuse anything else.
fn check_options(m: &Manifest, k: &ChannelKind, given: &serde_json::Map<String, Value>) -> Result<serde_json::Map<String, Value>, ApiError> {
    let offered = publish_options(m, k);
    let mut out = serde_json::Map::new();
    for (n, v) in given {
        let Some(spec) = offered.get(n) else {
            return Err(bad(format!("{} takes no option '{n}'{}", k.label, if offered.is_empty() { String::new() } else { format!(" (it takes {})", offered.keys().cloned().collect::<Vec<_>>().join(", ")) })));
        };
        if let Some(allowed) = spec["enum"].as_array() {
            if !allowed.contains(v) {
                return Err(bad(format!("'{n}' is one of {}", allowed.iter().map(|a| a.as_str().map(str::to_string).unwrap_or_else(|| a.to_string())).collect::<Vec<_>>().join(", "))));
            }
        }
        out.insert(n.clone(), v.clone());
    }
    Ok(out)
}

/// A kind a new channel could use. Each channel brings its own account: the
/// keys it asks for, or those of another channel of the same ability (`accounts`).
fn kind_view(app: &App, slug: &str, m: &Manifest, k: &ChannelKind) -> Value {
    let accounts: Vec<Value> = kiqr_core::connections::Connection::of(&app.store, slug, &m.id).iter().filter(|c| crate::accounts::ready(app, slug, c, m)).map(|c| json!({ "id": c.id, "label": c.label })).collect();
    json!({
        "kind": k.kind,
        "label": k.label,
        "name": k.display_name(),
        "icon": k.icon,
        "connection": m.id,
        "connection_name": m.name,
        "ready": true,
        "destination": k.destination,
        "auth": m.auth.iter().map(|a| json!({ "env": a.env, "label": a.label, "help": a.help, "optional": a.optional, "set": Value::Null })).collect::<Vec<_>>(),
        "connect": kiqr_core::connect::view(m.connect.as_ref()),
        "accounts": accounts,
        "options": publish_options(m, k),
    })
}

async fn channel_view(app: &App, slug: &str, c: &Channel) -> Value {
    crate::accounts::view(app, slug, c).await
}

/// Channels plus the kinds a new one could use.
async fn list_channels(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let mut list = Vec::new();
    for c in Channel::channels(&app.store, &slug)? {
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
    /// The ability to use when several publish the kind.
    pub connection: Option<String>,
    pub destination: serde_json::Map<String, Value>,
    /// Start with the keys of this channel or account of the same ability (the same mail account).
    pub same_as: Option<String>,
    /// The new channel's own keys, by name (from the user, never from Seemo).
    pub secrets: BTreeMap<String, String>,
}

/// Check a channel proposal against the kinds available; returns what will be saved.
pub async fn channel_preview(app: &App, slug: &str, b: &NewChannel) -> Result<Value, ApiError> {
    let kind = b.kind.trim().to_ascii_lowercase();
    if kind.is_empty() {
        return Err(bad("a channel needs a kind (email, or one a connection declares)"));
    }
    let all = kinds(app, slug).await;
    let Some((m, k)) = all.iter().find(|(m, k)| k.kind == kind && b.connection.as_deref().is_none_or(|c| c == m.id)).map(|x| (&x.0, &x.1)) else {
        let have: Vec<String> = all.iter().map(|(_, k)| k.kind.clone()).collect();
        return Err(bad(if have.is_empty() { "nothing can publish yet; install an ability that publishes under Setup → Abilities".to_string() } else { format!("nothing publishes '{kind}'; available: {}", have.join(", ")) }));
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
    // Its account: another one's keys, or the ones given; else the user adds them on the channel.
    let same = match b.same_as.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(Channel::find(&app.store, slug, &m.id, s).ok_or_else(|| bad(format!("{} has no account '{s}' in this project to share", m.name)))?),
        None => None,
    };
    let ready = match &same {
        Some(c) => crate::accounts::ready(app, slug, c, m),
        None => kiqr_core::extensions::missing_keys(m, |n| b.secrets.get(n).is_some_and(|v| !v.trim().is_empty())).is_empty(),
    };
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
        "same_as": same.as_ref().map(|c| c.id.clone()),
        "same_as_label": same.as_ref().map(|c| c.label.clone()),
        "ready": ready,
        "destination": destination,
    }))
}

/// Save a channel from a checked preview, with its keys when the user gave them.
pub async fn save_channel(app: &App, slug: &str, preview: &Value, by: &str, secrets: BTreeMap<String, String>) -> Result<Value, ApiError> {
    let new = crate::accounts::NewConnection {
        ability: preview["connection"].as_str().unwrap_or("").to_string(),
        label: preview["label"].as_str().unwrap_or("Channel").to_string(),
        kind: preview["kind"].as_str().unwrap_or("").to_string(),
        destination: preview["destination"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default(),
        secrets,
        same_as: preview["same_as"].as_str().map(str::to_string),
    };
    let c = crate::accounts::create(app, slug, new, by).await?;
    let mut v = channel_view(app, slug, &c).await;
    v["note"] = json!(if v["ready"].as_bool() == Some(true) { "The channel exists. Publishing to it still asks the user each time." } else { "The channel exists but has no account yet: the user connects it under Setup → Channels before anything can go out." });
    Ok(v)
}

async fn create_channel(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<NewChannel>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let _ = Project::load(&app.store, &slug)?;
    let preview = channel_preview(&app, &slug, &b).await?;
    let v = save_channel(&app, &slug, &preview, "user", b.secrets.clone()).await?;
    Ok((StatusCode::CREATED, Json(v)))
}

async fn get_channel(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let c = Channel::load(&app.store, &slug, &id)?;
    Ok(Json(channel_view(&app, &slug, &c).await))
}

async fn put_channel(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(p): Json<crate::accounts::Patch>) -> ApiResult<Value> {
    let c = crate::accounts::update(&app, &slug, &id, p).await?;
    Ok(Json(channel_view(&app, &slug, &c).await))
}

async fn delete_channel(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> Result<StatusCode, ApiError> {
    crate::accounts::delete(&app, &slug, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// -------------------------------------------------------------- publishing

/// An asset's body as Markdown and its media files, for sending.
pub(crate) fn asset_parts(app: &App, slug: &str, a: &Asset) -> Result<(String, Vec<(String, Vec<u8>, String)>)> {
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
    /// Choices the channel's publish tool offers (`list_channels` → `options`), e.g. `{status: "publish"}`.
    pub options: serde_json::Map<String, Value>,
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
        let have: Vec<String> = Channel::channels(&app.store, slug).unwrap_or_default().into_iter().map(|c| format!("{} ({})", c.id, c.label)).collect();
        bad(if have.is_empty() { "there is no channel yet; create one first (create_channel) or ask the user to add one under Setup → Channels".to_string() } else { format!("no channel '{}'; channels: {}", id.trim(), have.join(", ")) })
    })?;
    let cv = channel_view(app, slug, &c).await;
    if cv["ready"].as_bool() != Some(true) {
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("the channel '{}' has no account yet ({} needs {}); the user connects it under Setup → Channels", c.label, cv["connection_name"].as_str().unwrap_or(""), cv["missing"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default())));
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
    let options = {
        let reg = app.registry.read().await;
        match reg.connection(slug, &c.ability).and_then(|(_, m)| m.channels.iter().find(|k| k.kind == c.kind).map(|k| (m.clone(), k.clone()))) {
            Some((m, k)) => check_options(&m, &k, &b.options)?,
            None => serde_json::Map::new(),
        }
    };
    Ok(json!({
        "options": options,
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
        "asks_first": c.asks_first,
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
        a.publish_options = preview["options"].as_object().cloned().unwrap_or_default();
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
    let (body, _) = asset_parts(app, slug, &a).map_err(|e| bad(e.to_string()))?;
    // Links to the business's own site carry UTM parameters, so analytics can tell what KIQR sent where.
    let site_host = Project::load(&app.store, slug).ok().and_then(|p| url::Url::parse(&p.url).ok()).and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
    let campaign = a.source.get("campaign").and_then(|v| v.as_str()).map(str::to_string);
    let body = channels::add_utm(&body, &site_host, &c.kind, campaign.as_deref());
    let subject = preview["subject"].as_str().unwrap_or(&a.title).to_string();
    let m = {
        let reg = app.registry.read().await;
        reg.connection(slug, &c.ability).map(|(_, m)| m.clone()).ok_or_else(|| bad(format!("the ability '{}' is not installed or is off here", c.ability)))?
    };
    let k = m.channels.iter().find(|k| k.kind == c.kind).cloned().ok_or_else(|| bad(format!("{} no longer publishes '{}'", m.name, c.kind)))?;
    // The connection's publish tool, with the asset and the destination.
    let provide = m.tools.iter().find(|t| t.name == k.publish).map(|t| t.provide.clone()).or_else(|| m.provides.get(&k.publish).cloned()).ok_or_else(|| bad(format!("{} has no '{}'", m.name, k.publish)))?;
    let mut args = json!({
        "title": a.title,
        "subject": subject,
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
    // This publish's own choices last: they were on the card the user said yes to.
    if let Some(o) = preview["options"].as_object() {
        for (n, v) in o {
            args[n] = v.clone();
        }
    }
    let out = crate::ext::execute(app, slug, &m, &provide, None, Some(&k.publish), &args, Some(&c.id)).await.map_err(|e| bad(format!("{e:#}")))?;
    let url = out.text.clone().filter(|t| t.starts_with("http")).or_else(|| out.json.get("url").and_then(|u| u.as_str()).map(str::to_string)).or_else(|| out.json.get("link").and_then(|u| u.as_str()).map(str::to_string));
    let account = out.json.get("from").and_then(|f| f.as_str()).map(str::to_string);
    let detail = out.text.clone().unwrap_or_else(|| kiqr_core::profile::truncate_chars(&out.json.to_string(), 200));
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
    /// This publish's choices (see `PublishBody::options`).
    #[serde(default)]
    options: serde_json::Map<String, Value>,
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
    let mut a = a;
    a.publish_options = {
        let reg = app.registry.read().await;
        match reg.connection(&slug, &c.ability).and_then(|(_, m)| m.channels.iter().find(|k| k.kind == c.kind).map(|k| (m.clone(), k.clone()))) {
            Some((m, k)) => check_options(&m, &k, &b.options)?,
            None => serde_json::Map::new(),
        }
    };
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
        let body = PublishBody { asset: AssetRef { kind: a.kind.clone(), id: a.id.clone() }, channel: a.channel.clone().unwrap_or_default(), options: a.publish_options.clone(), ..Default::default() };
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
    /// Which mail account sends it (an email channel or account, by id or name); the default when empty.
    #[serde(alias = "connection")]
    pub from: String,
}

pub async fn email_preview(app: &App, slug: &str, b: &EmailBody) -> Result<Value, ApiError> {
    let to = b.to.trim();
    if to.is_empty() || !to.contains('@') {
        return Err(bad("`to` needs an email address"));
    }
    if b.subject.trim().is_empty() {
        return Err(bad("an email needs a subject"));
    }
    let m = {
        let cfg = app.config.read().await.clone();
        let reg = app.registry.read().await;
        reg.provider(slug, "email", cfg.abilities.get("email").map(String::as_str), |m| crate::ext::ready_in(app, Some(slug), m)).map(|(_, m)| m.clone())
    };
    let Some(m) = m else {
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, "no mail ability is set up for this project; install Email under Setup → Abilities and add an email channel".into()));
    };
    let provider = m.name.clone();
    let from = crate::ext::pick_connection(app, slug, &m, Some(b.from.as_str()).filter(|f| !f.trim().is_empty())).map_err(|e| bad(e.to_string()))?;
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
        "from": from.as_ref().map(|c| c.id.clone()),
        "from_label": from.as_ref().map(|c| c.label.clone()),
    }))
}

/// Send after a yes, through whichever connection serves `email`.
pub async fn do_email(app: &App, slug: &str, preview: &Value) -> Result<Value, ApiError> {
    let to = preview["to"].as_str().unwrap_or("").to_string();
    let subject = preview["subject"].as_str().unwrap_or("").to_string();
    let text = preview["text"].as_str().unwrap_or("").to_string();
    let m = {
        let cfg = app.config.read().await.clone();
        let reg = app.registry.read().await;
        reg.provider(slug, "email", cfg.abilities.get("email").map(String::as_str), |m| crate::ext::ready_in(app, Some(slug), m)).map(|(_, m)| m.clone()).ok_or_else(|| bad("no mail ability is set up for this project; add an email channel under Channels"))?
    };
    let provide = m.provides.get("email").cloned().unwrap_or_default();
    // The asset's files go along; the executor reads them when the connection sends any.
    let args = json!({ "to": to, "subject": subject, "text": text, "html": channels::markdown_to_html(&text), "asset": preview["asset"] });
    let out = crate::ext::execute(app, slug, &m, &provide, kiqr_core::abilities::find("email").as_ref(), None, &args, preview["from"].as_str()).await.map_err(|e| bad(format!("{e:#}")))?;
    let result = json!({ "sent": true, "to": to, "from": out.json.get("from"), "subject": subject, "connection": m.name, "response": out.json });
    app.bus.emit(Some(slug), EventKind::SeemoSays { text: format!("I sent \"{subject}\" to {to}.") });
    Ok(result)
}

/// `POST /api/projects/{slug}/email`: from the UI or CLI; the click is the yes.
async fn email_route(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<EmailBody>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let preview = email_preview(&app, &slug, &b).await?;
    Ok(Json(do_email(&app, &slug, &preview).await?))
}

