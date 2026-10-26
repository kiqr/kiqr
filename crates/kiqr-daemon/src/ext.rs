//! Abilities in the daemon: the registry of folders under `abilities/`, the
//! merged tool list a session gets, the executor that
//! turns a manifest's `provides` into a request, a command or built-in Rust,
//! and the routes behind Setup → Abilities (instructions, secrets).

use crate::api::{ApiError, ApiResult};
use crate::App;
use anyhow::{anyhow, bail, Context, Result};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use base64::Engine;
use kiqr_core::abilities::{self, Ability};
use kiqr_core::assets;
use kiqr_core::connections::Connection;
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{self, CommandSpec, Entry, HttpRequest, Manifest, Provide, Registry, ResultMap, Scope, Transport};
use kiqr_core::project::Project;
use kiqr_core::provider;
use kiqr_core::tools::{self, CallSpec, Group, ToolSpec};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/tools", get(get_tools))
        .route("/api/extensions", get(list_extensions))
        .route("/api/extensions/reload", post(reload_extensions))
        .route("/api/extensions/{id}", get(get_extension).delete(delete_extension))
        .route("/api/secrets/{name}", put(put_secret))
        .route("/api/projects/{slug}/secrets/{name}", put(put_project_secret))
        .route("/api/abilities/{id}/instructions", get(get_instructions).put(put_instructions).delete(delete_instructions))
        .route("/api/projects/{slug}/abilities/{id}/instructions", get(get_project_instructions).put(put_project_instructions).delete(delete_project_instructions))
        .route("/api/projects/{slug}/abilities/{ability}", post(run_ability))
        .route("/api/projects/{slug}/ext/{ext}/tools/{tool}", post(run_ext_tool))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}
fn not_found(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::NOT_FOUND, msg.into())
}

// ---------------------------------------------------------------- registry

pub async fn reload(app: &App) {
    let core_off = app.config.read().await.core_off.clone();
    *app.registry.write().await = Registry::load(app.store.home(), &core_off);
}

/// Is an installed ability usable for a project right now: no keys needed,
/// or one of the project's connections of it has them all; program found.
/// Without a project (the machine's view) only an ability that needs no keys is.
pub fn ready_in(app: &App, slug: Option<&str>, m: &Manifest) -> bool {
    missing_in(app, slug, m).is_empty() && (m.transport != Transport::Mcp || on_path(&m.command))
}

/// Keys an ability still needs for a project: none once a connection has
/// them all, else what its default connection (or a new one) lacks.
pub fn missing_in(app: &App, slug: Option<&str>, m: &Manifest) -> Vec<String> {
    let all = extensions::missing_keys(m, |_| false);
    if all.is_empty() {
        return all;
    }
    let Some(slug) = slug.filter(|s| !s.is_empty()) else { return all };
    let home = app.store.home();
    let conns = Connection::of(&app.store, slug, &m.id);
    if conns.iter().any(|c| c.missing(home, slug, m).is_empty()) {
        return vec![];
    }
    conns.first().map(|c| c.missing(home, slug, m)).unwrap_or(all)
}

/// The connection a call goes through: the one named (id or label), else the
/// ability's default. None for an ability that needs no keys and has none.
pub fn pick_connection(app: &App, slug: &str, m: &Manifest, name: Option<&str>) -> Result<Option<Connection>> {
    if let Some(n) = name.map(str::trim).filter(|n| !n.is_empty()) {
        return match Connection::find(&app.store, slug, &m.id, n) {
            Some(c) => Ok(Some(c)),
            None => {
                let have: Vec<String> = Connection::of(&app.store, slug, &m.id).into_iter().map(|c| c.label).collect();
                bail!("{} has no account '{n}' in this project{}", m.name, if have.is_empty() { String::new() } else { format!(" (there is {})", have.join(", ")) })
            }
        };
    }
    let found = Connection::default_of(&app.store, slug, &m.id);
    if found.is_none() && !extensions::missing_keys(m, |_| false).is_empty() {
        bail!("{} is not connected in this project yet; add an account under Setup → Abilities", m.name);
    }
    Ok(found)
}

/// Is a program there: a path that exists, or a name found on PATH.
pub fn on_path(cmd: &str) -> bool {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return false;
    }
    if cmd.contains('/') {
        return std::path::Path::new(cmd).exists();
    }
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file())).unwrap_or(false)
}

/// The groups a chat in this project may use: every built-in one plus the
/// connections' own.
pub async fn chat_groups(app: &App, slug: &str) -> Vec<Group> {
    let reg = app.registry.read().await;
    let cfg = app.config.read().await.clone();
    let off = project_off(app, slug);
    let mut out: Vec<Group> = Group::BUILTIN.iter().filter(|g| core_allows(&cfg, g) && core_allows_in(&off, g)).cloned().collect();
    for (_, m) in reg.connections(slug) {
        if off.contains(&m.id) {
            continue;
        }
        if !m.tools.is_empty() || (m.transport == Transport::Mcp && !m.expose.is_empty()) {
            out.push(Group::Ext(m.id.clone()));
        }
    }
    out
}

/// What this project switched off for itself (ids of installed and core abilities).
pub(crate) fn project_off(app: &App, slug: &str) -> Vec<String> {
    Project::load(&app.store, slug).map(|p| p.abilities_off).unwrap_or_default()
}

/// The project's own switches for the built-in groups.
pub fn core_allows_in(off: &[String], g: &Group) -> bool {
    let id = match g {
        Group::WriteAssets => "write",
        Group::Web => "read-web",
        Group::Tasks => "tasks",
        _ => return true,
    };
    !off.iter().any(|x| x == id)
}

/// Connection id → name, for labelling `ext:` groups in plain words.
pub async fn connection_names(app: &App, slug: &str) -> std::collections::HashMap<String, String> {
    let reg = app.registry.read().await;
    reg.connections(slug).into_iter().map(|(_, m)| (m.id.clone(), m.name.clone())).collect()
}

/// A group's wording, naming the connection behind an `ext:` group.
pub fn group_label(g: &Group, names: &std::collections::HashMap<String, String>) -> String {
    match g {
        Group::Ext(id) => names.get(id).map(|n| format!("Use {n}")).unwrap_or_else(|| g.label()),
        other => other.label(),
    }
}

/// Which built-in ability a tool group belongs to, so a switched-off one
/// takes its tools with it: writing, reading the web, scheduling tasks.
pub fn core_allows(cfg: &kiqr_core::Config, g: &Group) -> bool {
    match g {
        Group::WriteAssets => cfg.core_on("write"),
        Group::Web => cfg.core_on("read-web"),
        Group::Tasks => cfg.core_on("tasks"),
        _ => true,
    }
}

