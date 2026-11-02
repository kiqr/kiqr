//! Publishing: content goes out to a channel (a profile, a page, a community,
//! a list of people) as the channel's publisher. Here: the approvals behind
//! `publish_asset` and `send_email`, the user's own approve / schedule /
//! publish / unpublish on an asset, the scheduled publishes the daemon does,
//! and sending to a list (one message per person). The channel's `publish`
//! says whether Seemo publishes on its own (`auto`), asks first (`ask`) or
//! not at all (`off`); a spot that is no channel yet always asks, and the
//! yes makes it one.

use crate::api::{ApiError, ApiResult};
use crate::App;
use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json, Router,
};
use chrono::{DateTime, Utc};
use kiqr_core::vault::Credential;
use kiqr_core::assets::{self, Asset, Publication, Status};
use kiqr_core::channels::{Channel, Mode};
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{Kind, Manifest};
use kiqr_core::project::Project;
use kiqr_core::publishing;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/assets/{kind}/{id}/publish", axum::routing::post(publish_route))
        .route("/api/projects/{slug}/assets/{kind}/{id}/approve", axum::routing::post(approve_route))
        .route("/api/projects/{slug}/assets/{kind}/{id}/schedule", axum::routing::post(schedule_route).delete(unschedule_route))
        .route("/api/projects/{slug}/assets/{kind}/{id}/unpublish", axum::routing::post(unpublish_route))
        .route("/api/projects/{slug}/email", axum::routing::post(email_route))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

/// What the publisher sets itself on every publish; never an option.
const PUBLISH_ARGS: &[&str] = &["title", "subject", "text", "html", "summary", "tags", "asset", "note", "files", "date", "datetime", "image", "to"];

/// The choices a kind's publish tool takes per publish: the properties of its
/// input beyond what the publisher fills in and the spot's target fields.
pub(crate) fn publish_options(m: &Manifest, k: &Kind) -> serde_json::Map<String, Value> {
    let input = m.tools.iter().find(|t| t.name == k.publish).map(|t| t.input.clone()).unwrap_or(Value::Null);
    input["properties"].as_object().map(|o| o.iter().filter(|(n, _)| !PUBLISH_ARGS.contains(&n.as_str()) && !k.target.contains_key(*n) && n.as_str() != "connection").map(|(n, v)| (n.clone(), v.clone())).collect()).unwrap_or_default()
}

/// Keep the options a kind offers, with an allowed value; refuse anything else.
fn check_options(m: &Manifest, k: &Kind, given: &serde_json::Map<String, Value>) -> Result<serde_json::Map<String, Value>, ApiError> {
    let offered = publish_options(m, k);
    let mut out = serde_json::Map::new();
    for (n, v) in given {
        let Some(spec) = offered.get(n) else {
            return Err(bad(format!("{} takes no option '{n}'{}", k.display_name(), if offered.is_empty() { String::new() } else { format!(" (it takes {})", offered.keys().cloned().collect::<Vec<_>>().join(", ")) })));
        };
        if let Some(allowed) = spec["enum"].as_array() {
            if !allowed.contains(v) {
                return Err(bad(format!("'{n}' is one of {}", allowed.iter().map(|a| a.as_str().map(str::to_string).unwrap_or_else(|| a.to_string())).collect::<Vec<_>>().join(", "))));
            }
        }
        out.insert(n.clone(), v.clone());
    }
    // An option the tool requires is prepared for each publish (the files a site is built from, which Seemo writes).
    let input = m.tools.iter().find(|t| t.name == k.publish).map(|t| &t.input);
    for n in input.and_then(|i| i["required"].as_array()).into_iter().flatten().filter_map(Value::as_str) {
        if offered.contains_key(n) && out.get(n).is_none_or(|v| v.is_null() || v == "" || v.as_array().is_some_and(Vec::is_empty)) {
            let what = offered[n]["description"].as_str().map(|d| format!(" ({d})")).unwrap_or_default();
            return Err(bad(format!("{} needs '{n}' for each publish{what}; ask Seemo to prepare it", k.display_name())));
        }
    }
    Ok(out)
}

