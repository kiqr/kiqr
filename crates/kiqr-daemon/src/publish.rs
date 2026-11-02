//! Publishing: content goes out through an account, to a place in it when
//! the account has places (a subreddit, a branch, a list of people). Here:
//! the approvals behind `publish_asset`, `send_email` and `create_channel`
//! (a place), the user's own approve / schedule / publish / unpublish on an
//! asset, the scheduled publishes the daemon does, and sending to a list
//! (one message per person). A publish asks the user first unless the
//! account lets Seemo publish on its own; this module does the sending once
//! that is settled. `/channels` stays as an alias over accounts for one release.

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
use kiqr_core::connections::{Allow, Place};
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

/// Every (ability, kind) pair on in this project: what its accounts can publish as.
pub(crate) async fn kinds(app: &App, slug: &str) -> Vec<(Manifest, ChannelKind)> {
    kinds_where(app, slug, true).await
}

/// Every kind the installed abilities publish to, the ones switched off for the project included:
/// what an account may be created as (connecting switches the ability back on).
pub(crate) async fn all_kinds(app: &App, slug: &str) -> Vec<(Manifest, ChannelKind)> {
    kinds_where(app, slug, false).await
}

async fn kinds_where(app: &App, slug: &str, only_on: bool) -> Vec<(Manifest, ChannelKind)> {
    let reg = app.registry.read().await;
    let off = if only_on { crate::ext::project_off(app, slug) } else { vec![] };
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

/// The ability that publishes to a channel here (`connections::pick_publisher`),
/// with its kind. Several abilities of the channel's service may act on it;
/// one publishes.
pub(crate) async fn publisher(app: &App, slug: &str, c: &Channel) -> Result<(Manifest, ChannelKind), ApiError> {
    let on = kinds(app, slug).await;
    kiqr_core::connections::pick_publisher(c, &on).cloned().ok_or_else(|| bad(format!("no ability publishes to the channel '{}' here; switch one on under Setup → Abilities", c.label)))
}

/// What the publisher sets itself on every publish; never an option.
const PUBLISH_ARGS: &[&str] = &["title", "subject", "text", "html", "summary", "tags", "asset", "note", "files", "date", "datetime", "image", "to"];

/// A kind whose place has a required field: nothing goes out until the
/// account has a place.
pub(crate) fn needs_place(_m: &Manifest, k: &ChannelKind) -> bool {
    k.place.values().any(|spec| !spec["optional"].as_bool().unwrap_or(false))
}

/// Publishes through the `email` contract: its places are lists of people,
/// and a publish is one message per person.
pub(crate) fn kind_takes_lists(k: &ChannelKind) -> bool {
    k.publish == "email"
}

pub(crate) async fn takes_lists(app: &App, slug: &str, c: &Channel) -> bool {
    kinds(app, slug).await.iter().any(|(m, k)| m.service() == c.service() && k.kind == c.kind && kind_takes_lists(k))
}

/// Check a place's fields against a kind the project's abilities declare
/// (`connection` narrows to one ability). Returns the ability, the kind and
/// the fields as they will be saved.
pub async fn place_check(app: &App, slug: &str, kind: &str, connection: Option<&str>, given: &BTreeMap<String, Value>) -> Result<(Manifest, ChannelKind, BTreeMap<String, Value>), ApiError> {
    let kind = kind.trim().to_ascii_lowercase();
    if kind.is_empty() {
        return Err(bad("a place needs a kind (one an ability declares)"));
    }
    let all = kinds(app, slug).await;
    // An ability that publishes the kind first; one that only reads or replies can still have accounts of it.
    let fits = |(m, k): &&(Manifest, ChannelKind)| k.kind == kind && connection.is_none_or(|c| c == m.id || c == m.service());
    let Some((m, k)) = all.iter().filter(fits).find(|(_, k)| k.publishes()).or_else(|| all.iter().find(fits)).map(|x| (x.0.clone(), x.1.clone())) else {
        let have: Vec<String> = all.iter().map(|(_, k)| k.kind.clone()).collect();
        return Err(bad(if have.is_empty() { "nothing can publish yet; connect an account of something that publishes".to_string() } else { format!("nothing publishes '{kind}'; available: {}", have.join(", ")) }));
    };
    let mut out = BTreeMap::new();
    for (field, spec) in &k.place {
        let required = !spec["optional"].as_bool().unwrap_or(false);
        match given.get(field).and_then(|v| if v.is_string() { v.as_str().map(|s| s.trim().to_string()) } else { Some(v.to_string()) }).filter(|s| !s.is_empty()) {
            Some(v) => {
                if let Some(allowed) = spec["enum"].as_array().filter(|a| !a.is_empty()) {
                    if !allowed.iter().any(|a| a.as_str() == Some(v.as_str())) {
                        return Err(bad(format!("'{}' is one of {}", spec["label"].as_str().unwrap_or(field), allowed.iter().filter_map(|a| a.as_str()).collect::<Vec<_>>().join(", "))));
                    }
                }
                out.insert(field.clone(), Value::String(v));
            }
            None if required && !given.is_empty() => return Err(bad(format!("the place needs '{}' ({})", field, spec["label"].as_str().unwrap_or(field)))),
            None => {}
        }
    }
    Ok((m, k, out))
}

/// The choices a kind's publish tool takes per publish: the properties of its
/// input beyond what the publisher fills in and the channel's destination.
pub(crate) fn publish_options(m: &Manifest, k: &ChannelKind) -> serde_json::Map<String, Value> {
    let input = m.tools.iter().find(|t| t.name == k.publish).map(|t| t.input.clone()).unwrap_or(Value::Null);
    input["properties"].as_object().map(|o| o.iter().filter(|(n, _)| !PUBLISH_ARGS.contains(&n.as_str()) && !k.place.contains_key(*n) && n.as_str() != "connection" && n.as_str() != "place").map(|(n, v)| (n.clone(), v.clone())).collect()).unwrap_or_default()
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
    // An option the tool requires is prepared for each publish (the files a site is built from, which Seemo writes).
    let input = m.tools.iter().find(|t| t.name == k.publish).map(|t| &t.input);
    for n in input.and_then(|i| i["required"].as_array()).into_iter().flatten().filter_map(Value::as_str) {
        if offered.contains_key(n) && out.get(n).is_none_or(|v| v.is_null() || v == "" || v.as_array().is_some_and(Vec::is_empty)) {
            let what = offered[n]["description"].as_str().map(|d| format!(" ({d})")).unwrap_or_default();
            return Err(bad(format!("{} needs '{n}' for each publish{what}; ask Seemo to prepare it", k.label)));
        }
    }
    Ok(out)
}

/// A kind a new channel could use. Each channel brings its own account: the
/// keys it asks for, or those of another channel of the same ability (`accounts`).
fn kind_view(app: &App, slug: &str, m: &Manifest, k: &ChannelKind) -> Value {
    let accounts: Vec<Value> = kiqr_core::connections::Connection::of(&app.store, slug, m.service()).iter().filter(|c| crate::accounts::ready(app, slug, c, m)).map(|c| json!({ "id": c.id, "label": c.label })).collect();
    json!({
        "kind": k.kind,
        "label": k.label,
        "name": k.display_name(),
        "icon": k.icon,
        "connection": m.id,
        "connection_name": m.name,
        "ready": true,
        "place": k.place,
        "needs_place": needs_place(m, k),
        "takes_lists": kind_takes_lists(k),
        "auth": m.auth.iter().map(|a| json!({ "env": a.env, "label": a.label, "help": a.help, "optional": a.optional, "options": a.options, "placeholder": a.placeholder, "set": Value::Null })).collect::<Vec<_>>(),
        "connect": kiqr_core::connect::view(m.connect.as_ref()),
        "accounts": accounts,
        "options": publish_options(m, k),
        // Inputs the publish tool requires that no one picks from a list (the files a site is built from): Seemo prepares them, so a plain Publish cannot.
        "prepared": prepared_inputs(m, k),
        "learn": crate::accounts::learn_view(m),
    })
}

/// The publish tool's required inputs that are offered per publish but have no fixed values: what Seemo writes for each publish (a site's source files).
pub(crate) fn prepared_inputs(m: &Manifest, k: &ChannelKind) -> Vec<String> {
    let offered = publish_options(m, k);
    m.tools.iter().find(|t| t.name == k.publish).and_then(|t| t.input["required"].as_array()).into_iter().flatten().filter_map(Value::as_str).filter(|n| offered.get(*n).is_some_and(|s| s["enum"].as_array().is_none_or(Vec::is_empty))).map(str::to_string).collect()
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
    // One entry per service and kind: several abilities may declare it, and the one that publishes speaks for it.
    let all = kinds(&app, &slug).await;
    let mut kinds: Vec<Value> = Vec::new();
    let mut seen: Vec<(String, String)> = Vec::new();
    for (m, k) in all.iter().filter(|(_, k)| k.publishes()).chain(all.iter().filter(|(_, k)| !k.publishes())) {
        let key = (m.service().to_string(), k.kind.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let mut v = kind_view(&app, &slug, m, k);
        v["used_by"] = json!(all.iter().filter(|(o, ok)| o.service() == m.service() && ok.kind == k.kind).map(|(o, ok)| json!({ "id": o.id, "name": o.name, "does": ok.roles() })).collect::<Vec<_>>());
        kinds.push(v);
    }
    Ok(Json(json!({ "channels": list, "kinds": kinds })))
}

/// What Seemo (or the UI) proposes with `create_channel`: a place to publish
/// to. On an account the project has (`account`): a new place in it, or a
/// list of people (`list`) on a mail account. Without one: a new account of
/// the kind, with the place as its first; its keys come from the user later.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct NewChannel {
    pub label: String,
    pub kind: String,
    /// The ability to use when several publish the kind.
    pub connection: Option<String>,
    /// An account the project has: the place is added to it.
    pub account: String,
    /// The place's fields (older callers say `destination`).
    #[serde(alias = "destination")]
    pub place: serde_json::Map<String, Value>,
    /// A list of people (by id or name) as the place, on a mail account.
    pub list: String,
    /// A new account starts with the keys of this one of the same service.
    pub same_as: Option<String>,
    /// A new account's own keys, by name (from the user, never from Seemo).
    pub secrets: BTreeMap<String, String>,
}

/// Check a proposal against the kinds and accounts available; returns what will be saved.
pub async fn channel_preview(app: &App, slug: &str, b: &NewChannel) -> Result<Value, ApiError> {
    let given: BTreeMap<String, Value> = b.place.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    // Onto an account the project has.
    let account = match b.account.trim() {
        "" => None,
        a => Some(Channel::list(&app.store, slug)?.into_iter().find(|c| c.id == a || c.label.eq_ignore_ascii_case(a)).ok_or_else(|| bad(format!("no account '{a}' here; list_channels shows them")))?),
    };
    let kind = match (&account, b.kind.trim()) {
        (Some(c), "") => c.kind.clone(),
        (_, k) => k.to_ascii_lowercase(),
    };
    if let Some(c) = &account {
        if !c.is_channel() || (!kind.is_empty() && kind != c.kind) {
            return Err(bad(format!("'{}' does not publish{}", c.label, if kind.is_empty() { String::new() } else { format!(" as '{kind}'") })));
        }
    }
    let list = match b.list.trim() {
        "" => None,
        l => Some(kiqr_core::lists::List::find(&app.store, slug, l).ok_or_else(|| bad(format!("no list '{l}'; the lists are under Audience, or propose one with create_list")))?),
    };
    let (m, k, fields) = place_check(app, slug, &kind, b.connection.as_deref().or(account.as_ref().map(|c| c.ability.as_str())), &given).await?;
    if list.is_some() && !kind_takes_lists(&k) {
        return Err(bad(format!("'{}' does not send to lists of people; only a mail account does", k.label)));
    }
    if needs_place(&m, &k) && fields.is_empty() && list.is_none() {
        let need: Vec<&str> = k.place.iter().filter(|(_, s)| !s["optional"].as_bool().unwrap_or(false)).map(|(n, _)| n.as_str()).collect();
        return Err(bad(format!("a place of kind '{}' needs {}", k.kind, need.join(", "))));
    }
    let same = match b.same_as.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(Channel::find(&app.store, slug, m.service(), s).ok_or_else(|| bad(format!("{} has no account '{s}' in this project to share", m.name)))?),
        None => None,
    };
    let ready = match (&account, &same) {
        (Some(c), _) => crate::accounts::ready(app, slug, c, &m),
        (None, Some(c)) => crate::accounts::ready(app, slug, c, &m),
        (None, None) => kiqr_core::extensions::missing_keys(&m, |n| b.secrets.get(n).is_some_and(|v| !v.trim().is_empty())).is_empty(),
    };
    let place_label = match (&list, b.label.trim()) {
        (Some(l), "") => l.label.clone(),
        (_, "") => fields.values().find_map(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| k.label.clone()),
        (_, l) => l.to_string(),
    };
    Ok(json!({
        "label": if account.is_some() { place_label.clone() } else if b.label.trim().is_empty() { k.label.clone() } else { b.label.trim().to_string() },
        "place_label": place_label,
        "kind": k.kind,
        "kind_label": k.label,
        "connection": m.id,
        "connection_name": m.name,
        "account": account.as_ref().map(|c| c.id.clone()),
        "account_label": account.as_ref().map(|c| c.label.clone()),
        "list": list.as_ref().map(|l| l.id.clone()),
        "list_label": list.as_ref().map(|l| l.label.clone()),
        "same_as": same.as_ref().map(|c| c.id.clone()),
        "same_as_label": same.as_ref().map(|c| c.label.clone()),
        "ready": ready,
        "place": fields,
    }))
}