/// The tools a session gets: built-ins in `allowed`, the abilities that have a
/// ready connection, and the connections' own tools.
pub async fn tool_specs(app: &App, slug: &str, allowed: &[Group]) -> Vec<ToolSpec> {
    let reg = app.registry.read().await;
    let cfg = app.config.read().await.clone();
    let off = project_off(app, slug);
    let allowed: Vec<Group> = allowed.iter().filter(|g| core_allows(&cfg, g) && core_allows_in(&off, g) && !matches!(g, Group::Ext(id) if off.contains(id))).cloned().collect();
    let allowed = &allowed[..];
    let mut out = tools::builtin_specs(allowed);
    for a in abilities::all() {
        if !allowed.contains(&a.group) {
            continue;
        }
        if let Some((_, m)) = reg.provider(slug, a.id, cfg.abilities.get(a.id).map(String::as_str), |m| !off.contains(&m.id) && ready_in(app, Some(slug), m)) {
            let cost = m.provides.get(a.id).and_then(|p| p.cost.as_deref());
            out.push(with_connection_arg(a.spec(&m.name, cost), &Connection::of(&app.store, slug, &m.id)));
        }
    }
    for (_, m) in reg.connections(slug) {
        let g = Group::Ext(m.id.clone());
        if !allowed.contains(&g) {
            continue;
        }
        if m.transport == Transport::Mcp && !m.expose.is_empty() && ready_in(app, Some(slug), m) {
            // The server's own tools, raw, under the connection's group.
            match app.mcp.tools(app.store.home().root(), m, &launch_for(app, slug, m)).await {
                Ok(list) => {
                    for t in list {
                        let name = t["name"].as_str().unwrap_or("").to_string();
                        if name.is_empty() || !(m.expose.iter().any(|e| e == "all") || m.expose.contains(&name)) {
                            continue;
                        }
                        let shown = if out.iter().any(|s| s.name == name) { format!("{}_{}", m.id.replace('-', "_"), name) } else { name.clone() };
                        out.push(ToolSpec {
                            name: shown,
                            group: g.clone(),
                            label: format!("Using {}: {}", m.name, name.replace('_', " ")),
                            description: t["description"].as_str().unwrap_or("").to_string(),
                            input_schema: t.get("inputSchema").cloned().unwrap_or_else(|| json!({ "type": "object", "properties": {}, "additionalProperties": true })),
                            call: CallSpec::Ext { ext: m.id.clone(), tool: name },
                            cost: None,
                        });
                    }
                }
                Err(e) => tracing::warn!(id = %m.id, error = %e, "could not list the MCP server's tools"),
            }
        }
        for t in &m.tools {
            let mut name = t.name.clone();
            if out.iter().any(|s| s.name == name) {
                name = format!("{}_{}", m.id.replace('-', "_"), t.name);
            }
            out.push(ToolSpec {
                name,
                group: g.clone(),
                label: if t.label.trim().is_empty() { format!("Using {}", m.name) } else { t.label.clone() },
                description: if t.description.trim().is_empty() { format!("A tool of the connection {}.", m.name) } else { t.description.clone() },
                input_schema: if t.input.is_null() { json!({ "type": "object", "properties": {}, "additionalProperties": true }) } else { t.input.clone() },
                call: CallSpec::Ext { ext: m.id.clone(), tool: t.name.clone() },
                cost: t.provide.cost.clone(),
            });
            let last = out.pop().expect("just pushed");
            out.push(with_connection_arg(last, &Connection::of(&app.store, slug, &m.id)));
        }
    }
    out
}