/// The publish tool's required inputs that are offered per publish but have no fixed values: what Seemo writes for each publish (a site's source files).
pub(crate) fn prepared_inputs(m: &Manifest, k: &Kind) -> Vec<String> {
    let offered = publish_options(m, k);
    m.tools.iter().find(|t| t.name == k.publish).and_then(|t| t.input["required"].as_array()).into_iter().flatten().filter_map(Value::as_str).filter(|n| offered.get(*n).is_some_and(|s| s["enum"].as_array().is_none_or(Vec::is_empty))).map(str::to_string).collect()
}

/// The ability and kind that publish to a channel: for a list, the mail
/// sender's list kind; else the kind the channel is.
pub(crate) async fn publisher_of(app: &App, slug: &str, ch: &Channel) -> Result<(Manifest, Kind), ApiError> {
    let found = if ch.is_list() {
        crate::channels::list_sender(app, slug).await.and_then(|m| m.kinds.iter().find(|k| k.section() == "list" && k.publishes()).cloned().map(|k| (m, k)))
    } else {
        crate::channels::kind_for(app, slug, ch).await.filter(|(_, k)| k.publishes())
    };
    found.ok_or_else(|| bad(format!("nothing on here publishes to {}; switch its ability on under Setup → Abilities", ch.label)))
}

/// Somewhere to publish that may be no channel yet: a kind, the credential to publish as (default: the first the ability may use), its fields.
#[derive(Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct NewSpot {
    pub kind: String,
    #[serde(alias = "account")]
    pub credential: String,
    pub fields: BTreeMap<String, Value>,
}

/// Where a publish goes: the credential it goes out as (none for an ability that needs none), the channel (made, not saved, when the spot is new), and whether it is new.
pub(crate) struct Target {
    pub publisher: Option<Credential>,
    pub channel: Channel,
    pub new: bool,
}

impl Target {
    fn who(&self) -> String {
        self.publisher.as_ref().map(crate::creds::name_of).unwrap_or_default()
    }
    fn view(&self) -> Value {
        json!({ "channel": (!self.new).then(|| self.channel.id.clone()), "label": self.channel.label, "kind": self.channel.kind, "domain": self.channel.domain, "ownership": self.channel.ownership, "fields": self.channel.fields, "url": self.channel.url, "credential": self.publisher.as_ref().map(|c| c.id.clone()), "account_label": self.who(), "new": self.new })
    }
}

/// A new channel for a spot of a kind, published to as the credential.
pub(crate) async fn spot_channel(app: &App, slug: &str, c: Option<&Credential>, m: &Manifest, k: &Kind, fields: BTreeMap<String, Value>, by: &str) -> Result<Channel, ApiError> {
    let mut ch = Channel::for_spot(k, &c.map(crate::creds::name_of).unwrap_or_default(), fields, by).map_err(|e| bad(e.to_string()))?;
    if ch.domain.is_empty() {
        ch.domain = crate::channels::kind_domain(m, k);
    }
    ch.publisher = c.map(|c| c.id.clone()).unwrap_or_default();
    ch.id = Channel::new_id(&app.store, slug, &ch.label);
    Ok(ch)
}

