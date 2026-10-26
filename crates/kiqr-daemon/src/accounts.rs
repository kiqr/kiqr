//! Connections over HTTP: a project's accounts of its installed abilities
//! (two WordPress sites, two analytics properties), each with its own keys.
//! A connection whose ability publishes is a channel; `publish.rs` keeps the
//! `/channels` routes over the same records. The user sees "accounts" and
//! "channels"; "connection" stays in the code.

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use kiqr_core::connections::{self, Connection};
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{Manifest, Transport};
use kiqr_core::project::Project;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/connections", get(list).post(create_route))
        .route("/api/projects/{slug}/connections/{id}", get(get_one).put(update_route).delete(delete_route))
        .route("/api/projects/{slug}/connections/{id}/secrets/{name}", put(put_secret))
        .route("/api/projects/{slug}/connections/{id}/default", post(make_default))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

/// The installed manifest of an ability, as this project sees it.
pub async fn manifest(app: &App, slug: &str, ability: &str) -> Option<Manifest> {
    let reg = app.registry.read().await;
    reg.entry(Some(slug), ability).and_then(|e| e.manifest.clone()).filter(|_| reg.entry(Some(slug), ability).is_some_and(|e| e.error.is_none()))
}

/// Has every key it needs, and the program it runs is there.
pub fn ready(app: &App, slug: &str, c: &Connection, m: &Manifest) -> bool {
    c.missing(app.store.home(), slug, m).is_empty() && (m.transport != Transport::Mcp || crate::ext::on_path(&m.command))
}

