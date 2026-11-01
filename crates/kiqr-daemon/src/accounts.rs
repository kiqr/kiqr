//! Connections over HTTP: a project's accounts of its installed abilities
//! (two WordPress sites, two analytics properties), each with its own keys,
//! its permissions (what Seemo may do through it) and its places (where
//! inside it content goes). The user sees "accounts"; "connection" stays in
//! the code, and `publish.rs` keeps `/channels` as an alias over the same
//! records for one release.

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use kiqr_core::connections::{self, Allow, Connection, Place};
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
        .route("/api/projects/{slug}/connections/{id}/activity", get(activity))
        .route("/api/projects/{slug}/connections/{id}/choices/{name}", get(choices))
        .route("/api/projects/{slug}/connections/{id}/learn", post(learn_now))
        .route("/api/projects/{slug}/connections/{id}/allow", put(put_allow))
        .route("/api/projects/{slug}/connections/{id}/places", post(add_place))
        .route("/api/projects/{slug}/connections/{id}/places/{place}", put(update_place).delete(remove_place))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

/// The installed manifest of an ability, as this project sees it.
pub async fn manifest(app: &App, slug: &str, ability: &str) -> Option<Manifest> {
    let reg = app.registry.read().await;
    reg.entry(Some(slug), ability).and_then(|e| e.manifest.clone()).filter(|_| reg.entry(Some(slug), ability).is_some_and(|e| e.error.is_none()))
}

/// What the Source switch says, when an account of the ability can be a source.
pub fn learn_view(m: &Manifest) -> Value {
    json!(m.learn.as_ref().map(|l| json!({ "label": if l.label.is_empty() { "Learn from it into the profile".to_string() } else { l.label.clone() } })))
}

/// Has every key it needs, and the program it runs is there.
pub fn ready(app: &App, slug: &str, c: &Connection, m: &Manifest) -> bool {
    c.missing(app.store.home(), slug, m).is_empty() && (m.transport != Transport::Mcp || crate::ext::on_path(&m.command))
}

/// The manifest an account is read with: the ability it was made with, else
/// another installed ability of its service.
pub async fn manifest_for(app: &App, slug: &str, c: &Connection) -> Option<Manifest> {
    if let Some(m) = manifest(app, slug, &c.ability).await {
        return Some(m);
    }
    app.registry.read().await.of_service(slug, c.service()).first().map(|m| (*m).clone())
}