/// With more than one account of an ability in the project, its tools take
/// `connection`: which one, by name; the default when left out.
fn with_connection_arg(mut spec: ToolSpec, conns: &[Connection]) -> ToolSpec {
    if conns.len() < 2 {
        return spec;
    }
    let labels: Vec<String> = conns.iter().map(|c| c.label.clone()).collect();
    if !spec.input_schema.is_object() {
        spec.input_schema = json!({ "type": "object", "properties": {} });
    }
    if !spec.input_schema["properties"].is_object() {
        spec.input_schema["properties"] = json!({});
    }
    spec.input_schema["properties"]["connection"] = json!({ "type": "string", "enum": labels, "description": format!("Which account to use. Leave it out for the default, {}.", conns[0].label) });
    spec
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ToolsQuery {
    project: Option<String>,
    allowed: Option<String>,
}

/// The catalogue for the UI and for `kiqr mcp`: groups with labels, and the
/// tools of a project (with `project`), optionally narrowed to `allowed`.
async fn get_tools(State(app): State<App>, Query(q): Query<ToolsQuery>) -> ApiResult<Value> {
    let slug = q.project.clone().unwrap_or_default();
    let allowed: Vec<Group> = match &q.allowed {
        Some(a) => Group::parse_list(a),
        None => chat_groups(&app, &slug).await,
    };
    let specs = tool_specs(&app, &slug, &allowed).await;
    let mut groups = tools::groups_json();
    {
        let reg = app.registry.read().await;
        for (_, m) in reg.connections(&slug) {
            if m.tools.is_empty() {
                continue;
            }
            groups.push(json!({ "id": format!("ext:{}", m.id), "label": format!("Use {}", m.name), "tools": m.tools.iter().map(|t| t.name.clone()).collect::<Vec<_>>() }));
        }
        // Ability groups list the ability tools, whether or not a connection is ready.
        for g in groups.iter_mut() {
            let id = g["id"].as_str().unwrap_or("").to_string();
            let more: Vec<String> = abilities::all().into_iter().filter(|a| a.group.id() == id).map(|a| a.tool.to_string()).collect();
            if !more.is_empty() {
                let mut list: Vec<Value> = g["tools"].as_array().cloned().unwrap_or_default();
                list.extend(more.into_iter().map(Value::String));
                g["tools"] = Value::Array(list);
            }
        }
    }
    Ok(Json(json!({ "groups": groups, "tools": specs })))
}

// ------------------------------------------------------------- connections

fn connection_view(app: &App, e: &Entry, m: &Manifest) -> Value {
    let missing = missing_in(app, None, m);
    json!({
        "id": m.id,
        "name": m.name,
        "version": m.version,
        "summary": m.summary,
        "transport": m.transport,
        "homepage": m.homepage,
        "builtin": e.builtin,
        "folder": e.name,
        "scope": e.scope,
        "ready": ready_in(app, None, m),
        "missing": missing,
        "paused": e.paused,
        "git": e.git,
        "core": e.is_core(),
        "categories": m.categories,
        "icon": m.icon,
        "auth": m.auth.iter().map(|a| json!({ "env": a.env, "label": a.label, "help": a.help, "optional": a.optional, "options": a.options, "set": provider::has_env(&a.env) })).collect::<Vec<_>>(),
        "connect": kiqr_core::connect::view(m.connect.as_ref()),
        "provides": m.provides.iter().map(|(id, p)| json!({ "ability": id, "label": abilities::find(id).map(|a| a.label).unwrap_or(""), "cost": p.cost })).collect::<Vec<_>>(),
        "tools": m.tools.iter().map(|t| json!({ "name": t.name, "label": t.label, "cost": t.provide.cost })).collect::<Vec<_>>(),
        "channels": m.channels.iter().map(|c| json!({ "kind": c.kind, "label": c.label, "destination": c.destination })).collect::<Vec<_>>(),
        "hosts": m.hosts,
        "command": if m.transport == Transport::Mcp { Some(format!("{} {}", m.command, m.args.join(" ")).trim().to_string()) } else { None },
        "env": m.env,
        "program_found": if m.transport == Transport::Mcp { Some(on_path(&m.command)) } else { None },
    })
}

/// Every connection, plus folders whose manifest could not be read.
async fn list_extensions(State(app): State<App>) -> ApiResult<Value> {
    let reg = app.registry.read().await;
    let cfg = app.config.read().await.clone();
    let mut list: Vec<Value> = Vec::new();
    let mut broken: Vec<Value> = Vec::new();
    for e in &reg.entries {
        match (&e.manifest, &e.error) {
            (Some(m), None) => list.push(connection_view(&app, e, m)),
            (_, Some(err)) => broken.push(json!({ "folder": e.name, "scope": e.scope, "error": err })),
            _ => {}
        }
    }
    let abilities: Vec<Value> = abilities::all()
        .into_iter()
        .map(|a| {
            let configured = cfg.abilities.get(a.id).cloned();
            let candidates: Vec<Value> = reg.entries.iter().filter(|e| !e.paused).filter_map(|e| e.manifest.as_ref().filter(|m| m.provides.contains_key(a.id) && e.error.is_none())).map(|m| json!({ "id": m.id, "name": m.name, "ready": ready_in(&app, None, m) })).collect();
            let active = reg.provider("", a.id, configured.as_deref(), |m| ready_in(&app, None, m)).map(|(_, m)| json!({ "id": m.id, "name": m.name }));
            json!({ "id": a.id, "label": a.label, "tool": a.tool, "configured": configured, "active": active, "connections": candidates })
        })
        .collect();
    Ok(Json(json!({ "connections": list, "broken": broken, "abilities": abilities })))
}

async fn get_extension(State(app): State<App>, Path(id): Path<String>) -> ApiResult<Value> {
    let reg = app.registry.read().await;
    let found = reg.entries.iter().find_map(|e| e.manifest.as_ref().filter(|m| m.id == id && e.error.is_none()).map(|m| connection_view(&app, e, m)));
    found.map(Json).ok_or_else(|| not_found(format!("no connection '{id}'")))
}

async fn reload_extensions(State(app): State<App>) -> ApiResult<Value> {
    reload(&app).await;
    let n = app.registry.read().await.entries.len();
    Ok(Json(json!({ "entries": n })))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ScopeQuery {
    project: Option<String>,
}

/// Remove a user-installed ability's folder (instructions included).
async fn delete_extension(State(app): State<App>, Path(id): Path<String>, Query(q): Query<ScopeQuery>) -> Result<StatusCode, ApiError> {
    let (dir, name, scope) = {
        let reg = app.registry.read().await;
        let scope = q.project.clone().map(Scope::Project).unwrap_or(Scope::Global);
        let e = reg.entries.iter().find(|e| e.scope == scope && e.manifest.as_ref().is_some_and(|m| m.id == id)).ok_or_else(|| not_found(format!("no ability '{id}'")))?;
        if e.is_core() {
            return Err(bad("this is one of KIQR's core abilities: it can be switched off, not removed"));
        }
        (e.dir.clone(), e.id().to_string(), e.scope.clone())
    };
    remove_folder(&app, &dir, &name, &scope).await?;
    app.mcp.stop(&id).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Remove an ability's folder and say so (`ExtensionRemoved`). `id` is the ability id.
pub(crate) async fn remove_folder(app: &App, dir: &std::path::Path, id: &str, scope: &Scope) -> Result<(), ApiError> {
    std::fs::remove_dir_all(dir).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("could not remove {}: {e}", dir.display())))?;
    let label = match scope {
        Scope::Global => "abilities".to_string(),
        Scope::Project(s) => format!("abilities({s})"),
    };
    app.store.commit(&format!("{label}: remove {id}")).await?;
    reload(app).await;
    app.bus.emit(None, EventKind::ExtensionRemoved { id: id.to_string() });
    Ok(())
}

pub(crate) fn scope_id(s: &Scope) -> String {
    match s {
        Scope::Global => "global".into(),
        Scope::Project(p) => p.clone(),
    }
}

// ----------------------------------------------------------------- secrets

#[derive(Deserialize)]
struct SecretBody {
    #[serde(default)]
    value: String,
}

/// Is this a name some installed or shipped ability, or a provider, declares?
/// Secrets are set by name only, so an unknown name is refused.
pub(crate) async fn known_secret(app: &App, name: &str) -> bool {
    if crate::api::SECRETS.contains(&name) {
        return true;
    }
    let uses = |m: &Manifest| m.auth.iter().any(|a| a.env == name) || m.connect.as_ref().is_some_and(|c| c.writes().iter().any(|w| w == name)) || m.env.contains(&name.to_string()) || m.tools.iter().any(|t| t.provide.command.as_ref().is_some_and(|c| c.env.contains(&name.to_string())));
    let reg = app.registry.read().await;
    reg.entries.iter().filter_map(|e| e.manifest.as_ref()).any(uses) || extensions::shipped_all().iter().any(|(_, m)| uses(m))
}

/// Set (or, with an empty value, clear) one of KIQR's own keys. `slug` picks
/// the project's `.env`; none means the machine's. An ability's keys belong
/// to one of its connections (`set_connection_secret`). Never read back.
pub(crate) async fn set_secret(app: &App, slug: Option<&str>, name: &str, value: &str) -> Result<bool, ApiError> {
    let value = check_secret(app, name, value).await?;
    if !crate::api::SECRETS.contains(&name) {
        return Err(bad(format!("{name} belongs to an ability: set it on one of the project's accounts of it (Setup → Abilities)")));
    }
    let path = match slug {
        Some(s) => {
            Project::load(&app.store, s)?;
            app.store.home().project_env_file(s)
        }
        None => app.store.home().env_file(),
    };
    kiqr_core::secrets::write_env(&path, name, &value).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if slug.is_none() {
        if value.is_empty() {
            std::env::remove_var(name);
        } else {
            std::env::set_var(name, &value);
        }
    }
    app.bus.emit(slug, EventKind::SecretSet { name: name.to_string(), set: !value.is_empty(), project: slug.map(str::to_string) });
    Ok(!value.is_empty())
}

/// Set (or clear) one key of a connection: a name its ability uses.
pub(crate) async fn set_connection_secret(app: &App, slug: &str, conn: &str, name: &str, value: &str) -> Result<bool, ApiError> {
    let value = check_secret(app, name, value).await?;
    let c = Connection::load(&app.store, slug, conn).map_err(|e| not_found(e.to_string()))?;
    let m = {
        let reg = app.registry.read().await;
        reg.entry(Some(slug), &c.ability).and_then(|e| e.manifest.clone())
    }
    .ok_or_else(|| bad(format!("the ability '{}' is not installed", c.ability)))?;
    if !kiqr_core::connections::key_names(&m).iter().any(|n| n == name) {
        return Err(bad(format!("{} does not use {name}", m.name)));
    }
    if let Some(k) = m.auth.iter().find(|a| a.env == name && !a.options.is_empty()) {
        if !value.is_empty() && !k.options.contains(&value) {
            return Err(bad(format!("{} is one of {}", if k.label.is_empty() { name } else { k.label.as_str() }, k.options.join(", "))));
        }
    }
    kiqr_core::secrets::write_env(&c.env_file(app.store.home(), slug), name, &value).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    app.mcp.stop(&format!("{}#{}", c.ability, c.id)).await;
    app.bus.emit(Some(slug), EventKind::SecretSet { name: name.to_string(), set: !value.is_empty(), project: Some(slug.to_string()) });
    Ok(!value.is_empty())
}

async fn check_secret(app: &App, name: &str, value: &str) -> Result<String, ApiError> {
    if !extensions::valid_env_name(name) {
        return Err(bad("a secret name is upper-case letters, digits and underscores"));
    }
    if !known_secret(app, name).await {
        return Err(bad(format!("no ability or provider uses {name}")));
    }
    let mut value = value.trim().to_string();
    if value.contains('\n') {
        return Err(bad("a secret is one line"));
    }
    // A site address is used as a base for paths: "https://myblog.com/" and "https://myblog.com" mean the same.
    if (value.starts_with("https://") || value.starts_with("http://")) && value.ends_with('/') && url::Url::parse(&value).is_ok_and(|u| u.path() == "/" && u.query().is_none()) {
        value.pop();
    }
    Ok(value)
}

async fn put_secret(State(app): State<App>, Path(name): Path<String>, Json(b): Json<SecretBody>) -> ApiResult<Value> {
    let set = set_secret(&app, None, &name, &b.value).await?;
    Ok(Json(json!({ "name": name, "set": set })))
}

async fn put_project_secret(State(app): State<App>, Path((slug, name)): Path<(String, String)>, Json(b): Json<SecretBody>) -> ApiResult<Value> {
    let set = set_secret(&app, Some(&slug), &name, &b.value).await?;
    Ok(Json(json!({ "name": name, "set": set, "project": slug })))
}

// ------------------------------------------------------------ instructions

fn valid_ability_id(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && !name.starts_with(['-', '.'])
}

/// The instruction folder an (ability, file) pair names: a core ability's
/// shipped file, an installed folder's own SKILL.md, or a folder named
/// directly. `None` when the ability is unknown (a PUT then creates it).
async fn instruction_folder(app: &App, slug: Option<&str>, id: &str, file: Option<&str>) -> Result<Option<String>, ApiError> {
    if !valid_ability_id(id) {
        return Err(bad("an ability id is letters, digits and dashes"));
    }
    if let Some(c) = extensions::core_ability(id) {
        let files: Vec<&str> = c.instructions.iter().map(|(f, _)| *f).collect();
        return match file {
            Some(f) if files.contains(&f) => Ok(Some(f.to_string())),
            Some(f) => Err(not_found(format!("{} has no instructions file '{f}'", c.name))),
            None => files.first().map(|f| Some(f.to_string())).ok_or_else(|| not_found(format!("{} has no instructions to read; it is code", c.name))),
        };
    }
    let reg = app.registry.read().await;
    if let Some(e) = reg.entry(slug, id) {
        if let Some(f) = file {
            if f != e.name {
                return Err(not_found(format!("'{id}' has no instructions file '{f}'")));
            }
        }
        return Ok(Some(e.name.clone()));
    }
    if reg.entries.iter().any(|e| e.name == id && e.instructions.is_some()) || kiqr_core::extract::builtin_instructions(id).is_some() {
        return Ok(Some(id.to_string()));
    }
    Ok(None)
}

/// The text as it applies: the project's copy, then the machine's, then the shipped one.
fn instructions_text(app: &App, slug: Option<&str>, folder: &str) -> Result<String, ApiError> {
    if let Some(s) = slug {
        if let Ok(t) = std::fs::read_to_string(app.store.home().project_abilities_dir(s).join(folder).join("SKILL.md")) {
            return Ok(t);
        }
    }
    if let Ok(t) = std::fs::read_to_string(app.store.home().abilities_dir().join(folder).join("SKILL.md")) {
        return Ok(t);
    }
    shipped_text(folder).map(str::to_string).ok_or_else(|| not_found(format!("'{folder}' has no instructions")))
}

fn shipped_text(folder: &str) -> Option<&'static str> {
    kiqr_core::extract::builtin_instructions(folder).or_else(|| extensions::BUILTIN_EXTENSIONS.iter().find(|(n, _, _)| *n == folder).and_then(|(_, s, _)| *s))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct InstructionsQuery {
    file: Option<String>,
    /// Set by the UI: read it even when the ability is switched off here.
    all: Option<String>,
}

async fn read_instructions(app: &App, slug: Option<&str>, id: &str, q: &InstructionsQuery) -> Result<String, ApiError> {
    let folder = instruction_folder(app, slug, id, q.file.as_deref()).await?.ok_or_else(|| not_found(format!("no ability '{id}'")))?;
    if q.all.is_none() {
        if let Some(s) = slug {
            let (machine, project) = crate::market::on_for(app, s, id).await;
            if !machine || !project {
                return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("'{id}' is switched off {}; the user can turn it on under Setup → Abilities", if machine { "for this project" } else { "on this machine" })));
            }
        }
    }
    instructions_text(app, slug, &folder)
}