/// Save what a checked preview says: a place on an account, or a new account with its first place.
pub async fn save_channel(app: &App, slug: &str, preview: &Value, by: &str, secrets: BTreeMap<String, String>) -> Result<Value, ApiError> {
    let fields: BTreeMap<String, Value> = preview["place"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    let place_label = preview["place_label"].as_str().unwrap_or("").to_string();
    let list = preview["list"].as_str().unwrap_or("").to_string();
    if let Some(id) = preview["account"].as_str().filter(|a| !a.is_empty()) {
        let (c, p) = crate::accounts::create_place(app, slug, id, crate::accounts::PlaceBody { label: place_label, fields, default: false, list, kind: String::new() }).await?;
        let mut v = channel_view(app, slug, &c).await;
        v["place"] = json!(p);
        v["note"] = json!(format!("{} now has the place {}. Publishing there asks the user unless they let you publish on your own.", c.label, p.label));
        return Ok(v);
    }
    let new = crate::accounts::NewConnection {
        ability: preview["connection"].as_str().unwrap_or("").to_string(),
        label: preview["label"].as_str().unwrap_or("Account").to_string(),
        kind: preview["kind"].as_str().unwrap_or("").to_string(),
        place: fields,
        place_label,
        secrets,
        same_as: preview["same_as"].as_str().map(str::to_string),
        allow: BTreeMap::new(),
    };
    let c = crate::accounts::create(app, slug, new, by).await?;
    if !list.is_empty() {
        let _ = crate::accounts::create_place(app, slug, &c.id, crate::accounts::PlaceBody { list, default: true, ..Default::default() }).await?;
    }
    let c = Channel::load(&app.store, slug, &c.id)?;
    let mut v = channel_view(app, slug, &c).await;
    v["note"] = json!(if v["ready"].as_bool() == Some(true) { "The account exists. Publishing to it asks the user each time until they say otherwise." } else { "The account exists but is not connected yet: the user signs in under Setup → Accounts before anything can go out." });
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
    /// The account (older callers: the channel); empty means the one the asset was approved for.
    #[serde(alias = "account")]
    pub channel: String,
    /// The place inside it, by id or name; empty means the one the asset was approved for, else the account's default.
    pub place: String,
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
        bad(if have.is_empty() { "nothing publishes yet; the user connects an account under Setup → Accounts, or propose a place with create_channel".to_string() } else { format!("no account '{}' that publishes; there is {}", id.trim(), have.join(", ")) })
    })?;
    if !c.publishes() {
        return Err(bad(format!("Seemo may not publish on '{}'; the user can allow it on the account", c.label)));
    }
    let cv = channel_view(app, slug, &c).await;
    if cv["ready"].as_bool() != Some(true) {
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("the account '{}' is not connected yet ({} needs {}); the user signs in under Setup → Accounts", c.label, cv["connection_name"].as_str().unwrap_or(""), cv["missing"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default())));
    }
    Ok(c)
}

