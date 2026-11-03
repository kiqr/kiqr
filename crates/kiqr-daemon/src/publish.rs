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
        .route("/api/projects/{slug}/assets/{kind}/{id}/attempt", axum::routing::post(attempt_route))
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
        match crate::channels::matching(&app.store, slug, c.as_ref().map(|c| c.id.as_str()).unwrap_or(""), &m.id, &k, &crate::channels::kind_domain(&m, &k), &fields) {
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
/// Room left under a network's limit when Seemo shortens a post: links and
/// emoji count longer on some networks than their characters.
const SHORTEN_MARGIN: usize = 20;

/// A post longer than the network takes is shortened by Seemo before it goes
/// anywhere (at the yes, at a schedule, at the publish, and by the sweep):
/// in a browser it would only find Post greyed out. The original stays on
/// the asset (`source.shortened`) and Seemo says what it did. Refused only
/// when shortening fails. The new text follows the channel's rule: where
/// Seemo publishes on its own (`publish: auto`) it stays approved; anywhere
/// else nobody has said yes to it yet, so it goes back to Draft (`true`).
async fn fit(app: &App, slug: &str, a: Asset, k: &Kind, ch: &Channel) -> Result<(Asset, bool), ApiError> {
    let Some(limit) = k.text_limit.map(|l| l as usize) else { return Ok((a, false)) };
    let (body, _) = asset_parts(app, slug, &a).map_err(|e| bad(e.to_string()))?;
    let n = body.trim().chars().count();
    if n <= limit {
        return Ok((a, false));
    }
    let title = a.title.clone();
    let a = shorten(app, slug, a, &body, limit, &k.display_name(), ch).await.map_err(|e| bad(format!("\"{title}\" is {n} characters and {} takes {limit}, and I could not shorten it: {e:#}", k.display_name())))?;
    let asks = a.target.is_none();
    Ok((a, asks))
}

/// What a publish says when the shortened text waits for the user's yes.
fn shortened_asks(a: &Asset, ch: &Channel) -> ApiError {
    bad(format!("\"{}\" was too long for {}, so I shortened it; {} asks before anything goes out, so the new text waits for the user's yes on its card (or on To do)", a.title, ch.label, ch.label))
}

const SHORTEN: &str = "You shorten a social media post so it fits a network's character limit. Keep what it says, its voice, its line breaks, its first line and any link or call to action at the end; drop the least important words or clause. Never add anything, never add hashtags or emoji. Answer with the shortened post only, no quotes, no comment.";

async fn shorten(app: &App, slug: &str, mut a: Asset, body: &str, limit: usize, network: &str, ch: &Channel) -> anyhow::Result<Asset> {
    let target = limit.saturating_sub(SHORTEN_MARGIN).max(limit / 2);
    let from = body.trim().chars().count();
    let (rules, _) = kiqr_core::memory::load(&app.store, slug).map(|m| m.lines()).unwrap_or_default();
    let spec = app.config.read().await.models.chat.clone();
    let (prov, model) = kiqr_core::provider::resolve(&spec)?;
    let mut text = body.trim().to_string();
    for _ in 0..3 {
        let prompt = format!(
            "Network: {network}. Limit: {limit} characters; aim for at most {target}. This one is {} characters: cut at least {} characters, a whole sentence or clause if that is what it takes.{}\n\nThe post:\n{text}",
            text.chars().count(),
            text.chars().count().saturating_sub(target),
            if rules.is_empty() { String::new() } else { format!("\nThe owner's rules:\n{}", rules.join("\n")) }
        );
        let out = prov.complete(&kiqr_core::provider::Request { system: SHORTEN.into(), prompt, model: model.clone(), json: false, max_tokens: 800 }).await?;
        let out = out.trim().trim_matches('"').trim().to_string();
        if !out.is_empty() {
            text = out;
        }
        if text.chars().count() <= limit {
            break;
        }
    }
    let to = text.chars().count();
    if to > limit || to == 0 {
        anyhow::bail!("the shortest I got was {to} characters");
    }
    let name = assets::body_file(&a).map(str::to_string).unwrap_or_else(|| format!("{}.md", a.kind));
    let kept = a.source.get("shortened").and_then(|s| s["original"].as_str()).map(str::to_string).unwrap_or_else(|| body.trim().to_string());
    let auto = ch.publish == Mode::Auto;
    let mut note = json!({ "at": Utc::now(), "network": network, "from": from, "to": to, "original": kept });
    if !auto {
        // Nobody said yes to this text: back to Draft, remembering where and when it was going.
        note["channel"] = json!(a.target.take());
        note["publish_at"] = json!(a.publish_at.take());
    }
    a.source.insert("shortened".into(), note);
    a.publish_error = None;
    let a = assets::save(&app.store, slug, &a)?;
    let a = assets::write_file(&app.store, slug, &a.kind, &a.id, "body", &name, (text + "\n").as_bytes())?;
    if let Err(e) = app.store.commit(&format!("[auto] assets({slug}): shortened {} to fit {network}", a.title)).await {
        tracing::warn!(%slug, error = %e, "could not commit the shortened post");
    }
    crate::api::emit_saved(app, slug, &a, false);
    let then = if auto {
        format!("{} lets me publish on my own, so it stays approved", ch.label)
    } else {
        format!("{} asks before anything goes out, so it needs your yes again", ch.label)
    };
    app.bus.emit(Some(slug), EventKind::SeemoSays { text: format!("\"{}\" was {from} characters and {network} takes {limit}, so I shortened it to {to}. {then}; the original is on its card.", a.title) });
    Ok(a)
}

/// The sweep: approved posts that do not fit where they go are shortened
/// before their time comes. Each is tried at most every six hours.
pub async fn heal_due(app: &App, slug: &str) {
    static TRIED: std::sync::LazyLock<tokio::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>> = std::sync::LazyLock::new(Default::default);
    let Ok(list) = assets::list(&app.store, slug) else { return };
    for a in list.into_iter().filter(|a| matches!(a.status, Status::Ready | Status::Scheduled) && a.publications.is_empty() && !a.archived && a.publish_attempt.is_none()) {
        let Some(t) = a.target.clone() else { continue };
        let Ok(ch) = Channel::load(&app.store, slug, &t) else { continue };
        let Ok((_, k)) = publisher_of(app, slug, &ch).await else { continue };
        let Some(limit) = k.text_limit.map(|l| l as usize) else { continue };
        let Ok((body, _)) = asset_parts(app, slug, &a) else { continue };
        if body.trim().chars().count() <= limit {
            continue;
        }
        let key = format!("{slug}/{}/{}", a.kind, a.id);
        {
            let mut tried = TRIED.lock().await;
            if tried.get(&key).is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(6 * 3600)) {
                continue;
            }
            tried.insert(key.clone(), std::time::Instant::now());
        }
        let job = format!("publish:{key}");
        if !app.jobs.write().await.insert(job.clone()) {
            continue;
        }
        if let Err(e) = shorten(app, slug, a, &body, limit, &k.display_name(), &ch).await {
            tracing::warn!(%slug, asset = %key, error = %e, "could not shorten a post that is too long");
        }
        app.jobs.write().await.remove(&job);
    }
}