/// The target a request means: a channel by id or name, else a spot (a
/// channel already, or new), else the one the asset was approved for. The
/// channel must let Seemo publish, and its publisher must work.
pub(crate) async fn resolve(app: &App, slug: &str, named: &str, spot: Option<&NewSpot>, a: Option<&Asset>) -> Result<Target, ApiError> {
    let (publisher, channel, new) = if !named.trim().is_empty() {
        let ch = Channel::find(&app.store, slug, named).filter(|c| !c.suggested).ok_or_else(|| {
            let have: Vec<String> = Channel::all(&app.store, slug).unwrap_or_default().into_iter().filter(|e| !e.suggested).map(|e| format!("{} ({})", e.id, e.label)).collect();
            bad(if have.is_empty() { "there are no channels yet; add where the business's people are first (a profile, a community, a list)".to_string() } else { format!("there is no channel '{}'; there is {}", named.trim(), have.join(", ")) })
        })?;
        (publisher_for(app, slug, &ch).await?, ch, false)
    } else if let Some(sp) = spot.filter(|s| !s.kind.trim().is_empty()) {
        let (m, k) = crate::channels::kind(app, slug, sp.kind.trim()).await.ok_or_else(|| bad(format!("nothing on here publishes '{}'", sp.kind.trim())))?;
        let conn = crate::creds::pick(app, slug, &m, Some(sp.credential.as_str()).filter(|c| !c.is_empty())).await.map_err(|e| bad(e.to_string()))?;
        let c = conn.map(|c| c.primary().clone());
        let fields = if k.account_is_target { BTreeMap::new() } else { k.check_target(&sp.fields).map_err(|e| bad(e.to_string()))? };
        match crate::channels::matching(&app.store, slug, c.as_ref().map(|c| c.id.as_str()).unwrap_or(""), &k, &crate::channels::kind_domain(&m, &k), &fields) {
            Some(found) => (publisher_for(app, slug, &found).await.ok().flatten().or(c), found, false),
            None => (c.clone(), spot_channel(app, slug, c.as_ref(), &m, &k, fields, "user").await?, true),
        }
    } else {
        let id = a.and_then(|a| a.target.clone()).filter(|t| !t.is_empty()).ok_or_else(|| bad("name where it goes (`target`, a channel); it is not approved for one yet"))?;
        let ch = Channel::load(&app.store, slug, &id).map_err(|_| bad(format!("it was approved for '{id}', which is no longer a channel; pick another")))?;
        (publisher_for(app, slug, &ch).await?, ch, false)
    };
    if channel.publish == Mode::Off {
        return Err(bad(format!("publishing to {} is off; the user can turn it on on the channel", channel.label)));
    }
    if let Some(c) = &publisher {
        let st = crate::creds::state(app, slug, c).await;
        if !st.ready() {
            return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("{}; the user sees to it in the Vault", crate::creds::state_text(&c.label, &st))));
        }
    }
    Ok(Target { publisher, channel, new })
}

/// Who a channel publishes as: its publisher, required when its ability asks for a credential.
async fn publisher_for(app: &App, slug: &str, ch: &Channel) -> Result<Option<Credential>, ApiError> {
    if ch.publisher.is_empty() {
        let needs = crate::channels::publishing_ability(app, slug, ch).await.is_none_or(|m| !m.needs().is_empty());
        if needs {
            return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("{} has no publisher yet; the user picks who Seemo posts as there on the Channels page", ch.label)));
        }
        return Ok(None);
    }
    app.vault.get(&ch.publisher)?.map(Some).ok_or_else(|| ApiError(StatusCode::PRECONDITION_FAILED, format!("the credential {} published as is gone; the user picks another on the Channels page", ch.label)))
}