/// One connection for the UI and for Seemo: never its values, only which are set.
pub async fn view(app: &App, slug: &str, c: &Connection) -> Value {
    let m = manifest_for(app, slug, c).await;
    let keys = c.keys(app.store.home(), slug);
    let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
    v["channel"] = json!(c.is_channel());
    v["publishes"] = json!(c.publishes());
    v["default"] = json!(c.default);
    v["destination_text"] = json!(c.destination_text());
    // Every ability of its service that is on here uses it; on a channel, with what each does there.
    let on = crate::publish::kinds(app, slug).await;
    let mut used_by: Vec<Value> = Vec::new();
    {
        let reg = app.registry.read().await;
        let off = crate::ext::project_off(app, slug);
        for (_, o) in reg.connections(slug).into_iter().filter(|(_, o)| o.service() == c.service() && !off.contains(&o.id)) {
            let does: Vec<&str> = o.channels.iter().filter(|k| c.is_channel() && k.kind == c.kind).flat_map(|k| k.roles()).collect();
            if c.is_channel() && does.is_empty() {
                continue;
            }
            used_by.push(json!({ "id": o.id, "name": o.name, "does": does }));
        }
    }
    v["service"] = json!(c.service());
    v["used_by"] = json!(used_by);
    // A source: what learning from it reads, when its ability can teach the project.
    v["source"] = json!(c.learns());
    v["can_learn"] = json!(m.as_ref().and_then(|m| m.learn.as_ref()).is_some());
    v["learn"] = m.as_ref().map(learn_view).unwrap_or(Value::Null);
    // The kinds it could publish as, for making it publishable.
    v["can_publish"] = json!(m.as_ref().is_some_and(|m| !m.channels.is_empty()));
    // What Seemo may do here: every role its service's abilities have, with the choice made or the default.
    v["allow"] = json!(permissions(app, slug, c).await.into_iter().map(|(role, a, acts)| json!({ "role": role, "allow": a, "acts": acts })).collect::<Vec<_>>());
    // Places, a list's with how many people are on it; and whether this account sends to lists at all.
    v["places"] = json!(c.places.iter().map(|p| {
        let mut pv = json!(p);
        if let Some(l) = p.list_id() {
            pv["contacts"] = json!(kiqr_core::lists::recipients(app.store.home(), slug, l).len());
        }
        pv
    }).collect::<Vec<_>>());
    v["takes_lists"] = json!(c.is_channel() && crate::publish::takes_lists(app, slug, c).await);
    if c.is_channel() {
        let p = connections::pick_publisher(c, &on);
        v["publisher"] = json!(p.map(|(pm, _)| pm.id.clone()));
        v["publisher_name"] = json!(p.map(|(pm, _)| pm.name.clone()));
        v["publishers"] = json!(connections::publishers(c, &on).iter().map(|(pm, _)| json!({ "id": pm.id, "name": pm.name })).collect::<Vec<_>>());
    }
    match &m {
        Some(m) => {
            v["ability_name"] = json!(m.name);
            // Older UI code calls it the connection.
            v["connection"] = json!(m.id);
            v["connection_name"] = json!(m.name);
            v["ready"] = json!(ready(app, slug, c, m));
            v["missing"] = json!(c.missing(app.store.home(), slug, m));
            // `written`: a sign-in keeps it (a token), never typed; `choices`: a list read through the account.
            let written: Vec<String> = kiqr_core::connect::written_tokens(&m.connect).into_iter().filter(|w| !m.auth.iter().any(|a| &a.env == w && a.typed)).collect();
            // Signed in one way or another (a token kept, a key file set): what a key's `choices` are read with.
            v["signed_in"] = json!(m.connect.iter().any(|w| w.satisfied(&|k: &str| keys.contains_key(k))));
            v["auth"] = json!(m.auth.iter().map(|a| json!({ "env": a.env, "label": a.label, "help": a.help, "optional": a.optional, "options": a.options, "placeholder": a.placeholder, "set": keys.contains_key(&a.env), "written": written.contains(&a.env), "choices": a.choices.is_some() })).collect::<Vec<_>>());
            v["connect"] = kiqr_core::connect::view(&m.connect);
            if let Some(k) = m.channels.iter().find(|k| k.kind == c.kind) {
                v["kind_label"] = json!(k.label);
                v["kind_name"] = json!(k.display_name());
                v["icon"] = json!(k.icon);
                // The fields a place here has, and whether one is needed before anything goes out.
                v["place_fields"] = json!(k.place);
                v["needs_place"] = json!(crate::publish::needs_place(m, k));
            }
            v["service_card"] = json!(m.service_card());
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

/// The roles an account can be allowed, with what it allows now: the roles
/// the abilities of its service declare on tools and kinds, `learn` when one
/// can teach the project, `measure` when one reads numbers. Reading is
/// always there. Each with whether it acts (three choices) or not (on/off).
pub async fn permissions(app: &App, slug: &str, c: &Connection) -> Vec<(String, Allow, bool)> {
    let reg = app.registry.read().await;
    let off = crate::ext::project_off(app, slug);
    let mut roles: Vec<String> = vec!["read".into()];
    for (_, m) in reg.connections(slug).into_iter().filter(|(_, m)| m.service() == c.service() && !off.contains(&m.id)) {
        for t in &m.tools {
            if !t.role.is_empty() && !roles.contains(&t.role) {
                roles.push(t.role.clone());
            }
        }
        for k in &m.channels {
            for r in k.roles() {
                if !roles.iter().any(|x| x == r) {
                    roles.push(r.to_string());
                }
            }
        }
        if m.learn.is_some() && !roles.iter().any(|r| r == "learn") {
            roles.push("learn".into());
        }
        if !m.sources.is_empty() && !roles.iter().any(|r| r == "measure") {
            roles.push("measure".into());
        }
    }
    let order = |r: &str| connections::PERMISSIONS.iter().position(|p| *p == r).unwrap_or(99);
    roles.sort_by_key(|r| order(r));
    roles.into_iter().map(|r| { let a = c.allow(&r); let acts = Allow::acts(&r); (r, a, acts) }).collect()
}

/// What an account of this service starts with: the manifests' `allow`
/// defaults, the first to name a role winning.
pub async fn default_allow(app: &App, slug: &str, service: &str) -> BTreeMap<String, Allow> {
    let reg = app.registry.read().await;
    let mut out = BTreeMap::new();
    for m in reg.of_service(slug, service) {
        for (role, a) in &m.allow {
            out.entry(role.clone()).or_insert(*a);
        }
    }
    out
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct NewConnection {
    pub ability: String,
    pub label: String,
    /// One of the kinds the ability publishes; empty: the service's first kind that does, if any.
    pub kind: String,
    /// The first place's fields, already checked (`publish::place_check`). Older callers say `destination`.
    #[serde(alias = "destination")]
    pub place: BTreeMap<String, Value>,
    /// What to call that first place; its first value when empty.
    pub place_label: String,
    /// Keys to save right away, by name.
    pub secrets: BTreeMap<String, String>,
    /// Another connection of the same ability whose keys this one starts with (the same mail account).
    pub same_as: Option<String>,
    /// Permissions chosen at setup, over the manifests' defaults: role → auto | ask | off.
    pub allow: BTreeMap<String, String>,
}

/// Add a connection: its record, then its keys. The first of an ability is its default.
pub async fn create(app: &App, slug: &str, b: NewConnection, by: &str) -> Result<Connection, ApiError> {
    let _ = Project::load(&app.store, slug)?;
    let m = manifest(app, slug, &b.ability).await.ok_or_else(|| bad(format!("'{}' is not installed", b.ability)))?;
    let kind = b.kind.trim().to_ascii_lowercase();
    // An account of a service that publishes is publishable from the start (its kind), unless told otherwise.
    let kind = if kind.is_empty() { crate::publish::kinds(app, slug).await.into_iter().find(|(o, k)| o.service() == m.service() && k.publishes()).map(|(_, k)| k.kind).unwrap_or_default() } else { kind };
    if !kind.is_empty() && !crate::publish::kinds(app, slug).await.iter().any(|(o, k)| o.service() == m.service() && k.kind == kind) {
        return Err(bad(format!("{} does not publish '{kind}'", m.name)));
    }
    let mut allow = default_allow(app, slug, m.service()).await;
    for (role, a) in &b.allow {
        allow.insert(role.clone(), parse_allow(role, a)?);
    }
    let same = match b.same_as.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(Connection::find(&app.store, slug, m.service(), s).ok_or_else(|| bad(format!("{} has no account '{s}' here to share", m.name)))?),
        None => None,
    };
    let siblings = Connection::of(&app.store, slug, m.service());
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
        service: m.service.id.clone(),
        default: siblings.is_empty(),
        created_by: by.to_string(),
        kind,
        allow,
        ..Default::default()
    };
    let mut c = c;
    if !b.place.is_empty() {
        let label = if b.place_label.trim().is_empty() { b.place.values().find_map(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| c.label.clone()) } else { b.place_label.trim().to_string() };
        c.put_place(Place { id: String::new(), kind: c.kind.clone(), label, fields: b.place, default: true });
    }
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
    app.store.commit(&format!("account({slug}): add {} ({})", c.label, m.name)).await?;
    app.bus.emit(Some(slug), EventKind::ConnectionSaved { connection: c.id.clone(), label: c.label.clone(), ability: c.ability.clone(), kind: c.kind.clone() });
    // Learn from it now when it is ready; otherwise the scheduler does once it is signed in.
    if c.learns() && m.learn.is_some() && ready(app, slug, &c, &m) {
        let _ = crate::learn::start(app, slug, Some(c.id.clone())).await;
    }
    Ok(c)
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Patch {
    pub label: Option<String>,
    /// Permissions to change: role → auto | ask | off (on/off for roles that never act).
    pub allow: Option<BTreeMap<String, String>>,
    /// Older callers: false is `allow.publish = auto`, true is `ask`.
    pub asks_first: Option<bool>,
    /// Older callers: the Source switch, now `allow.learn`.
    pub learns: Option<bool>,
    /// The ability that publishes here, of those that can; empty goes back to the default.
    pub publisher: Option<String>,
    /// Make it publishable as a kind its service publishes (with the first `place` when the kind needs one), or empty to stop publishing.
    pub kind: Option<String>,
    #[serde(alias = "destination")]
    pub place: Option<BTreeMap<String, Value>>,
}

fn parse_allow(role: &str, a: &str) -> Result<Allow, ApiError> {
    if !connections::PERMISSIONS.contains(&role) {
        return Err(bad(format!("'{role}' is not something an account allows ({})", connections::PERMISSIONS.join(", "))));
    }
    let v = Allow::parse(a).ok_or_else(|| bad(format!("'{a}' is not auto, ask or off")))?;
    if !Allow::acts(role) && v == Allow::Ask {
        return Err(bad(format!("'{role}' never asks; it is on or off")));
    }
    Ok(v)
}

pub async fn update(app: &App, slug: &str, id: &str, p: Patch) -> Result<Connection, ApiError> {
    let mut c = Connection::load(&app.store, slug, id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    if let Some(l) = p.label.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        if Connection::of(&app.store, slug, c.service()).iter().any(|o| o.id != c.id && o.label.eq_ignore_ascii_case(&l)) {
            return Err(bad(format!("another account is already called '{l}'")));
        }
        c.label = l;
    }
    let was_learning = c.learns();
    let mut changes: BTreeMap<String, String> = p.allow.unwrap_or_default();
    if let Some(a) = p.asks_first {
        changes.insert("publish".into(), if a { "ask" } else { "auto" }.into());
    }
    if let Some(on) = p.learns {
        changes.insert("learn".into(), if on { "on" } else { "off" }.into());
    }
    let m = manifest_for(app, slug, &c).await;
    for (role, a) in &changes {
        let v = parse_allow(role, a)?;
        if role == "learn" && v != Allow::Off && m.as_ref().and_then(|m| m.learn.as_ref()).is_none() {
            return Err(bad(format!("nothing to learn from in '{}'", c.label)));
        }
        c.allow.insert(role.clone(), v);
    }
    let learn_now = c.learns() && !was_learning && m.as_ref().is_some_and(|m| m.learn.is_some() && ready(app, slug, &c, m));
    let mut place = p.place;
    if let Some(k) = p.kind.map(|k| k.trim().to_ascii_lowercase()) {
        if k.is_empty() {
            c.kind.clear();
            c.places.clear();
            c.publisher.clear();
        } else if k != c.kind {
            // Becoming publishable: the first place is checked against the kind, as for a new one.
            let fields = place.take().unwrap_or_default();
            let (_, kind, checked) = crate::publish::place_check(app, slug, &k, Some(&c.ability), &fields).await?;
            c.kind = kind.kind.clone();
            c.places.clear();
            if !checked.is_empty() {
                c.put_place(Place { id: String::new(), kind: c.kind.clone(), label: place_label(&checked, &c.label), fields: checked, default: true });
            }
        }
    }
    if let Some(fields) = place {
        // The default place's fields, checked.
        if !c.is_channel() {
            return Err(bad("only an account that publishes has places"));
        }
        let (_, _, checked) = crate::publish::place_check(app, slug, &c.kind, Some(&c.ability), &fields).await?;
        let (id, label) = c.place(None).map(|p| (p.id.clone(), p.label.clone())).unwrap_or_default();
        c.put_place(Place { id, kind: c.kind.clone(), label: if label.is_empty() { place_label(&checked, &c.label) } else { label }, fields: checked, default: true });
    }
    if let Some(id) = p.publisher.map(|s| s.trim().to_string()) {
        if !c.is_channel() {
            return Err(bad("only a channel has a publisher"));
        }
        let on = crate::publish::kinds(app, slug).await;
        let can = connections::publishers(&c, &on);
        if !id.is_empty() && !can.iter().any(|(m, _)| m.id == id) {
            let names: Vec<&str> = can.iter().map(|(m, _)| m.id.as_str()).collect();
            return Err(bad(if names.is_empty() { format!("no ability publishes to '{}' here", c.label) } else { format!("'{id}' does not publish here; one of {}", names.join(", ")) }));
        }
        c.publisher = id;
    }
    c.save(&app.store, slug)?;
    app.store.commit(&format!("account({slug}): {}", c.label)).await?;
    app.bus.emit(Some(slug), EventKind::ConnectionSaved { connection: c.id.clone(), label: c.label.clone(), ability: c.ability.clone(), kind: c.kind.clone() });
    if learn_now {
        let _ = crate::learn::start(app, slug, Some(c.id.clone())).await;
    }
    Ok(c)
}

/// What to call a place from its fields: the first text value, else the account's name.
fn place_label(fields: &BTreeMap<String, Value>, fallback: &str) -> String {
    fields.values().find_map(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| fallback.to_string())
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct PlaceBody {
    pub label: String,
    pub fields: BTreeMap<String, Value>,
    pub default: bool,
    /// A mailing list (by id or name) as the place, on a mail account.
    pub list: String,
}

/// Add a place to an account: the kind's fields checked, or a list of people
/// to email. The first place is the default.
pub async fn create_place(app: &App, slug: &str, id: &str, b: PlaceBody) -> Result<(Connection, Place), ApiError> {
    let mut c = Connection::load(&app.store, slug, id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    if !c.is_channel() {
        return Err(bad(format!("'{}' does not publish, so it has no places", c.label)));
    }
    let place = if !b.list.trim().is_empty() {
        if !crate::publish::takes_lists(app, slug, &c).await {
            return Err(bad(format!("'{}' does not send to lists of people; only a mail account does", c.label)));
        }
        let l = kiqr_core::lists::List::find(&app.store, slug, &b.list).ok_or_else(|| bad(format!("no list '{}'", b.list.trim())))?;
        Place { id: format!("list-{}", l.id), kind: Place::LIST.into(), label: if b.label.trim().is_empty() { l.label.clone() } else { b.label.trim().to_string() }, fields: [("list".to_string(), json!(l.id))].into_iter().collect(), default: b.default }
    } else {
        let (_, _, checked) = crate::publish::place_check(app, slug, &c.kind, Some(&c.ability), &b.fields).await?;
        if checked.is_empty() {
            return Err(bad(format!("a place on '{}' needs its fields", c.label)));
        }
        Place { id: String::new(), kind: c.kind.clone(), label: if b.label.trim().is_empty() { place_label(&checked, &c.label) } else { b.label.trim().to_string() }, fields: checked, default: b.default }
    };
    if c.places.iter().any(|p| p.id != place.id && p.label.eq_ignore_ascii_case(&place.label)) {
        return Err(bad(format!("'{}' already has a place called '{}'", c.label, place.label)));
    }
    c.put_place(place.clone());
    let saved = c.places.iter().find(|p| p.label == place.label).cloned().unwrap_or(place);
    c.save(&app.store, slug)?;
    app.store.commit(&format!("account({slug}): {} publishes to {}", c.label, saved.label)).await?;
    app.bus.emit(Some(slug), EventKind::ConnectionSaved { connection: c.id.clone(), label: c.label.clone(), ability: c.ability.clone(), kind: c.kind.clone() });
    Ok((c, saved))
}

async fn add_place(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(b): Json<PlaceBody>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let (c, p) = create_place(&app, &slug, &id, b).await?;
    let mut v = view(&app, &slug, &c).await;
    v["place"] = json!(p);
    Ok((StatusCode::CREATED, Json(v)))
}

async fn update_place(State(app): State<App>, Path((slug, id, pid)): Path<(String, String, String)>, Json(b): Json<PlaceBody>) -> ApiResult<Value> {
    let mut c = Connection::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    let mut p = c.places.iter().find(|p| p.id == pid).cloned().ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no place '{pid}' on '{}'", c.label)))?;
    if !b.label.trim().is_empty() {
        p.label = b.label.trim().to_string();
    }
    if !b.fields.is_empty() && !p.is_list() {
        let (_, _, checked) = crate::publish::place_check(&app, &slug, &c.kind, Some(&c.ability), &b.fields).await?;
        p.fields = checked;
    }
    if b.default {
        p.default = true;
    }
    c.put_place(p);
    c.save(&app.store, &slug)?;
    app.store.commit(&format!("account({slug}): {}'s places", c.label)).await?;
    app.bus.emit(Some(&slug), EventKind::ConnectionSaved { connection: c.id.clone(), label: c.label.clone(), ability: c.ability.clone(), kind: c.kind.clone() });
    Ok(Json(view(&app, &slug, &c).await))
}

async fn remove_place(State(app): State<App>, Path((slug, id, pid)): Path<(String, String, String)>) -> ApiResult<Value> {
    let mut c = Connection::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    let gone = c.remove_place(&pid).ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no place '{pid}' on '{}'", c.label)))?;
    c.save(&app.store, &slug)?;
    app.store.commit(&format!("account({slug}): {} no longer publishes to {}", c.label, gone.label)).await?;
    app.bus.emit(Some(&slug), EventKind::ConnectionSaved { connection: c.id.clone(), label: c.label.clone(), ability: c.ability.clone(), kind: c.kind.clone() });
    Ok(Json(view(&app, &slug, &c).await))
}

/// `PUT …/allow {role: auto|ask|off, …}`: what Seemo may do here.
async fn put_allow(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(b): Json<BTreeMap<String, String>>) -> ApiResult<Value> {
    let c = update(&app, &slug, &id, Patch { allow: Some(b), ..Default::default() }).await?;
    Ok(Json(view(&app, &slug, &c).await))
}

pub async fn delete(app: &App, slug: &str, id: &str) -> Result<(), ApiError> {
    let c = Connection::delete(&app.store, slug, id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    app.mcp.stop_connection(&c.id).await;
    app.store.commit(&format!("account({slug}): remove {}", c.label)).await?;
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
    // An ability's accounts are its service's, shared with the other abilities of it.
    let service = match q.ability.as_deref() {
        Some(a) => Some(manifest(&app, &slug, a).await.map(|m| m.service().to_string()).unwrap_or_else(|| a.to_string())),
        None => None,
    };
    let mut out = Vec::new();
    for c in Connection::list(&app.store, &slug)?.iter().filter(|c| service.as_deref().is_none_or(|s| c.service() == s)) {
        out.push(view(&app, &slug, c).await);
    }
    Ok(Json(json!({ "connections": out })))
}

async fn create_route(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<NewConnection>) -> Result<(StatusCode, Json<Value>), ApiError> {
    // A first place comes checked against its kind.
    let mut b = b;
    if !b.place.is_empty() {
        let m = manifest(&app, &slug, &b.ability).await.ok_or_else(|| bad(format!("'{}' is not installed", b.ability)))?;
        let kind = if b.kind.trim().is_empty() { crate::publish::kinds(&app, &slug).await.into_iter().find(|(o, k)| o.service() == m.service() && k.publishes()).map(|(_, k)| k.kind).unwrap_or_default() } else { b.kind.trim().to_ascii_lowercase() };
        let (_, k, checked) = crate::publish::place_check(&app, &slug, &kind, Some(&m.id), &b.place).await?;
        b.kind = k.kind.clone();
        b.place = checked;
    }
    let c = create(&app, &slug, b, "user").await?;
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

/// "Learn now": read this source again and write the profile from it.
async fn learn_now(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let c = Connection::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    if !c.learns() {
        return Err(bad(format!("'{}' is not a source; allow learning on it first", c.label)));
    }
    crate::learn::start(&app, &slug, Some(c.id.clone())).await?;
    Ok(Json(json!({ "started": true })))
}

/// What a key can be set to, read through the account (a key's `choices`).
async fn choices(State(app): State<App>, Path((slug, id, name)): Path<(String, String, String)>) -> ApiResult<Value> {
    let c = Connection::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    let list = crate::ext::key_choices(&app, &slug, &c, &name).await.map_err(|e| bad(format!("{e:#}")))?;
    Ok(Json(json!({ "name": name, "choices": list })))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ActivityQuery {
    limit: Option<usize>,
}

/// What the abilities of its service did through an account, newest first, with today's count and the cap.
async fn activity(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Query(q): Query<ActivityQuery>) -> ApiResult<Value> {
    let c = Connection::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    let home = app.store.home();
    let mut all = c.activity(home, &slug);
    all.reverse();
    all.truncate(q.limit.unwrap_or(50).clamp(1, 500));
    let cap = app.registry.read().await.of_service(&slug, c.service()).iter().filter_map(|m| m.daily_cap).min();
    Ok(Json(json!({ "connection": c.id, "service": c.service(), "today": c.actions_today(home, &slug), "daily_cap": cap, "activity": all })))
}

async fn make_default(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    Connection::make_default(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    let c = Connection::load(&app.store, &slug, &id)?;
    app.store.commit(&format!("account({slug}): {} is the default for {}", c.label, c.ability)).await?;
    app.bus.emit(Some(&slug), EventKind::ConnectionSaved { connection: c.id.clone(), label: c.label.clone(), ability: c.ability.clone(), kind: c.kind.clone() });
    Ok(Json(view(&app, &slug, &c).await))
}