/// What Seemo saves for a network must fit it: refused with the numbers, so
/// it rewrites in the same turn. By the asset's tags (its network first).
pub(crate) async fn check_length(app: &App, slug: &str, tags: &[String], text: &str) -> Result<(), ApiError> {
    let kinds = crate::networks::installed(app, slug).await;
    let Some(k) = tags.iter().find_map(|t| kinds.iter().find(|k| k.kind.eq_ignore_ascii_case(t))) else { return Ok(()) };
    let Some(limit) = k.text_limit.map(|l| l as usize) else { return Ok(()) };
    let n = text.trim().chars().count();
    if n > limit {
        return Err(bad(format!("that is {n} characters and {} takes {limit}: write it shorter (aim for {} or less, links count long) and save again", k.display_name(), limit.saturating_sub(SHORTEN_MARGIN))));
    }
    Ok(())
}

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
    if let Some(t) = &a.publish_attempt {
        return Err(ApiError(StatusCode::CONFLICT, format!("\"{}\" may have gone out already ({}): the last try clicked send but never saw it arrive. The user checks the channel and says on the asset whether it is there or to try again", a.title, t.at.format("%Y-%m-%d %H:%M UTC"))));
    }
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
        if a.target.is_none() {
            return Err(shortened_asks(&a, &entry));
        }
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
    let (a, asks) = fit(app, slug, a, &k, &entry).await?;
    if asks {
        return Err(shortened_asks(&a, &entry));
    }
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
    // In the browser a publish takes a minute or two: it goes on after the answer, and Seemo says how it went.
    if provide.hand.is_some() && by != "schedule" {
        let key = format!("publish:{slug}/{}/{}", a.kind, a.id);
        if !app.jobs.write().await.insert(key.clone()) {
            return Err(ApiError(StatusCode::CONFLICT, format!("\"{}\" is being published already", a.title)));
        }
        let (title_of, entry_id, entry_label) = (a.title.clone(), entry.id.clone(), entry.label.clone());
        let (app2, slug2, title) = (app.clone(), slug.to_string(), a.title.clone());
        let (asset_ref, label) = ((a.kind.clone(), a.id.clone()), entry.label.clone());
        tokio::spawn(async move {
            let r = send_out(&app2, &slug2, a, &entry, &m, &k, &provide, &args, &cid, &where_, now, run).await;
            app2.jobs.write().await.remove(&key);
            match r {
                Ok(v) => {
                    let link = v["url"].as_str().map(|u| format!(": {u}")).unwrap_or_default();
                    app2.bus.emit(Some(&slug2), EventKind::SeemoSays { text: format!("\"{title}\" is up on {label}{link}") });
                }
                Err(ApiError(_, msg)) => failed(&app2, &slug2, &asset_ref.0, &asset_ref.1, &title, &msg, false).await,
            }
        });
        return Ok(json!({
            "publishing": true,
            "asset": { "type": kind, "id": id, "title": title_of },
            "target": entry_id,
            "target_label": entry_label,
            "note": format!("Publishing to {entry_label} in the browser now; it takes a minute or two. The result comes in the chat on its own: do not publish it again or check on it."),
        }));
    }
    send_out(app, slug, a, &entry, &m, &k, &provide, &args, &cid, &where_, now, run).await
}