/// A new spot the user said yes to becomes a channel; the channel as saved.
async fn keep(app: &App, slug: &str, t: &Target) -> Result<Channel, ApiError> {
    if !t.new {
        return Ok(t.channel.clone());
    }
    let mut ch = t.channel.clone();
    if let Some(s) = Channel::all(&app.store, slug).unwrap_or_default().into_iter().find(|x| x.suggested && x.kind == ch.kind && kiqr_core::channels::same_fields(&x.fields, &ch.fields)) {
        ch.id = s.id;
    }
    ch.suggested = false;
    ch.save(&app.store, slug).map_err(|x| bad(x.to_string()))?;
    let _ = app.store.commit(&format!("channels({slug}): add {} as {}", ch.label, t.who())).await;
    crate::channels::added(app, slug, &ch);
    Ok(ch)
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
    /// Choices the kind's publish tool offers (the channel's `options`), e.g. `{status: "publish"}`.
    pub options: serde_json::Map<String, Value>,
    /// The channel, by id or name; empty: `spot`, else the one the asset was approved for.
    pub target: String,
    /// Somewhere that is no channel yet (a subreddit): it becomes one with the yes.
    pub spot: Option<NewSpot>,
    /// Set, the asset is scheduled for then instead of published now.
    pub at: Option<DateTime<Utc>>,
    pub note: String,
    pub by: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct AssetRef {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
}

/// What the card shows before a yes: the asset, where it goes and through whom, the first lines.
pub async fn publish_preview(app: &App, slug: &str, b: &PublishBody) -> Result<Value, ApiError> {
    let a = assets::load(&app.store, slug, &b.asset.kind, &b.asset.id)?;
    if a.archived {
        return Err(bad("it is archived; bring it back first"));
    }
    if b.at.is_some_and(|t| t <= Utc::now()) {
        return Err(bad("that time has passed; leave `at` out to publish now"));
    }
    let t = resolve(app, slug, &b.target, b.spot.as_ref(), Some(&a)).await?;
    let (pm, pk) = publisher_of(app, slug, &t.channel).await?;
    let options = check_options(&pm, &pk, &b.options)?;
    let (body, files) = asset_parts(app, slug, &a).map_err(|e| bad(e.to_string()))?;
    // To a list: how many people, so the card can say.
    let recipients = t.channel.is_list().then(|| kiqr_core::lists::recipients(app.store.home(), slug, &t.channel.id).len());
    Ok(json!({
        "options": options,
        "publisher": pm.id,
        "publisher_name": pm.name,
        "asset": { "type": a.kind, "id": a.id, "title": a.title, "status": a.status },
        "target": t.view(),
        "recipients": recipients,
        "subject": a.title,
        "excerpt": kiqr_core::profile::truncate_chars(body.trim(), 400),
        "words": body.split_whitespace().count(),
        "files": files.iter().map(|(n, b, m)| json!({ "name": n, "bytes": b.len(), "mime": m })).collect::<Vec<_>>(),
        "at": b.at,
        "note": b.note,
        // On its own only where the channel lets Seemo publish on its own: the card is the record. Somewhere new asks.
        "asks_first": t.channel.publish != Mode::Auto || t.new,
    }))
}

/// Publish what the card showed (never resolved again), or, when it carries
/// `at`, approve it for that target and leave it to the scheduler. After a yes.
pub async fn do_publish(app: &App, slug: &str, preview: &Value, by: &str) -> Result<Value, ApiError> {
    let kind = preview["asset"]["type"].as_str().unwrap_or("").to_string();
    let id = preview["asset"]["id"].as_str().unwrap_or("").to_string();
    let mut a = assets::load(&app.store, slug, &kind, &id)?;
    let tv = &preview["target"];
    let c: Option<Credential> = match tv["credential"].as_str().filter(|c| !c.is_empty()) {
        Some(id) => Some(app.vault.get(id)?.ok_or_else(|| bad("the credential on the card is gone"))?),
        None => None,
    };
    let entry = match tv["channel"].as_str() {
        Some(eid) => Channel::load(&app.store, slug, eid).map_err(|_| bad("the channel on the card is gone"))?,
        None => {
            let (m, k) = crate::channels::kind(app, slug, tv["kind"].as_str().unwrap_or("")).await.ok_or_else(|| bad("nothing publishes there any more"))?;
            let fields: BTreeMap<String, Value> = tv["fields"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
            let ch = spot_channel(app, slug, c.as_ref(), &m, &k, fields, if by == "user" { "user" } else { "seemo" }).await?;
            if let Some(c) = &c {
                crate::access::grant(app, slug, &m.id, &c.id, "publisher").await?;
            }
            keep(app, slug, &Target { publisher: c.clone(), channel: ch, new: true }).await?
        }
    };
    if let Some(at) = preview["at"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Utc)) {
        a.publish_options = preview["options"].as_object().cloned().unwrap_or_default();
        let a = set_schedule(app, slug, a, &entry, Some(at)).await?;
        return Ok(json!({
            "scheduled": true,
            "asset": { "type": a.kind, "id": a.id, "title": a.title, "status": a.status },
            "target": entry.id,
            "target_label": entry.label,
            "at": at,
            "note": "Scheduled. The daemon publishes it at that time; the asset is marked scheduled until then.",
        }));
    }
    let (m, k) = publisher_of(app, slug, &entry).await?;
    let cid = c.as_ref().map(|c| c.id.clone()).unwrap_or_default();
    let who = c.as_ref().map(crate::creds::name_of).unwrap_or_else(|| m.name.clone());
    let (body, _) = asset_parts(app, slug, &a).map_err(|e| bad(e.to_string()))?;
    // Links to the business's own site carry UTM parameters, so analytics can tell what KIQR sent where.
    let site_host = Project::load(&app.store, slug).ok().and_then(|p| url::Url::parse(&p.url).ok()).and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
    let campaign = a.source.get("campaign").and_then(|v| v.as_str()).map(str::to_string);
    let body = publishing::add_utm(&body, &site_host, &k.kind, campaign.as_deref());
    let subject = preview["subject"].as_str().unwrap_or(&a.title).to_string();
    let provide = m.tools.iter().find(|t| t.name == k.publish).map(|t| t.provide.clone()).or_else(|| m.provides.get(&k.publish).cloned()).ok_or_else(|| bad(format!("{} has no '{}'", m.name, k.publish)))?;
    let where_ = format!("{} as {who}", entry.label);
    let now = Utc::now();
    let run = by.starts_with("task:").then(|| by.to_string());
    // To a list of people: one message each, in the background; the publish is recorded when the send starts.
    if entry.is_list() {
        let conn = crate::creds::pick(app, slug, &m, Some(cid.as_str()).filter(|c| !c.is_empty())).await.map_err(|e| bad(e.to_string()))?;
        let started = crate::lists::send(app, slug, conn, &m, &provide, &entry, &subject, &body, Some(AssetRef { kind: a.kind.clone(), id: a.id.clone() }), by).await?;
        let a = record(app, slug, a, &entry, Publication { channel: entry.id.clone(), credential: cid.clone(), ability: m.id.clone(), from: None, to: None, url: None, at: now, run }, &where_).await?;
        return Ok(json!({
            "published": true,
            "sending": true,
            "asset": { "type": a.kind, "id": a.id, "title": a.title },
            "target": entry.id,
            "target_label": entry.label,
            "recipients": started,
            "detail": format!("sending to {started} people"),
            "note": format!("Sending to {started} people on {}, one message each, in the background; the list page shows how far it got.", entry.label),
        }));
    }
    let mut args = json!({
        "title": a.title,
        "subject": subject,
        "text": body,
        "html": publishing::markdown_to_html(&body),
        "summary": a.summary,
        "tags": a.tags,
        "asset": { "type": a.kind, "id": a.id },
        "note": preview["note"],
        // For files a site builds from (frontmatter, a dated path): the day of this publish, and the asset's first image as named in `{{files}}`.
        "date": now.format("%Y-%m-%d").to_string(),
        "datetime": now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "image": a.files.iter().find(|f| f.role != "body" && mime_guess::from_path(&f.path).first_or_octet_stream().type_() == "image").map(|f| f.path.clone()),
    });
    for (dk, dv) in &crate::channels::fields_for(&entry, &k) {
        args[dk] = dv.clone();
    }
    // This publish's own choices last: they were on the card the user said yes to.
    if let Some(o) = preview["options"].as_object() {
        for (n, v) in o {
            args[n] = v.clone();
        }
    }
    let out = crate::ext::execute(app, slug, &m, &provide, None, Some(&k.publish), &args, Some(cid.as_str()).filter(|c| !c.is_empty())).await.map_err(|e| bad(format!("{e:#}")))?;
    let url = out.text.clone().filter(|t| t.starts_with("http")).or_else(|| out.json.get("url").and_then(|u| u.as_str()).map(str::to_string)).or_else(|| out.json.get("link").and_then(|u| u.as_str()).map(str::to_string));
    let from = out.json.get("from").and_then(|f| f.as_str()).map(str::to_string);
    let detail = out.text.clone().unwrap_or_else(|| kiqr_core::profile::truncate_chars(&out.json.to_string(), 200));
    let a = record(app, slug, a, &entry, Publication { channel: entry.id.clone(), credential: cid.clone(), ability: m.id.clone(), from, to: None, url: url.clone(), at: now, run }, &where_).await?;
    Ok(json!({
        "published": true,
        "asset": { "type": a.kind, "id": a.id, "title": a.title },
        "target": entry.id,
        "target_label": entry.label,
        "url": url,
        "detail": detail,
        "note": format!("Published to {where_}. The asset is marked published."),
    }))
}

/// The asset went out: its publication, the channel's count, a commit, the event.
async fn record(app: &App, slug: &str, mut a: Asset, entry: &Channel, p: Publication, where_: &str) -> Result<Asset, ApiError> {
    let (credential, url) = (p.credential.clone(), p.url.clone());
    a.publications.push(p);
    a.target = Some(entry.id.clone());
    a.publish_at = None;
    a.publish_error = None;
    let a = assets::save(&app.store, slug, &a)?;
    crate::api::emit_saved(app, slug, &a, false);
    if let Ok(mut e) = Channel::load(&app.store, slug, &entry.id) {
        e.publications += 1;
        e.last_published = Some(Utc::now());
        let _ = e.save(&app.store, slug);
    }
    app.store.commit(&format!("[auto] publish({slug}): {} → {where_}", a.title)).await?;
    app.bus.emit(Some(slug), EventKind::AssetPublished { asset_type: a.kind.clone(), id: a.id.clone(), channel: entry.id.clone(), credential, url });
    Ok(a)
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
// channel the user approved the asset for, the time the daemon
// publishes it, and whether KIQR still counts it as published.

#[derive(Deserialize, Default)]
#[serde(default)]
struct ApproveBody {
    target: String,
    spot: Option<NewSpot>,
}

#[derive(Deserialize)]
struct ScheduleBody {
    at: DateTime<Utc>,
    #[serde(default)]
    target: String,
    #[serde(default)]
    spot: Option<NewSpot>,
    /// This publish's choices (see `PublishBody::options`).
    #[serde(default)]
    options: serde_json::Map<String, Value>,
}

/// Approve the asset for a channel, with or without a time. Ready, or Scheduled.
async fn set_schedule(app: &App, slug: &str, mut a: Asset, e: &Channel, at: Option<DateTime<Utc>>) -> Result<Asset, ApiError> {
    if a.archived {
        return Err(bad("it is archived; bring it back first"));
    }
    if !a.publications.is_empty() {
        return Err(bad("it is published already; unpublish it first"));
    }
    a.target = Some(e.id.clone());
    a.publish_at = at;
    a.publish_error = None;
    let a = assets::save(&app.store, slug, &a)?;
    crate::api::emit_saved(app, slug, &a, false);
    let _ = app.store.commit(&format!("assets({slug}): {} for {}", a.title, e.label)).await;
    app.bus.emit(Some(slug), EventKind::AssetApproved { asset_type: a.kind.clone(), id: a.id.clone(), title: a.title.clone(), target: e.id.clone(), at });
    Ok(a)
}

/// `POST …/approve {target | spot}`: the user's yes to this asset, for that channel (a new spot becomes one). It is Ready.
async fn approve_route(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>, Json(b): Json<ApproveBody>) -> ApiResult<Asset> {
    let _ = Project::load(&app.store, &slug)?;
    let a = assets::load(&app.store, &slug, &kind, &id)?;
    let t = resolve(&app, &slug, &b.target, b.spot.as_ref(), Some(&a)).await?;
    publisher_of(&app, &slug, &t.channel).await?;
    let e = keep(&app, &slug, &t).await?;
    let at = a.publish_at;
    Ok(Json(set_schedule(&app, &slug, a, &e, at).await?))
}

/// `POST …/schedule {at, target?}`: publish it then, to its entry or the one named. Scheduled.
async fn schedule_route(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>, Json(b): Json<ScheduleBody>) -> ApiResult<Asset> {
    let _ = Project::load(&app.store, &slug)?;
    let a = assets::load(&app.store, &slug, &kind, &id)?;
    if b.at <= Utc::now() {
        return Err(bad("that time has passed; publish it now instead"));
    }
    let t = resolve(&app, &slug, &b.target, b.spot.as_ref(), Some(&a)).await?;
    let (m, k) = publisher_of(&app, &slug, &t.channel).await?;
    let mut a = a;
    a.publish_options = check_options(&m, &k, &b.options)?;
    let e = keep(&app, &slug, &t).await?;
    Ok(Json(set_schedule(&app, &slug, a, &e, Some(b.at)).await?))
}

/// `DELETE …/schedule`: drop the time; it stays approved for its entry (Ready).
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
/// down: there is no take-back out there, and the record says so. The asset
/// goes back to Ready for its entry.
async fn unpublish_route(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>) -> ApiResult<Asset> {
    let _ = Project::load(&app.store, &slug)?;
    let mut a = assets::load(&app.store, &slug, &kind, &id)?;
    if a.publications.is_empty() {
        return Err(bad("it is not published"));
    }
    if a.target.is_none() {
        a.target = a.publications.last().map(|p| p.channel.clone()).filter(|x| !x.is_empty());
    }
    a.publications.clear();
    let a = assets::save(&app.store, &slug, &a)?;
    crate::api::emit_saved(&app, &slug, &a, false);
    app.bus.emit(Some(&slug), EventKind::AssetUnpublished { asset_type: a.kind.clone(), id: a.id.clone(), title: a.title.clone() });
    Ok(Json(a))
}

/// Publish what is due: Scheduled assets whose time has come, to the entry
/// they were approved for. The scheduler calls this every tick. A failure
/// stays on the asset (it falls back to Ready, with the reason) and Seemo says so.
pub async fn publish_due(app: &App, slug: &str, now: DateTime<Utc>) {
    let Ok(list) = assets::list(&app.store, slug) else { return };
    for a in list.into_iter().filter(|a| a.status == Status::Scheduled && a.publish_at.is_some_and(|t| t <= now)) {
        let key = format!("publish:{slug}/{}/{}", a.kind, a.id);
        if !app.jobs.write().await.insert(key.clone()) {
            continue;
        }
        let body = PublishBody { asset: AssetRef { kind: a.kind.clone(), id: a.id.clone() }, target: a.target.clone().unwrap_or_default(), options: a.publish_options.clone(), ..Default::default() };
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
    /// One address or several, separated by commas; or leave it out and name a `list`.
    pub to: String,
    /// A list (a channel, by id or name): one message each, from the list's publisher.
    pub list: String,
    pub subject: String,
    pub text: String,
    pub asset: Option<AssetRef>,
    /// Which mail server sends it (a credential, by id or name); the list's, else the first allowed, when empty.
    pub from: String,
}

/// The ability that sends email here, and the credential a send goes from.
async fn mail_from(app: &App, slug: &str, named: &str, list: Option<&Channel>) -> Result<(Manifest, Option<kiqr_core::conn::Conn>), ApiError> {
    let m = {
        let cfg = app.config.read().await.clone();
        let reg = app.registry.read().await;
        reg.provider(slug, "email", cfg.abilities.get("email").map(String::as_str), |m| crate::ext::ready_in(app, Some(slug), m)).map(|(_, m)| m.clone())
    };
    let Some(m) = m else {
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, "nothing here sends email yet; the user installs an ability that does and gives it a mail server from the Vault".into()));
    };
    if let Some(l) = list {
        if l.publish == Mode::Off {
            return Err(bad(format!("sending to {} is off; the user can turn it on on the list", l.label)));
        }
    }
    let name = list.map(|l| l.publisher.as_str()).filter(|a| !a.is_empty()).or(Some(named.trim()).filter(|n| !n.is_empty()));
    let from = crate::creds::pick(app, slug, &m, name).await.map_err(|e| bad(e.to_string()))?;
    Ok((m, from))
}

pub async fn email_preview(app: &App, slug: &str, b: &EmailBody) -> Result<Value, ApiError> {
    let to = b.to.trim();
    let list = match b.list.trim() {
        "" => None,
        l => Some(kiqr_core::lists::find(&app.store, slug, l).ok_or_else(|| bad(format!("no list '{l}'; lists are channels")))?),
    };
    match (&list, to.is_empty()) {
        (None, true) => return Err(bad("`to` needs an email address, or name a `list`")),
        (Some(_), false) => return Err(bad("name `to` or a `list`, not both")),
        (None, false) if !to.contains('@') => return Err(bad("`to` needs an email address")),
        _ => {}
    }
    if b.subject.trim().is_empty() {
        return Err(bad("an email needs a subject"));
    }
    let (m, from) = mail_from(app, slug, &b.from, list.as_ref()).await?;
    let recipients = list.as_ref().map(|l| kiqr_core::lists::recipients(app.store.home(), slug, &l.id).len());
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
        "list": list.as_ref().map(|l| l.id.clone()),
        "list_label": list.as_ref().map(|l| l.label.clone()),
        "recipients": recipients,
        "subject": b.subject.trim(),
        "text": b.text.trim(),
        "excerpt": kiqr_core::profile::truncate_chars(b.text.trim(), 400),
        "asset": b.asset.as_ref().filter(|r| !r.id.is_empty()).map(|r| json!({ "type": r.kind, "id": r.id })),
        "files": files,
        "ability_name": m.name,
        "from": from.as_ref().map(|c| c.id().to_string()),
        "from_label": from.as_ref().map(|c| crate::creds::name_of(c.primary())),
        // On its own only to a list whose channel lets Seemo send on its own; one address always asks.
        "asks_first": list.as_ref().is_none_or(|l| l.publish != Mode::Auto),
    }))
}