async fn get_instructions(State(app): State<App>, Path(id): Path<String>, Query(q): Query<InstructionsQuery>) -> Result<String, ApiError> {
    read_instructions(&app, None, &id, &q).await
}

async fn get_project_instructions(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Query(q): Query<InstructionsQuery>) -> Result<String, ApiError> {
    Project::load(&app.store, &slug)?;
    read_instructions(&app, Some(&slug), &id, &q).await
}

#[derive(Deserialize)]
struct InstructionsBody {
    text: String,
    #[serde(default)]
    file: Option<String>,
}

/// Write an ability's instructions at a scope: a new ability (taught here),
/// the user's edit of their own, or a copy of shipped text. Frontmatter is
/// added when the text has none.
pub(crate) async fn save_instructions(app: &App, scope: Scope, id: &str, file: Option<&str>, text: &str) -> ApiResult<Value> {
    let slug = match &scope {
        Scope::Global => None,
        Scope::Project(s) => Some(s.as_str()),
    };
    let folder = instruction_folder(app, slug, id, file).await?.unwrap_or_else(|| id.to_string());
    let text = text.trim();
    if text.is_empty() {
        return Err(bad("instructions need some text"));
    }
    let dir = match &scope {
        Scope::Global => app.store.home().abilities_dir(),
        Scope::Project(s) => {
            Project::load(&app.store, s)?;
            app.store.home().project_abilities_dir(s)
        }
    };
    let body = if text.starts_with("---") { format!("{text}\n") } else { format!("---\nname: {id}\ndescription: {}\n---\n\n{text}\n", first_line(text)) };
    app.store.write_text(&dir.join(&folder).join("SKILL.md"), &body)?;
    let label = match &scope {
        Scope::Global => "abilities".to_string(),
        Scope::Project(s) => format!("abilities({s})"),
    };
    app.store.commit(&format!("{label}: {id}")).await?;
    reload(app).await;
    app.bus.emit(None, EventKind::AbilitySaved { id: id.to_string(), scope: scope_id(&scope) });
    Ok(Json(crate::market::one(app, slug, id).await.unwrap_or(Value::Null)))
}

fn first_line(text: &str) -> String {
    let l = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim_start_matches('#').trim();
    kiqr_core::profile::truncate_chars(l, 140)
}

async fn put_instructions(State(app): State<App>, Path(id): Path<String>, Json(b): Json<InstructionsBody>) -> ApiResult<Value> {
    save_instructions(&app, Scope::Global, &id, b.file.as_deref(), &b.text).await
}

async fn put_project_instructions(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(b): Json<InstructionsBody>) -> ApiResult<Value> {
    save_instructions(&app, Scope::Project(slug), &id, b.file.as_deref(), &b.text).await
}

/// Drop the copy at a scope: a project's copy goes, the machine's edit of a
/// shipped file goes back to the shipped text, and a folder with nothing
/// else in it is removed.
async fn delete_instructions_in(app: &App, scope: Scope, id: &str, file: Option<&str>) -> Result<StatusCode, ApiError> {
    let slug = match &scope {
        Scope::Global => None,
        Scope::Project(s) => Some(s.as_str()),
    };
    let folder = instruction_folder(app, slug, id, file).await?.ok_or_else(|| not_found(format!("no ability '{id}'")))?;
    let root = match &scope {
        Scope::Global => app.store.home().abilities_dir(),
        Scope::Project(s) => app.store.home().project_abilities_dir(s),
    };
    let dir = root.join(&folder);
    let path = dir.join("SKILL.md");
    if !path.exists() {
        return Err(not_found(format!("there is no copy of '{folder}' {}", if slug.is_some() { "for this project" } else { "on this machine" })));
    }
    let label = match &scope {
        Scope::Global => "abilities".to_string(),
        Scope::Project(s) => format!("abilities({s})"),
    };
    if scope == Scope::Global {
        if let Some(shipped) = shipped_text(&folder) {
            app.store.write_text(&path, shipped)?;
            let _ = std::fs::write(dir.join(".builtin"), shipped);
            app.store.commit(&format!("{label}: {id} back to the shipped text")).await?;
            reload(app).await;
            app.bus.emit(None, EventKind::AbilitySaved { id: id.to_string(), scope: scope_id(&scope) });
            return Ok(StatusCode::NO_CONTENT);
        }
    }
    if extensions::is_core_id(id) && scope == Scope::Global {
        return Err(bad("a core ability's instructions can be edited, not removed"));
    }
    std::fs::remove_file(&path).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let _ = std::fs::remove_file(dir.join(".builtin"));
    let _ = std::fs::remove_file(dir.join("SKILL.previous.md"));
    let empty = std::fs::read_dir(&dir).map(|rd| rd.flatten().all(|e| e.file_name().to_str().is_some_and(|n| n.starts_with('.')))).unwrap_or(true);
    if empty && !dir.join("extension.yaml").exists() {
        remove_folder(app, &dir, id, &scope).await?;
    } else {
        app.store.commit(&format!("{label}: remove the instructions of {id}")).await?;
        reload(app).await;
        app.bus.emit(None, EventKind::AbilitySaved { id: id.to_string(), scope: scope_id(&scope) });
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_instructions(State(app): State<App>, Path(id): Path<String>, Query(q): Query<InstructionsQuery>) -> Result<StatusCode, ApiError> {
    delete_instructions_in(&app, Scope::Global, &id, q.file.as_deref()).await
}

async fn delete_project_instructions(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Query(q): Query<InstructionsQuery>) -> Result<StatusCode, ApiError> {
    delete_instructions_in(&app, Scope::Project(slug), &id, q.file.as_deref()).await
}

#[derive(Deserialize)]
pub(crate) struct Install {
    /// A folder on this machine, or a git URL.
    pub source: String,
    /// Folder name to install under; default: the manifest id, else the source's last path part.
    #[serde(default)]
    pub name: Option<String>,
}

/// Copy an ability folder (SKILL.md and/or extension.yaml, templates) into `abilities/`.
pub(crate) async fn install_into(app: &App, scope: Scope, b: Install) -> Result<(StatusCode, Json<Value>), ApiError> {
    let source = b.source.trim().to_string();
    if source.is_empty() {
        return Err(bad("source is a folder path or a git URL"));
    }
    let tmp = app.store.home().root().join("tmp").join("install").join(uuid::Uuid::new_v4().to_string());
    let from: std::path::PathBuf = if source.starts_with("http://") || source.starts_with("https://") || source.starts_with("git@") || source.ends_with(".git") {
        std::fs::create_dir_all(&tmp).map_err(anyhow::Error::from)?;
        let status = tokio::process::Command::new("git").args(["clone", "--depth", "1", "--quiet", &source, "repo"]).current_dir(&tmp).status().await.map_err(|e| bad(format!("could not run git: {e}")))?;
        if !status.success() {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(bad(format!("git could not clone {source}")));
        }
        tmp.join("repo")
    } else {
        let p = std::path::PathBuf::from(&source);
        if !p.is_dir() {
            return Err(bad(format!("{source} is not a folder")));
        }
        p
    };
    let has_instructions = from.join("SKILL.md").exists();
    let manifest_text = std::fs::read_to_string(from.join("extension.yaml")).ok();
    if !has_instructions && manifest_text.is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(bad("the folder has neither SKILL.md nor extension.yaml"));
    }
    let manifest = match &manifest_text {
        Some(t) => Some(extensions::parse_manifest(t).map_err(|e| bad(format!("extension.yaml: {e:#}")))?),
        None => None,
    };
    let name = b.name.clone().filter(|n| !n.trim().is_empty()).or_else(|| manifest.as_ref().map(|m| m.id.clone())).or_else(|| from.file_name().map(|n| n.to_string_lossy().trim_end_matches(".git").to_string())).unwrap_or_default();
    if !valid_ability_id(&name) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(bad(format!("'{name}' is not a usable folder name")));
    }
    let dest_root = match &scope {
        Scope::Global => app.store.home().abilities_dir(),
        Scope::Project(s) => {
            Project::load(&app.store, s)?;
            app.store.home().project_abilities_dir(s)
        }
    };
    let dest = dest_root.join(&name);
    if dest.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(ApiError(StatusCode::CONFLICT, format!("an ability named '{name}' is already installed; remove it first")));
    }
    copy_dir(&from, &dest).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("could not copy: {e}")))?;
    let _ = std::fs::remove_dir_all(&tmp);
    let label = match &scope {
        Scope::Global => "abilities".to_string(),
        Scope::Project(s) => format!("abilities({s})"),
    };
    app.store.commit(&format!("{label}: install {name}")).await?;
    reload(app).await;
    let id = manifest.as_ref().map(|m| m.id.clone()).unwrap_or_else(|| name.clone());
    if has_instructions {
        app.bus.emit(None, EventKind::AbilitySaved { id: id.clone(), scope: scope_id(&scope) });
    }
    app.bus.emit(None, EventKind::ExtensionInstalled { id: id.clone(), name: manifest.as_ref().map(|m| m.name.clone()).unwrap_or_else(|| name.clone()) });
    let slug = match &scope {
        Scope::Global => None,
        Scope::Project(s) => Some(s.as_str()),
    };
    let mut view = crate::market::one(app, slug, &id).await.unwrap_or(Value::Null);
    if view.is_object() {
        view["name_on_disk"] = json!(name);
    }
    Ok((StatusCode::CREATED, Json(view)))
}