/// Put it out through the channel's publisher and record it.
#[allow(clippy::too_many_arguments)]
async fn send_out(app: &App, slug: &str, a: Asset, entry: &Channel, m: &Manifest, k: &Kind, provide: &kiqr_core::extensions::Provide, args: &Value, cid: &str, where_: &str, now: DateTime<Utc>, run: Option<String>) -> Result<Value, ApiError> {
    let out = crate::ext::execute(app, slug, m, provide, None, Some(&k.publish), args, Some(cid).filter(|c| !c.is_empty())).await.map_err(|e| bad(format!("{e:#}")))?;
    let url = out.text.clone().filter(|t| t.starts_with("http")).or_else(|| out.json.get("url").and_then(|u| u.as_str()).map(str::to_string)).or_else(|| out.json.get("link").and_then(|u| u.as_str()).map(str::to_string));
    let from = out.json.get("from").and_then(|f| f.as_str()).map(str::to_string);
    let detail = out.text.clone().unwrap_or_else(|| kiqr_core::profile::truncate_chars(&out.json.to_string(), 200));
    let a = record(app, slug, a, entry, Publication { channel: entry.id.clone(), credential: cid.to_string(), ability: m.id.clone(), from, to: None, url: url.clone(), at: now, run }, where_).await?;
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
    a.publish_attempt = None;
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
    if let Ok((_, k)) = publisher_of(app, slug, e).await {
        let (fitted, asks) = fit(app, slug, a, &k, e).await?;
        if asks {
            // The yes was for the longer text; the shortened one waits for its own.
            return Ok(fitted);
        }
        a = fitted;
    }
    if let Some(s) = a.source.get_mut("shortened").and_then(|s| s.as_object_mut()) {
        s.remove("channel");
        s.remove("publish_at");
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
    // A post shortened back to Draft keeps the time it had, if that is still ahead.
    let was = a.source.get("shortened").and_then(|s| s["publish_at"].as_str()).and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Utc));
    let at = a.publish_at.or(was).filter(|t| *t > Utc::now());
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
    // One that may have gone out already waits for a person, whatever its time says.
    for a in list.into_iter().filter(|a| a.status == Status::Scheduled && a.publish_at.is_some_and(|t| t <= now) && a.publish_attempt.is_none()) {
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
            failed(app, slug, &a.kind, &a.id, &a.title, &msg, true).await;
        }
        app.jobs.write().await.remove(&key);
    }
}