/// Send after a yes, from the account the card named.
pub async fn do_email(app: &App, slug: &str, preview: &Value) -> Result<Value, ApiError> {
    let to = preview["to"].as_str().unwrap_or("").to_string();
    let subject = preview["subject"].as_str().unwrap_or("").to_string();
    let text = preview["text"].as_str().unwrap_or("").to_string();
    let list = preview["list"].as_str().filter(|l| !l.is_empty()).map(|l| kiqr_core::lists::load(&app.store, slug, l)).transpose()?;
    let (m, c) = mail_from(app, slug, preview["from"].as_str().unwrap_or(""), list.as_ref()).await?;
    let provide = m.provides.get("email").cloned().unwrap_or_default();
    // To a list: one message per person, in the background.
    if let Some(l) = list {
        let asset = preview["asset"].as_object().and_then(|o| Some(AssetRef { kind: o.get("type")?.as_str()?.to_string(), id: o.get("id")?.as_str()?.to_string() }));
        let from = c.as_ref().map(|c| crate::creds::name_of(c.primary()));
        let n = crate::lists::send(app, slug, c, &m, &provide, &l, &subject, &text, asset, "seemo").await?;
        app.bus.emit(Some(slug), EventKind::SeemoSays { text: format!("I am sending \"{subject}\" to the {} people on {}.", n, l.label) });
        return Ok(json!({ "sent": true, "sending": true, "list": l.id, "list_label": l.label, "recipients": n, "from": from, "subject": subject, "note": format!("Sending to {n} people, one message each, in the background.") }));
    }
    // The asset's files go along; the executor reads them when the ability sends any.
    let args = json!({ "to": to, "subject": subject, "text": text, "html": publishing::markdown_to_html(&text), "asset": preview["asset"] });
    let out = crate::ext::execute(app, slug, &m, &provide, kiqr_core::abilities::find("email").as_ref(), None, &args, c.as_ref().map(|c| c.id())).await.map_err(|e| bad(format!("{e:#}")))?;
    let result = json!({ "sent": true, "to": to, "from": out.json.get("from"), "subject": subject, "response": out.json });
    app.bus.emit(Some(slug), EventKind::SeemoSays { text: format!("I sent \"{subject}\" to {to}.") });
    Ok(result)
}

/// `POST /api/projects/{slug}/email`: from the UI or CLI; the click is the yes.
async fn email_route(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<EmailBody>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let preview = email_preview(&app, &slug, &b).await?;
    Ok(Json(do_email(&app, &slug, &preview).await?))
}