/// Copy files (not symlinks, not `.git`), at most 20 MB in all.
pub(crate) fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> Result<()> {
    let mut total: u64 = 0;
    fn walk(from: &std::path::Path, to: &std::path::Path, total: &mut u64) -> Result<()> {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)? {
            let e = e?;
            let name = e.file_name();
            if name == ".git" {
                continue;
            }
            let meta = e.metadata()?;
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                walk(&e.path(), &to.join(&name), total)?;
            } else {
                *total += meta.len();
                if *total > 20 * 1024 * 1024 {
                    bail!("the folder is larger than 20 MB");
                }
                std::fs::copy(e.path(), to.join(&name))?;
            }
        }
        Ok(())
    }
    walk(from, to, &mut total)
}


// ---------------------------------------------------------------- executor

/// What a connection produced.
pub struct ExecOut {
    /// The response, for the model (large file fields removed).
    pub json: Value,
    pub text: Option<String>,
    /// A file: bytes and media type.
    pub file: Option<(Vec<u8>, String)>,
}

/// Run one `provides` entry or extra tool of an installed ability. Secrets
/// come from the project's `.env`, then the machine's.
/// `conn` names the connection (id or label); none uses the ability's default.
pub async fn execute(app: &App, slug: &str, m: &Manifest, provide: &Provide, ability: Option<&Ability>, tool: Option<&str>, args: &Value, conn: Option<&str>) -> Result<ExecOut> {
    let conn = pick_connection(app, slug, m, conn)?;
    let mut args = provide.mapped_args(args);
    if let Some(o) = args.as_object_mut() {
        o.remove("connection");
    }
    let files = call_files(app, slug, provide, &mut args)?;
    let stored = kiqr_core::connections::env_for(app.store.home(), slug, conn.as_ref(), m);
    // Tokens a sign-in flow mints or refreshes for this call come first.
    let minted = std::sync::Arc::new(crate::connect::call_env(app, slug, m, conn.as_ref()).await?);
    let env = move |k: &str| minted.get(k).cloned().or_else(|| stored(k));
    match m.transport {
        Transport::Http if !provide.steps.is_empty() => steps_exec(m, &provide.steps, &args, &provide.result, &env, &files).await,
        Transport::Http => {
            let req = provide.request.as_ref().ok_or_else(|| anyhow!("{} has no request for this", m.name))?;
            http_exec(m, req, &args, &provide.result, &env, &files).await
        }
        Transport::Command => {
            let cmd = provide.command.as_ref().ok_or_else(|| anyhow!("{} has no command for this", m.name))?;
            command_exec(cmd, &args, &provide.result, &env).await
        }
        Transport::Mcp => {
            let server_tool = provide.tool.clone().or_else(|| tool.map(str::to_string)).ok_or_else(|| anyhow!("{} does not say which of its tools to call", m.name))?;
            let call_args = match &provide.args {
                Some(t) => extensions::fill_value(t, &args, &env),
                None => args.clone(),
            };
            let r = app.mcp.call(app.store.home().root(), m, &mcp_env(m, conn.as_ref(), &env), &server_tool, &call_args).await?;
            let out = crate::mcp_host::read_result(&r);
            if out.is_error {
                bail!("{}: {}", m.name, kiqr_core::profile::truncate_chars(out.text.trim(), 300));
            }
            let mut json = out.structured.clone().unwrap_or_else(|| json!({ "text": out.text }));
            let mut file = out.file;
            // The manifest may still point at a file inside the structured answer.
            if file.is_none() && (provide.result.file_url.is_some() || provide.result.file_b64.is_some()) {
                let client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(60)).build()?;
                let read = read_result(m, &client, json.clone(), &provide.result).await?;
                json = read.json;
                file = read.file;
            }
            let text = provide.result.text.as_ref().and_then(|p| extensions::pointer(&json, p)).and_then(|v| v.as_str().map(str::to_string)).or_else(|| if out.text.is_empty() { None } else { Some(out.text.clone()) });
            Ok(ExecOut { json, text, file })
        }
        Transport::Smtp => crate::smtp::exec(app, slug, m, &args, &env).await,
        Transport::Builtin => crate::builtin::run(app, slug, m, ability, tool, &args).await,
    }
}

/// The launch for a project's default connection of a hosted program.
pub fn launch_for(app: &App, slug: &str, m: &Manifest) -> crate::mcp_host::Launch {
    let conn = Connection::default_of(&app.store, slug, &m.id);
    let env = kiqr_core::connections::env_for(app.store.home(), slug, conn.as_ref(), m);
    mcp_env(m, conn.as_ref(), &env)
}

/// What a hosted program is started with: its connection (the server runs
/// once per connection) and the values of the names it asks for.
pub fn mcp_env(m: &Manifest, conn: Option<&Connection>, env: &impl Fn(&str) -> Option<String>) -> crate::mcp_host::Launch {
    let vars = kiqr_core::connections::key_names(m).into_iter().filter_map(|k| env(&k).map(|v| (k, v))).collect();
    crate::mcp_host::Launch { key: conn.map(|c| format!("{}#{}", m.id, c.id)).unwrap_or_else(|| m.id.clone()), vars }
}