/// The place a publish goes to: the one named, else the asset's own, else
/// the account's default. An account whose kind needs a place, or that
/// sends to lists, must end up with one.
async fn place_of(app: &App, slug: &str, c: &Channel, a: &Asset, named: &str) -> Result<Option<Place>, ApiError> {
    let wanted = Some(named.trim()).filter(|n| !n.is_empty()).map(str::to_string).or_else(|| a.place.clone().filter(|_| a.channel.as_deref() == Some(c.id.as_str())));
    let found = c.place(wanted.as_deref()).cloned();
    if let (Some(w), None) = (&wanted, &found) {
        let have: Vec<String> = c.places.iter().map(|p| format!("{} ({})", p.id, p.label)).collect();
        return Err(bad(if have.is_empty() { format!("'{}' has no place '{w}'", c.label) } else { format!("'{}' has no place '{w}'; its places: {}", c.label, have.join(", ")) }));
    }
    if found.is_none() {
        let (m, k) = publisher(app, slug, c).await?;
        if kind_takes_lists(&k) {
            return Err(bad(format!("'{}' sends to lists of people: name a `place` (a list on the account; create_channel adds one), or use send_email with `to` for one address", c.label)));
        }
        if needs_place(&m, &k) {
            return Err(bad(format!("'{}' needs a place to publish to (a {}); the user adds one on the account, or propose it with create_channel", c.label, k.place.keys().cloned().collect::<Vec<_>>().join(", "))));
        }
    }
    Ok(found)
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
    let place = place_of(app, slug, &c, &a, &b.place).await?;
    let (body, files) = asset_parts(app, slug, &a).map_err(|e| bad(e.to_string()))?;
    let subject = place.as_ref().and_then(|p| p.fields.get("subject")).and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()).map(str::to_string).unwrap_or_else(|| a.title.clone());
    let (pm, pk) = publisher(app, slug, &c).await?;
    let options = check_options(&pm, &pk, &b.options)?;
    // To a list: how many people, so the card can say.
    let recipients = place.as_ref().and_then(|p| p.list_id()).map(|l| kiqr_core::lists::recipients(app.store.home(), slug, l).len());
    Ok(json!({
        "options": options,
        "publisher": pm.id,
        "publisher_name": pm.name,
        "asset": { "type": a.kind, "id": a.id, "title": a.title, "status": a.status },
        "channel": c.id,
        "channel_label": c.label,
        "account": c.id,
        "account_label": c.label,
        "kind": c.kind,
        "place": place.as_ref().map(|p| p.id.clone()),
        "place_label": place.as_ref().map(|p| p.label.clone()),
        "place_fields": place.as_ref().map(|p| p.fields.clone()),
        "recipients": recipients,
        "destination_text": place.as_ref().map(|p| p.label.clone()).unwrap_or_default(),
        "connection_name": cv["connection_name"],
        "subject": subject,
        "excerpt": kiqr_core::profile::truncate_chars(body.trim(), 400),
        "words": body.split_whitespace().count(),
        "files": files.iter().map(|(n, b, m)| json!({ "name": n, "bytes": b.len(), "mime": m })).collect::<Vec<_>>(),
        "at": b.at,
        "note": b.note,
        // The account lets Seemo publish on its own: the card is the record.
        "asks_first": c.allow("publish") != Allow::Auto,
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
    let place = c.place(preview["place"].as_str()).cloned();
    if let Some(at) = preview["at"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Utc)) {
        a.publish_options = preview["options"].as_object().cloned().unwrap_or_default();
        let a = set_schedule(app, slug, a, &c, place.as_ref(), Some(at)).await?;
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
    let (m, k) = publisher(app, slug, &c).await?;
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
        // For files a site builds from (frontmatter, a dated path): the day of this publish, and the asset's first image as named in `{{files}}`.
        "date": Utc::now().format("%Y-%m-%d").to_string(),
        "datetime": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "image": a.files.iter().find(|f| f.role != "body" && mime_guess::from_path(&f.path).first_or_octet_stream().type_() == "image").map(|f| f.path.clone()),
    });
    if let Some(p) = place.as_ref().filter(|p| !p.is_list()) {
        for (dk, dv) in &p.fields {
            args[dk] = dv.clone();
        }
        args["place"] = json!(p.id);
    }
    // This publish's own choices last: they were on the card the user said yes to.
    if let Some(o) = preview["options"].as_object() {
        for (n, v) in o {
            args[n] = v.clone();
        }
    }
    let where_ = place.as_ref().map(|p| format!("{} ({})", c.label, p.label)).unwrap_or_else(|| c.label.clone());
    // To a list of people: one message each, in the background; the publish is recorded when the send starts.
    if let Some(list_id) = place.as_ref().and_then(|p| p.list_id()) {
        let l = kiqr_core::lists::List::load(&app.store, slug, list_id)?;
        let started = crate::lists::send(app, slug, &c, &m, &provide, &l, &subject, &body, Some(AssetRef { kind: a.kind.clone(), id: a.id.clone() }), by).await?;
        let now = Utc::now();
        a.publications.push(Publication { connector: c.id.clone(), place: place.as_ref().map(|p| p.id.clone()), account: None, url: None, at: now, run: if by.starts_with("task:") { Some(by.to_string()) } else { None } });
        a.channel = Some(c.id.clone());
        a.place = place.as_ref().map(|p| p.id.clone());
        a.publish_at = None;
        a.publish_error = None;
        let a = assets::save(&app.store, slug, &a)?;
        crate::api::emit_saved(app, slug, &a, false);
        c.publications += 1;
        c.last_published = Some(now);
        let _ = c.save(&app.store, slug);
        app.store.commit(&format!("[auto] publish({slug}): {} → {where_}", a.title)).await?;
        app.bus.emit(Some(slug), EventKind::AssetPublished { asset_type: a.kind.clone(), id: a.id.clone(), channel: c.id.clone(), place: a.place.clone(), url: None });
        return Ok(json!({
            "published": true,
            "sending": true,
            "asset": { "type": a.kind, "id": a.id, "title": a.title },
            "channel": c.id,
            "channel_label": c.label,
            "place": a.place,
            "place_label": place.as_ref().map(|p| p.label.clone()),
            "recipients": started,
            "detail": format!("sending to {started} people"),
            "note": format!("Sending to {started} people on {}, one message each, in the background; the list page shows how far it got.", l.label),
        }));
    }
    let out = crate::ext::execute(app, slug, &m, &provide, None, Some(&k.publish), &args, Some(&c.id)).await.map_err(|e| bad(format!("{e:#}")))?;
    let url = out.text.clone().filter(|t| t.starts_with("http")).or_else(|| out.json.get("url").and_then(|u| u.as_str()).map(str::to_string)).or_else(|| out.json.get("link").and_then(|u| u.as_str()).map(str::to_string));
    let account = out.json.get("from").and_then(|f| f.as_str()).map(str::to_string);
    let detail = out.text.clone().unwrap_or_else(|| kiqr_core::profile::truncate_chars(&out.json.to_string(), 200));
    let now = Utc::now();
    a.publications.push(Publication { connector: c.id.clone(), place: place.as_ref().map(|p| p.id.clone()), account, url: url.clone(), at: now, run: if by.starts_with("task:") { Some(by.to_string()) } else { None } });
    a.channel = Some(c.id.clone());
    a.place = place.as_ref().map(|p| p.id.clone());
    a.publish_at = None;
    a.publish_error = None;
    let a = assets::save(&app.store, slug, &a)?;
    crate::api::emit_saved(app, slug, &a, false);
    c.publications += 1;
    c.last_published = Some(now);
    let _ = c.save(&app.store, slug);
    app.store.commit(&format!("[auto] publish({slug}): {} → {where_}", a.title)).await?;
    app.bus.emit(Some(slug), EventKind::AssetPublished { asset_type: a.kind.clone(), id: a.id.clone(), channel: c.id.clone(), place: a.place.clone(), url: url.clone() });
    Ok(json!({
        "published": true,
        "asset": { "type": a.kind, "id": a.id, "title": a.title },
        "channel": c.id,
        "channel_label": c.label,
        "place": a.place,
        "place_label": place.as_ref().map(|p| p.label.clone()),
        "url": url,
        "detail": detail,
        "note": "Published. The asset is marked published and the account's count went up.",
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
    #[serde(alias = "account")]
    channel: String,
    place: String,
}

#[derive(Deserialize)]
struct ScheduleBody {
    at: DateTime<Utc>,
    #[serde(default, alias = "account")]
    channel: String,
    #[serde(default)]
    place: String,
    /// This publish's choices (see `PublishBody::options`).
    #[serde(default)]
    options: serde_json::Map<String, Value>,
}

/// Approve the asset for an account (and a place in it), with or without a time. Ready, or Scheduled.
async fn set_schedule(app: &App, slug: &str, mut a: Asset, c: &Channel, place: Option<&Place>, at: Option<DateTime<Utc>>) -> Result<Asset, ApiError> {
    if a.archived {
        return Err(bad("it is archived; bring it back first"));
    }
    if !a.publications.is_empty() {
        return Err(bad("it is published already; unpublish it first"));
    }
    a.channel = Some(c.id.clone());
    a.place = place.map(|p| p.id.clone());
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
    let place = place_of(&app, &slug, &c, &a, &b.place).await?;
    let at = a.publish_at;
    Ok(Json(set_schedule(&app, &slug, a, &c, place.as_ref(), at).await?))
}

/// `POST …/schedule {at, channel?}`: publish it then, through its channel or the one named. Scheduled.
async fn schedule_route(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>, Json(b): Json<ScheduleBody>) -> ApiResult<Asset> {
    let _ = Project::load(&app.store, &slug)?;
    let a = assets::load(&app.store, &slug, &kind, &id)?;
    if b.at <= Utc::now() {
        return Err(bad("that time has passed; publish it now instead"));
    }
    let c = ready_channel(&app, &slug, channel_of(&a, &b.channel)?).await?;
    let place = place_of(&app, &slug, &c, &a, &b.place).await?;
    let mut a = a;
    a.publish_options = {
        let (m, k) = publisher(&app, &slug, &c).await?;
        check_options(&m, &k, &b.options)?
    };
    Ok(Json(set_schedule(&app, &slug, a, &c, place.as_ref(), Some(b.at)).await?))
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
        let body = PublishBody { asset: AssetRef { kind: a.kind.clone(), id: a.id.clone() }, channel: a.channel.clone().unwrap_or_default(), place: a.place.clone().unwrap_or_default(), options: a.publish_options.clone(), ..Default::default() };
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
    /// A list of people (by id or name): one message each.
    pub list: String,
    pub subject: String,
    pub text: String,
    pub asset: Option<AssetRef>,
    /// Which mail account sends it (by id or name); the default when empty.
    #[serde(alias = "connection")]
    pub from: String,
}

pub async fn email_preview(app: &App, slug: &str, b: &EmailBody) -> Result<Value, ApiError> {
    let to = b.to.trim();
    let list = match b.list.trim() {
        "" => None,
        l => Some(kiqr_core::lists::List::find(&app.store, slug, l).ok_or_else(|| bad(format!("no list '{l}'; the lists are under Audience")))?),
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
    let m = {
        let cfg = app.config.read().await.clone();
        let reg = app.registry.read().await;
        reg.provider(slug, "email", cfg.abilities.get("email").map(String::as_str), |m| crate::ext::ready_in(app, Some(slug), m)).map(|(_, m)| m.clone())
    };
    let Some(m) = m else {
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, "no mail account is connected for this project; the user connects one under Setup → Accounts".into()));
    };
    let provider = m.name.clone();
    let from = crate::ext::pick_connection(app, slug, &m, Some(b.from.as_str()).filter(|f| !f.trim().is_empty())).map_err(|e| bad(e.to_string()))?;
    if let Some(c) = &from {
        if c.allow("publish") == Allow::Off {
            return Err(bad(format!("Seemo may not send from '{}'; the user can allow it on the account", c.label)));
        }
    }
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
        "connection_name": provider,
        "from": from.as_ref().map(|c| c.id.clone()),
        "from_label": from.as_ref().map(|c| c.label.clone()),
        // A mail account the user let Seemo send from on its own: the card is the record.
        "asks_first": from.as_ref().is_none_or(|c| c.allow("publish") != Allow::Auto),
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
        reg.provider(slug, "email", cfg.abilities.get("email").map(String::as_str), |m| crate::ext::ready_in(app, Some(slug), m)).map(|(_, m)| m.clone()).ok_or_else(|| bad("no mail account is connected for this project; the user connects one under Setup → Accounts"))?
    };
    let provide = m.provides.get("email").cloned().unwrap_or_default();
    // To a list: one message per person, in the background.
    if let Some(list_id) = preview["list"].as_str().filter(|l| !l.is_empty()) {
        let l = kiqr_core::lists::List::load(&app.store, slug, list_id)?;
        let c = crate::ext::pick_connection(app, slug, &m, preview["from"].as_str()).map_err(|e| bad(e.to_string()))?.ok_or_else(|| bad("no mail account to send from"))?;
        let asset = preview["asset"].as_object().and_then(|o| Some(AssetRef { kind: o.get("type")?.as_str()?.to_string(), id: o.get("id")?.as_str()?.to_string() }));
        let n = crate::lists::send(app, slug, &c, &m, &provide, &l, &subject, &text, asset, "seemo").await?;
        app.bus.emit(Some(slug), EventKind::SeemoSays { text: format!("I am sending \"{subject}\" to the {} people on {}.", n, l.label) });
        return Ok(json!({ "sent": true, "sending": true, "list": l.id, "list_label": l.label, "recipients": n, "from": c.label, "subject": subject, "connection": m.name, "note": format!("Sending to {n} people, one message each, in the background.") }));
    }
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