/// A publish that did not happen: the reason stays on the asset and Seemo says so. When its last click
/// was made, it may have gone out after all: the asset says that instead, and waits for a person.
async fn failed(app: &App, slug: &str, kind: &str, id: &str, title: &str, msg: &str, scheduled: bool) {
    tracing::warn!(slug, asset = %format!("{kind}/{id}"), error = %msg, scheduled, "a publish failed");
    let mut maybe = false;
    if let Ok(mut x) = assets::load(&app.store, slug, kind, id) {
        x.publish_at = None;
        match x.publish_attempt.as_mut() {
            Some(t) => {
                maybe = true;
                t.note = msg.to_string();
                x.publish_error = None;
            }
            None => x.publish_error = Some(msg.to_string()),
        }
        if let Ok(x) = assets::save(&app.store, slug, &x) {
            crate::api::emit_saved(app, slug, &x, false);
        }
        let _ = app.store.commit(&format!("[auto] publish({slug}): {title} did not go out")).await;
    }
    let when = if scheduled { " as scheduled" } else { "" };
    let text = if maybe {
        format!("I am not sure \"{title}\" went out{when}: I clicked send, but never saw it arrive ({msg}). Please look on the channel and tell me on the asset whether it is there; I will not send it again until you do.")
    } else {
        format!("I could not publish \"{title}\"{when}: {msg}")
    };
    app.bus.emit(Some(slug), EventKind::SeemoSays { text });
}

#[derive(Deserialize)]
struct AttemptBody {
    /// It is there (a link, if the user has one), or it is not and may be tried again.
    went_out: bool,
    #[serde(default)]
    url: Option<String>,
}

/// `POST …/assets/{kind}/{id}/attempt`: the user says whether a publish that may have gone out did.
async fn attempt_route(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>, Json(b): Json<AttemptBody>) -> ApiResult<Value> {
    let mut a = assets::load(&app.store, &slug, &kind, &id)?;
    let Some(t) = a.publish_attempt.clone() else {
        return Err(bad("nothing waits to be confirmed on this asset"));
    };
    if b.went_out {
        let entry = Channel::load(&app.store, &slug, &t.channel).map_err(|_| bad("the channel it went to is gone"))?;
        let (m, _) = publisher_of(&app, &slug, &entry).await?;
        let url = b.url.filter(|u| u.starts_with("http"));
        let a = record(&app, &slug, a, &entry, Publication { channel: entry.id.clone(), credential: entry.publisher.clone(), ability: m.id.clone(), from: None, to: None, url, at: t.at, run: None }, &entry.label).await?;
        return Ok(Json(json!({ "asset": a })));
    }
    a.publish_attempt = None;
    let a = assets::save(&app.store, &slug, &a)?;
    crate::api::emit_saved(&app, &slug, &a, false);
    app.store.commit(&format!("assets({slug}): {} did not go out; it may be published again", a.title)).await?;
    Ok(Json(json!({ "asset": a })))
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