/// The asset's files, when the call names any (`{{file…}}`): their bytes for
/// the executor, and `files` in the arguments as `[{name, mime, size}]`
/// (with `base64` when the manifest asks for it, for APIs that take JSON only).
fn call_files(app: &App, slug: &str, provide: &Provide, args: &mut Value) -> Result<Files> {
    let text = serde_json::to_string(provide).unwrap_or_default();
    if !text.contains("{{file") {
        return Ok(vec![]);
    }
    let (Some(kind), Some(id)) = (args["asset"]["type"].as_str(), args["asset"]["id"].as_str()) else { return Ok(vec![]) };
    if id.is_empty() {
        return Ok(vec![]);
    }
    let a = assets::load(&app.store, slug, kind, id)?;
    let files = crate::publish::asset_parts(app, slug, &a)?.1;
    let b64 = text.contains("base64");
    if let Some(o) = args.as_object_mut() {
        o.insert(
            "files".into(),
            json!(files
                .iter()
                .map(|(n, b, mime)| {
                    let mut f = json!({ "name": n, "mime": mime, "size": b.len() });
                    if b64 {
                        f["base64"] = json!(base64::engine::general_purpose::STANDARD.encode(b));
                    }
                    f
                })
                .collect::<Vec<_>>()),
        );
    }
    Ok(files)
}

fn no_leftover(s: &str, what: &str) -> Result<()> {
    if let Some(i) = s.find("${") {
        let end = s[i..].find('}').map(|e| i + e + 1).unwrap_or(s.len());
        bail!("{what} needs {} which is not set; add it under Setup → Abilities, or in the project's Channels for a mail account", &s[i..end]);
    }
    Ok(())
}

/// The asset's files a call may send: (name, bytes, type).
pub type Files = Vec<(String, Vec<u8>, String)>;

/// What a server answered, after a success status.
struct Answer {
    client: reqwest::Client,
    ctype: String,
    bytes: Vec<u8>,
}

impl Answer {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.bytes).unwrap_or_else(|_| json!({ "text": String::from_utf8_lossy(&self.bytes) }))
    }
}

/// A file a template names: `{{file}}`, `{{files.0}}`, or a file name.
fn file_ref<'a>(template: &str, args: &Value, env: &(impl Fn(&str) -> Option<String> + Clone), files: &'a Files) -> Result<&'a (String, Vec<u8>, String)> {
    let v = extensions::fill_value(&Value::String(template.to_string()), args, env);
    let name = v.get("name").and_then(|n| n.as_str()).map(str::to_string).or_else(|| v.as_str().map(str::to_string)).unwrap_or_default();
    files.iter().find(|(n, _, _)| *n == name).ok_or_else(|| if files.is_empty() { anyhow!("the asset has no files to send") } else { anyhow!("{template} is not one of the asset's files") })
}

/// Make one request: URL and hosts, query, headers, auth, and the body
/// (`json`, `form`, `multipart` or `body_file`). A failure status is an error.
async fn send(m: &Manifest, req: &HttpRequest, args: &Value, env: &(impl Fn(&str) -> Option<String> + Clone), files: &Files) -> Result<Answer> {
    let url_s = extensions::fill(&req.url, args, env.clone());
    no_leftover(&url_s, "the request URL")?;
    let mut url = url::Url::parse(url_s.trim()).with_context(|| format!("{url_s} is not a URL"))?;
    if !extensions::host_allowed(&url, &extensions::hosts_for(m, env)) {
        bail!("{} may not call {}", m.name, url.host_str().unwrap_or(""));
    }
    for (k, v) in &req.query {
        let v = extensions::fill(v, args, env.clone());
        if !v.is_empty() {
            url.query_pairs_mut().append_pair(k, &v);
        }
    }
    let client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(req.timeout_s.unwrap_or(120).clamp(5, 900))).user_agent(kiqr_core::fetch::USER_AGENT).build()?;
    let method = reqwest::Method::from_bytes(req.method.to_ascii_uppercase().as_bytes())?;
    let mut r = client.request(method, url.clone());
    let mut has_ctype = false;
    for (k, v) in &req.headers {
        let v = extensions::fill(v, args, env.clone());
        no_leftover(&v, &format!("the header {k}"))?;
        has_ctype |= k.eq_ignore_ascii_case("content-type");
        r = r.header(k, v);
    }
    if let Some(pair) = &req.basic_auth {
        let user = extensions::fill(pair.first().map(String::as_str).unwrap_or(""), args, env.clone());
        let pass = pair.get(1).map(|p| extensions::fill(p, args, env.clone()));
        no_leftover(&user, "the user name")?;
        if let Some(p) = &pass {
            no_leftover(p, "the password")?;
        }
        r = r.basic_auth(user, pass);
    }
    if let Some(body) = &req.json {
        let b = extensions::fill_value(body, args, env);
        no_leftover(&b.to_string(), "the request body")?;
        r = r.json(&b);
    } else if !req.form.is_empty() {
        let mut pairs = Vec::new();
        for (k, v) in &req.form {
            let v = extensions::fill(v, args, env.clone());
            no_leftover(&v, &format!("the form field {k}"))?;
            pairs.push((k.clone(), v));
        }
        r = r.form(&pairs);
    } else if !req.multipart.is_empty() {
        let mut form = reqwest::multipart::Form::new();
        for part in &req.multipart {
            if let Some(v) = &part.value {
                let v = extensions::fill(v, args, env.clone());
                no_leftover(&v, &format!("the part {}", part.name))?;
                form = form.text(part.name.clone(), v);
            } else if let Some(f) = &part.file {
                let (name, bytes, mime) = file_ref(f, args, env, files)?;
                let file_name = part.filename.as_ref().map(|t| extensions::fill(t, args, env.clone())).filter(|t| !t.is_empty()).unwrap_or_else(|| name.clone());
                let mime = part.mime.as_ref().map(|t| extensions::fill(t, args, env.clone())).filter(|t| !t.is_empty()).unwrap_or_else(|| mime.clone());
                form = form.part(part.name.clone(), reqwest::multipart::Part::bytes(bytes.clone()).file_name(file_name).mime_str(&mime)?);
            }
        }
        r = r.multipart(form);
    } else if let Some(f) = &req.body_file {
        let (_, bytes, mime) = file_ref(f, args, env, files)?;
        if !has_ctype {
            r = r.header(reqwest::header::CONTENT_TYPE, mime.as_str());
        }
        r = r.body(bytes.clone());
    }
    let resp = r.send().await.with_context(|| format!("calling {}", url.host_str().unwrap_or("")))?;
    let status = resp.status();
    let ctype = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let bytes = resp.bytes().await?.to_vec();
    if !status.is_success() {
        let text = String::from_utf8_lossy(&bytes);
        bail!("{} answered {}: {}", m.name, status.as_u16(), kiqr_core::profile::truncate_chars(text.trim(), 300));
    }
    Ok(Answer { client, ctype, bytes })
}

async fn http_exec(m: &Manifest, req: &HttpRequest, args: &Value, map: &ResultMap, env: &(impl Fn(&str) -> Option<String> + Clone), files: &Files) -> Result<ExecOut> {
    let a = send(m, req, args, env, files).await?;
    // A file straight back.
    if !a.ctype.starts_with("application/json") && !a.ctype.starts_with("text/") && !a.bytes.is_empty() && map.file_url.is_none() && map.file_b64.is_none() {
        return Ok(ExecOut { json: json!({ "bytes": a.bytes.len(), "mime": a.ctype }), text: None, file: Some((a.bytes.clone(), map.mime.clone().unwrap_or(a.ctype.clone()))) });
    }
    read_result(m, &a.client, a.json(), map).await
}