/// One connection for the UI and for Seemo: never its values, only which are set.
pub async fn view(app: &App, slug: &str, c: &Connection) -> Value {
    let m = manifest(app, slug, &c.ability).await;
    let keys = c.keys(app.store.home(), slug);
    let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
    v["channel"] = json!(c.is_channel());
    v["default"] = json!(c.default);
    v["destination_text"] = json!(c.destination_text());
    match &m {
        Some(m) => {
            v["ability_name"] = json!(m.name);
            // Older UI code calls it the connection.
            v["connection"] = json!(m.id);
            v["connection_name"] = json!(m.name);
            v["ready"] = json!(ready(app, slug, c, m));
            v["missing"] = json!(c.missing(app.store.home(), slug, m));
            v["auth"] = json!(m.auth.iter().map(|a| json!({ "env": a.env, "label": a.label, "help": a.help, "optional": a.optional, "options": a.options, "placeholder": a.placeholder, "set": keys.contains_key(&a.env) })).collect::<Vec<_>>());
            v["connect"] = kiqr_core::connect::view(m.connect.as_ref());
            if let Some(k) = m.channels.iter().find(|k| k.kind == c.kind) {
                v["kind_label"] = json!(k.label);
                v["kind_name"] = json!(k.display_name());
                v["icon"] = json!(k.icon);
            }
        }
        None => {
            v["ability_name"] = json!(c.ability);
            v["connection_name"] = json!(c.ability);
            v["ready"] = json!(false);
            v["missing"] = json!([]);
            v["auth"] = json!([]);
            v["error"] = json!("the ability is not installed");
        }
    }
    v
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct NewConnection {
    pub ability: String,
    pub label: String,
    /// Set for a channel: one of the kinds the ability publishes.
    pub kind: String,
    /// A channel's destination, already checked (`publish::channel_preview`).
    pub destination: BTreeMap<String, Value>,
    /// Keys to save right away, by name.
    pub secrets: BTreeMap<String, String>,
    /// Another connection of the same ability whose keys this one starts with (the same mail account).
    pub same_as: Option<String>,
}

/// Add a connection: its record, then its keys. The first of an ability is its default.
pub async fn create(app: &App, slug: &str, b: NewConnection, by: &str) -> Result<Connection, ApiError> {
    let _ = Project::load(&app.store, slug)?;
    let m = manifest(app, slug, &b.ability).await.ok_or_else(|| bad(format!("'{}' is not installed", b.ability)))?;
    let kind = b.kind.trim().to_ascii_lowercase();
    if !kind.is_empty() && !m.channels.iter().any(|k| k.kind == kind) {
        return Err(bad(format!("{} does not publish '{kind}'", m.name)));
    }
    let same = match b.same_as.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(Connection::find(&app.store, slug, &m.id, s).ok_or_else(|| bad(format!("{} has no account '{s}' here to share", m.name)))?),
        None => None,
    };
    let siblings = Connection::of(&app.store, slug, &m.id);
    let base = b.label.trim().to_string();
    let base = if base.is_empty() { m.name.clone() } else { base };
    // Labels name accounts in Seemo's tools, so they stay unique per ability.
    let mut label = base.clone();
    let mut n = 2;
    while siblings.iter().any(|c| c.label.eq_ignore_ascii_case(&label)) {
        label = format!("{base} {n}");
        n += 1;
    }
    let c = Connection {
        id: connections::new_id(&app.store, slug, &label),
        label,
        ability: m.id.clone(),
        default: siblings.is_empty(),
        created_by: by.to_string(),
        kind,
        destination: b.destination,
        ..Default::default()
    };
    c.save(&app.store, slug)?;
    let home = app.store.home();
    if let Some(s) = &same {
        for (k, v) in s.keys(home, slug) {
            kiqr_core::secrets::write_env(&c.env_file(home, slug), &k, &v).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }
    for (name, value) in b.secrets.iter().filter(|(_, v)| !v.trim().is_empty()) {
        if let Err(e) = crate::ext::set_connection_secret(app, slug, &c.id, name, value).await {
            let _ = Connection::delete(&app.store, slug, &c.id);
            return Err(e);
        }
    }
    let what = if c.is_channel() { "channel" } else { "account" };
    app.store.commit(&format!("{what}({slug}): add {} ({})", c.label, m.name)).await?;
    app.bus.emit(Some(slug), EventKind::ConnectionSaved { connection: c.id.clone(), label: c.label.clone(), ability: c.ability.clone(), kind: c.kind.clone() });
    Ok(c)
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Patch {
    pub label: Option<String>,
    pub destination: Option<serde_json::Map<String, Value>>,
    /// false lets Seemo publish through this channel without a card.
    pub asks_first: Option<bool>,
}

pub async fn update(app: &App, slug: &str, id: &str, p: Patch) -> Result<Connection, ApiError> {
    let mut c = Connection::load(&app.store, slug, id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    if let Some(l) = p.label.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        if Connection::of(&app.store, slug, &c.ability).iter().any(|o| o.id != c.id && o.label.eq_ignore_ascii_case(&l)) {
            return Err(bad(format!("another account is already called '{l}'")));
        }
        c.label = l;
    }
    if let Some(d) = p.destination {
        if !c.is_channel() {
            return Err(bad("only a channel has a destination"));
        }
        let checked = crate::publish::channel_preview(app, slug, &crate::publish::NewChannel { label: c.label.clone(), kind: c.kind.clone(), connection: Some(c.ability.clone()), destination: d, ..Default::default() }).await?;
        c.destination = checked["destination"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    }
    if let Some(a) = p.asks_first {
        c.asks_first = a;
    }
    c.save(&app.store, slug)?;
    app.store.commit(&format!("{}({slug}): {}", if c.is_channel() { "channel" } else { "account" }, c.label)).await?;
    app.bus.emit(Some(slug), EventKind::ConnectionSaved { connection: c.id.clone(), label: c.label.clone(), ability: c.ability.clone(), kind: c.kind.clone() });
    Ok(c)
}

pub async fn delete(app: &App, slug: &str, id: &str) -> Result<(), ApiError> {
    let c = Connection::delete(&app.store, slug, id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    app.mcp.stop(&format!("{}#{}", c.ability, c.id)).await;
    app.store.commit(&format!("{}({slug}): remove {}", if c.is_channel() { "channel" } else { "account" }, c.label)).await?;
    app.bus.emit(Some(slug), EventKind::ConnectionDeleted { connection: c.id, ability: c.ability });
    Ok(())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ListQuery {
    ability: Option<String>,
}

async fn list(State(app): State<App>, Path(slug): Path<String>, Query(q): Query<ListQuery>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let mut out = Vec::new();
    for c in Connection::list(&app.store, &slug)?.iter().filter(|c| q.ability.as_deref().is_none_or(|a| c.ability == a)) {
        out.push(view(&app, &slug, c).await);
    }
    Ok(Json(json!({ "connections": out })))
}

async fn create_route(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<NewConnection>) -> Result<(StatusCode, Json<Value>), ApiError> {
    if !b.kind.trim().is_empty() {
        return Err(bad("a channel is added under /channels, where its destination is checked"));
    }
    let c = create(&app, &slug, NewConnection { destination: BTreeMap::new(), ..b }, "user").await?;
    Ok((StatusCode::CREATED, Json(view(&app, &slug, &c).await)))
}

async fn get_one(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let c = Connection::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    Ok(Json(view(&app, &slug, &c).await))
}

async fn update_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(p): Json<Patch>) -> ApiResult<Value> {
    let c = update(&app, &slug, &id, p).await?;
    Ok(Json(view(&app, &slug, &c).await))
}

async fn delete_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> Result<StatusCode, ApiError> {
    delete(&app, &slug, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SecretBody {
    #[serde(default)]
    value: String,
}

async fn put_secret(State(app): State<App>, Path((slug, id, name)): Path<(String, String, String)>, Json(b): Json<SecretBody>) -> ApiResult<Value> {
    let set = crate::ext::set_connection_secret(&app, &slug, &id, &name, &b.value).await?;
    Ok(Json(json!({ "name": name, "set": set, "connection": id })))
}

async fn make_default(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    Connection::make_default(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    let c = Connection::load(&app.store, &slug, &id)?;
    app.store.commit(&format!("account({slug}): {} is the default for {}", c.label, c.ability)).await?;
    app.bus.emit(Some(&slug), EventKind::ConnectionSaved { connection: c.id.clone(), label: c.label.clone(), ability: c.ability.clone(), kind: c.kind.clone() });
    Ok(Json(view(&app, &slug, &c).await))
}