/// Several requests in order. Each step sees the arguments and what earlier
/// steps saved (`{{name.var}}`); `when` skips one, `for_each: files` runs one
/// per file and saves a list. The result reads the last answer, or, for a
/// `text` that is a template, anything saved.
async fn steps_exec(m: &Manifest, steps: &[extensions::Step], args: &Value, map: &ResultMap, env: &(impl Fn(&str) -> Option<String> + Clone), files: &Files) -> Result<ExecOut> {
    let mut ctx = if args.is_object() { args.clone() } else { json!({}) };
    let mut saved = serde_json::Map::new();
    let mut last: Option<Answer> = None;
    for st in steps {
        let label = |i: Option<usize>| match i {
            Some(i) => format!("step '{}' (file {})", st.name, i + 1),
            None => format!("step '{}'", st.name),
        };
        if let Some(w) = &st.when {
            if !extensions::truthy(&extensions::fill_value(&Value::String(w.clone()), &ctx, env)) {
                let skipped = if st.for_each.is_some() { json!([]) } else { Value::Null };
                ctx[&st.name] = skipped.clone();
                saved.insert(st.name.clone(), skipped);
                continue;
            }
        }
        let runs: Vec<Option<(usize, Value)>> = match st.for_each.as_deref() {
            Some(list) => {
                let items = extensions::lookup(&ctx, list).and_then(|v| v.as_array().cloned()).unwrap_or_default();
                if items.len() > extensions::MAX_EACH {
                    bail!("{}: {} files, at most {}", label(None), items.len(), extensions::MAX_EACH);
                }
                items.into_iter().enumerate().map(Some).collect()
            }
            None => vec![None],
        };
        let mut kept = Vec::new();
        for run in runs {
            let mut c = ctx.clone();
            let i = run.as_ref().map(|(i, _)| *i);
            if let Some((i, item)) = run {
                c["file"] = ctx["files"].get(i).cloned().unwrap_or(Value::Null);
                c["item"] = item;
                c["index"] = json!(i);
            }
            let a = send(m, &st.request, &c, env, files).await.with_context(|| label(i))?;
            let body = a.json();
            let value = if st.save.is_empty() {
                summarize(&body)
            } else {
                let mut o = serde_json::Map::new();
                for (var, p) in &st.save {
                    let v = extensions::pointer(&body, p).cloned().ok_or_else(|| anyhow!("{}: the answer has nothing at {p}", label(i)))?;
                    o.insert(var.clone(), v);
                }
                Value::Object(o)
            };
            kept.push(value);
            last = Some(a);
        }
        let value = if st.for_each.is_some() { Value::Array(kept) } else { kept.pop().unwrap_or(Value::Null) };
        ctx[&st.name] = value.clone();
        saved.insert(st.name.clone(), value);
    }
    let text = match &map.text {
        Some(t) if t.contains("{{") => Some(extensions::fill(t, &ctx, env.clone())).filter(|t| !t.trim().is_empty()),
        Some(p) => last.as_ref().and_then(|a| extensions::pointer(&a.json(), p).and_then(|v| v.as_str().map(str::to_string))),
        None => None,
    };
    match last {
        Some(a) if map.file_url.is_some() || map.file_b64.is_some() => {
            let mut out = read_result(m, &a.client, a.json(), map).await?;
            out.text = text.or(out.text);
            Ok(out)
        }
        _ => Ok(ExecOut { json: Value::Object(saved), text, file: None }),
    }
}

/// Apply a result map: fetch or decode the file, pick the text, and leave a
/// response the model can read.
async fn read_result(m: &Manifest, client: &reqwest::Client, body: Value, map: &ResultMap) -> Result<ExecOut> {
    let mut json = body.clone();
    let mut file = None;
    if let Some(p) = &map.file_url {
        let u = extensions::pointer(&body, p).and_then(|v| v.as_str()).ok_or_else(|| anyhow!("{} answered without a file at {p}", m.name))?;
        let url = url::Url::parse(u).with_context(|| format!("{u} is not a URL"))?;
        if !extensions::host_allowed(&url, &m.hosts) {
            bail!("{} points at {}, which it may not call", m.name, url.host_str().unwrap_or(""));
        }
        let resp = client.get(url).send().await.context("downloading the result")?;
        if !resp.status().is_success() {
            bail!("downloading the result failed with {}", resp.status().as_u16());
        }
        let ctype = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").split(';').next().unwrap_or("").trim().to_string();
        let bytes = resp.bytes().await?.to_vec();
        let mime = if ctype.is_empty() || ctype == "application/octet-stream" { map.mime.clone().unwrap_or(ctype) } else { ctype };
        file = Some((bytes, mime));
    } else if let Some(p) = &map.file_b64 {
        let b64 = extensions::pointer(&body, p).and_then(|v| v.as_str()).ok_or_else(|| anyhow!("{} answered without file data at {p}", m.name))?;
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(b64.trim())).context("the file data is not base64")?;
        let mime = map.mime.clone().unwrap_or_else(|| sniff_mime(&bytes));
        file = Some((bytes, mime));
        // Do not hand the model megabytes of base64.
        let ptr = if p.starts_with('/') { p.clone() } else { format!("/{}", p.replace('.', "/")) };
        if let Some(slot) = json.pointer_mut(&ptr) {
            *slot = json!("<file>");
        }
    }
    let text = map.text.as_ref().and_then(|p| extensions::pointer(&body, p)).and_then(|v| v.as_str().map(str::to_string));
    if let Some((bytes, mime)) = &file {
        json = json!({ "bytes": bytes.len(), "mime": mime, "response": summarize(&json) });
    }
    Ok(ExecOut { json, text, file })
}

/// Keep a response readable: strings over 2 000 characters are cut.
fn summarize(v: &Value) -> Value {
    match v {
        Value::String(s) if s.chars().count() > 2000 => Value::String(format!("{}…", s.chars().take(2000).collect::<String>())),
        Value::Array(a) => Value::Array(a.iter().take(50).map(summarize).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), summarize(x))).collect()),
        other => other.clone(),
    }
}

pub fn sniff_mime(b: &[u8]) -> String {
    if b.starts_with(b"\x89PNG") {
        "image/png"
    } else if b.starts_with(b"\xFF\xD8") {
        "image/jpeg"
    } else if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if b.starts_with(b"GIF8") {
        "image/gif"
    } else if b.get(4..8) == Some(b"ftyp") {
        "video/mp4"
    } else if b.starts_with(b"ID3") || b.starts_with(b"\xFF\xFB") {
        "audio/mpeg"
    } else if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WAVE") {
        "audio/wav"
    } else if b.starts_with(b"<svg") || b.starts_with(b"<?xml") {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    }
    .to_string()
}

pub fn extension_for(mime: &str) -> &'static str {
    match mime.split(';').next().unwrap_or("").trim() {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "audio/mpeg" => "mp3",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/ogg" => "ogg",
        "application/pdf" => "pdf",
        _ => "bin",
    }
}

async fn command_exec(spec: &CommandSpec, args: &Value, map: &ResultMap, env: &(impl Fn(&str) -> Option<String> + Clone)) -> Result<ExecOut> {
    let program = extensions::fill(&spec.command, args, env.clone());
    if !on_path(&program) {
        bail!("the program {program} is not installed on this computer");
    }
    let mut cmd = tokio::process::Command::new(&program);
    for a in &spec.args {
        cmd.arg(extensions::fill(a, args, env.clone()));
    }
    cmd.env_clear();
    for k in ["PATH", "HOME", "LANG", "LC_ALL", "TMPDIR", "USER"] {
        if let Ok(v) = std::env::var(k) {
            cmd.env(k, v);
        }
    }
    for k in &spec.env {
        if let Some(v) = env(k) {
            cmd.env(k, v);
        }
    }
    if let Some(d) = &spec.cwd {
        cmd.current_dir(d);
    }
    cmd.stdin(if spec.stdin == "json" { std::process::Stdio::piped() } else { std::process::Stdio::null() });
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    let mut child = cmd.spawn().with_context(|| format!("starting {program}"))?;
    if spec.stdin == "json" {
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(args.to_string().as_bytes()).await;
            let _ = stdin.shutdown().await;
        }
    }
    let out = tokio::time::timeout(Duration::from_secs(spec.timeout_s.unwrap_or(120).clamp(1, 900)), child.wait_with_output()).await.map_err(|_| anyhow!("{program} took too long"))??;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("{program} failed ({}): {}", out.status, kiqr_core::profile::truncate_chars(err.trim(), 300));
    }
    let body: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| json!({ "text": String::from_utf8_lossy(&out.stdout).trim() }));
    let client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(60)).build()?;
    let m = Manifest { hosts: vec![], ..Default::default() };
    read_result(&m, &client, body, map).await
}

// ----------------------------------------------------------- running tools

/// Who asked: the chat or task behind the session, from the MCP server.
fn by_of(headers: &HeaderMap, args: &Value) -> String {
    args.get("by").and_then(|v| v.as_str()).map(str::to_string).or_else(|| headers.get("x-kiqr-by").and_then(|v| v.to_str().ok()).map(str::to_string)).unwrap_or_else(|| "seemo".into())
}

/// Put a produced file on an asset (the one named in `asset`, else a new one)
/// and answer with where it is.
pub(crate) async fn attach(app: &App, slug: &str, kind_default: &str, args: &Value, by: &str, bytes: Vec<u8>, mime: String, connection: &str, ability: &str) -> Result<Value, ApiError> {
    let ext = extension_for(&mime);
    let stem = args.get("name").and_then(|v| v.as_str()).map(|s| assets::slugify(s)).filter(|s| !s.is_empty()).unwrap_or_else(|| ability.to_string());
    let file = format!("{stem}.{ext}");
    let (kind, id) = match (args.get("asset").and_then(|a| a.get("type")).and_then(|v| v.as_str()), args.get("asset").and_then(|a| a.get("id")).and_then(|v| v.as_str())) {
        (Some(t), Some(i)) if !t.is_empty() && !i.is_empty() => (t.to_string(), i.to_string()),
        _ => {
            let title = args.get("title").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()).map(str::to_string).or_else(|| args.get("prompt").and_then(|v| v.as_str()).map(|p| kiqr_core::profile::truncate_chars(p, 60))).unwrap_or_else(|| format!("{ability} {}", chrono::Utc::now().format("%-d %b %H:%M")));
            let a = assets::create(&app.store, slug, assets::NewAsset { kind: kind_default.to_string(), title, by: Some(by.to_string()), ..Default::default() })?;
            crate::api::emit_saved(app, slug, &a, true);
            (a.kind, a.id)
        }
    };
    // The role the type names for an image: a cover on an article, media on a post.
    let role = assets::find_type(&app.store, &kind).map(|t| t.role_for(ext)).unwrap_or_else(|| "media".into());
    let a = assets::write_file(&app.store, slug, &kind, &id, &role, &file, &bytes)?;
    crate::api::emit_saved(app, slug, &a, false);
    app.bus.emit(Some(slug), EventKind::MediaCreated { ability: ability.to_string(), asset_type: kind.clone(), id: id.clone(), file: file.clone(), provider: connection.to_string() });
    Ok(json!({
        "ok": true,
        "asset": { "type": kind, "id": id, "title": a.title },
        "file": file,
        "role": role,
        "url": format!("/api/projects/{slug}/assets/{kind}/{id}/files/{file}"),
        "bytes": bytes.len(),
        "mime": mime,
        "connection": connection,
    }))
}

/// Preset sizes become width and height, and an orientation word, so every
/// connection sees the same arguments.
fn normalize_media_args(mut args: Value, kinds: &[kiqr_core::extensions::ChannelKind]) -> Value {
    if let Some(o) = args.as_object_mut() {
        let preset = o.get("size").and_then(|v| v.as_str()).and_then(|n| abilities::size_for(n, kinds));
        if let Some((w, h)) = preset {
            o.entry("width").or_insert(json!(w));
            o.entry("height").or_insert(json!(h));
        }
        let w = o.get("width").and_then(|v| v.as_u64());
        let h = o.get("height").and_then(|v| v.as_u64());
        if let (Some(w), Some(h)) = (w, h) {
            let orientation = if w > h { "landscape" } else if h > w { "portrait" } else { "square" };
            o.entry("orientation").or_insert(json!(orientation));
            o.entry("aspect").or_insert(json!(format!("{}:{}", w / gcd(w, h), h / gcd(w, h))));
        }
    }
    args
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a.max(1)
    } else {
        gcd(b, a % b)
    }
}

/// `POST /api/projects/{slug}/abilities/{ability}`: run the ability through
/// the connection that serves it.
async fn run_ability(State(app): State<App>, Path((slug, ability)): Path<(String, String)>, headers: HeaderMap, Json(args): Json<Value>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let a = abilities::find(&ability).ok_or_else(|| not_found(format!("no ability '{ability}'")))?;
    let cfg = app.config.read().await.clone();
    let (m, provide) = {
        let reg = app.registry.read().await;
        let off = project_off(&app, &slug);
        let Some((_, m)) = reg.provider(&slug, a.id, cfg.abilities.get(a.id).map(String::as_str), |m| !off.contains(&m.id) && ready_in(&app, Some(&slug), m)) else {
            return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("Nothing can \"{}\" yet. Install an ability for it under Setup → Abilities.", a.label.to_lowercase())));
        };
        (m.clone(), m.provides.get(a.id).cloned().unwrap_or_default())
    };
    let by = by_of(&headers, &args);
    let args = normalize_media_args(args, &crate::networks::installed(&app, &slug).await);
    let out = execute(&app, &slug, &m, &provide, Some(&a), None, &args, args["connection"].as_str()).await.map_err(|e| bad(format!("{e:#}")))?;
    // A file lands on the asset at once. The asset is what the user approves,
    // in the chat (where it shows as it will look) or in the library; an image
    // is never approved on its own.
    let mut v = match out.file {
        Some((bytes, mime)) if a.attach => attach(&app, &slug, a.asset_type, &args, &by, bytes, mime, &m.name, a.id).await?,
        Some((bytes, mime)) => json!({ "ok": true, "bytes": bytes.len(), "mime": mime, "connection": m.name }),
        None => json!({ "ok": true, "connection": m.name, "response": out.json }),
    };
    if let Some(t) = out.text {
        v["text"] = json!(t);
    }
    Ok(Json(v))
}

/// `POST /api/projects/{slug}/ext/{ext}/tools/{tool}`: one of a connection's own tools.
async fn run_ext_tool(State(app): State<App>, Path((slug, ext, tool)): Path<(String, String, String)>, headers: HeaderMap, Json(args): Json<Value>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let (m, t) = {
        let reg = app.registry.read().await;
        let (_, m) = reg.connection(&slug, &ext).ok_or_else(|| not_found(format!("no connection '{ext}'")))?;
        let t = match m.tools.iter().find(|t| t.name == tool) {
            Some(t) => t.clone(),
            None if m.transport == Transport::Mcp && (m.expose.iter().any(|e| e == "all") || m.expose.contains(&tool)) => extensions::ExtraTool { name: tool.clone(), ..Default::default() },
            None => return Err(not_found(format!("{} has no tool '{tool}'", m.name))),
        };
        (m.clone(), t)
    };
    if project_off(&app, &slug).contains(&m.id) {
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("{} is switched off for this project (Setup → Abilities).", m.name)));
    }
    if !ready_in(&app, Some(&slug), &m) {
        let missing = missing_in(&app, Some(&slug), &m);
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("{} is not set up yet (missing {}). Add it under Setup → Abilities.", m.name, missing.join(", "))));
    }
    let by = by_of(&headers, &args);
    let out = execute(&app, &slug, &m, &t.provide, None, Some(&t.name), &args, args["connection"].as_str()).await.map_err(|e| bad(format!("{e:#}")))?;
    let mut v = match out.file {
        Some((bytes, mime)) if t.provide.attach.unwrap_or(false) => attach(&app, &slug, "image", &args, &by, bytes, mime, &m.name, &t.name).await?,
        Some((bytes, mime)) => json!({ "ok": true, "bytes": bytes.len(), "mime": mime, "connection": m.name, "note": "the file was not kept; pass `asset` to put it on one" }),
        None => out.json,
    };
    if let Some(t) = out.text {
        if v.is_object() {
            v["text"] = json!(t);
        }
    }
    Ok(Json(v))
}
