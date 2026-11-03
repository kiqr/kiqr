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
use kiqr_core::conn::{Activity, Conn};
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{self, CommandSpec, Entry, HttpRequest, Manifest, Provide, Registry, ResultMap, Scope, Transport};
use kiqr_core::project::Project;
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
    missing_in(app, slug, m).is_empty() && (m.transport != Transport::Mcp || program_found(app, slug, m))
}

/// The program an MCP ability runs on is there: its own command, or the
/// command of the ability it names with `program:` (installed and on).
pub fn program_found(app: &App, slug: Option<&str>, m: &Manifest) -> bool {
    match program_of_sync(app, slug, m) {
        // The command, and everything it needs (a virtual display): an account is not ready without them.
        Some(p) => on_path(&p.command) && crate::programs::all_satisfied(&p),
        None => false,
    }
}

/// The manifest whose command an MCP ability runs: its own, or the one it
/// names with `program:`. None when that one is not installed.
pub fn program_of_sync(app: &App, slug: Option<&str>, m: &Manifest) -> Option<Manifest> {
    if m.program.is_empty() {
        return Some(m.clone());
    }
    let reg = app.registry.try_read().ok()?;
    reg.entry(slug, &m.program).and_then(|e| e.manifest.clone()).filter(|p| p.transport == Transport::Mcp && !p.command.is_empty())
}

pub async fn program_of(app: &App, slug: Option<&str>, m: &Manifest) -> Result<Manifest> {
    if m.program.is_empty() {
        return Ok(m.clone());
    }
    let reg = app.registry.read().await;
    reg.entry(slug, &m.program).and_then(|e| e.manifest.clone()).filter(|p| p.transport == Transport::Mcp && !p.command.is_empty()).ok_or_else(|| anyhow!("{} runs on '{}', which is not installed", m.name, m.program))
}

/// What an ability still needs for a project, in words ("a login for
/// x.com"): its required credentials the project has not granted it, or
/// granted but not complete. Without a project, every required one.
pub fn missing_in(app: &App, slug: Option<&str>, m: &Manifest) -> Vec<String> {
    match slug.filter(|s| !s.is_empty()) {
        Some(slug) => crate::creds::missing(app, slug, m),
        None => m.needs().iter().filter(|n| !n.optional).map(|n| crate::vault::describe(app, n)).collect(),
    }
}

/// Is a program there: a path that exists, or a name found on PATH.
pub fn on_path(cmd: &str) -> bool {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return false;
    }
    kiqr_core::process::find(cmd).is_some()
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

/// Account id → name, for labelling `ext:` groups in plain words.
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
        if let Some((e, m)) = reg.provider(slug, a.id, cfg.abilities.get(a.id).map(String::as_str), |m| !off.contains(&m.id) && ready_in(app, Some(slug), m)) {
            let cost = m.provides.get(a.id).and_then(|p| p.cost.as_deref());
            let mut spec = a.spec(&m.name, cost);
            // A design (`source`) always renders here, free, whichever generator is picked.
            if !e.is_core() {
                if let Some((_, core)) = core_provider(&reg, slug, a.id, &off) {
                    spec.description.push_str(&format!(" `source` (SVG or HTML) still renders through {}, free; `prompt` goes to {}{}.", core.name, m.name, cost.map(|c| format!(" ({c})")).unwrap_or_default()));
                }
            }
            out.push(with_credential_arg(spec, &crate::creds::granted(app, slug, &m.id)));
        }
    }
    for (_, m) in reg.connections(slug) {
        let g = Group::Ext(m.id.clone());
        if !allowed.contains(&g) {
            continue;
        }
        if m.transport == Transport::Mcp && !m.expose.is_empty() && ready_in(app, Some(slug), m) {
            // The server's own tools, raw, under the connection's group.
            let program = program_of_sync(app, Some(slug), m).unwrap_or_else(|| m.clone());
            match app.mcp.tools(app.store.home().root(), &program, &launch_for(app, slug, m).await).await {
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
        let conns = crate::creds::granted(app, slug, &m.id);
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
                // A tool that acts asks first, like a publish; reading runs at once.
                call: if t.acts() { CallSpec::Action { ext: m.id.clone(), tool: t.name.clone() } } else { CallSpec::Ext { ext: m.id.clone(), tool: t.name.clone() } },
                cost: t.provide.cost.clone(),
            });
            let last = out.pop().expect("just pushed");
            out.push(with_credential_arg(last, &conns));
        }
    }
    out
}

/// With more than one credential granted to an ability in the project, its
/// tools take `credential`: which one, by name; the first when left out.
fn with_credential_arg(mut spec: ToolSpec, conns: &[kiqr_core::vault::Credential]) -> ToolSpec {
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
    spec.input_schema["properties"]["credential"] = json!({ "type": "string", "enum": labels, "description": format!("Which one to act as. Leave it out for {}.", conns[0].label) });
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
        "credentials": m.needs(),
        "provides": m.provides.iter().map(|(id, p)| json!({ "ability": id, "label": abilities::find(id).map(|a| a.label).unwrap_or(""), "cost": p.cost })).collect::<Vec<_>>(),
        "tools": m.tools.iter().map(|t| json!({ "name": t.name, "label": t.label, "cost": t.provide.cost })).collect::<Vec<_>>(),
        "kinds": m.kinds.iter().map(|k| json!({ "kind": k.kind, "label": k.label, "name": k.display_name(), "section": k.section(), "target": k.target, "publishes": k.publishes() })).collect::<Vec<_>>(),
        "settings": m.settings,
        "hosts": m.hosts,
        "command": if m.transport == Transport::Mcp { Some(format!("{} {}", m.command, m.args.join(" ")).trim().to_string()) } else { None },
        "env": m.env,
        "program_found": if m.transport == Transport::Mcp { Some(program_found(app, None, m)) } else { None },
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
    let uses = |m: &Manifest| m.env.contains(&name.to_string()) || m.tools.iter().any(|t| t.provide.command.as_ref().is_some_and(|c| c.env.contains(&name.to_string())));
    let reg = app.registry.read().await;
    reg.entries.iter().filter_map(|e| e.manifest.as_ref()).any(uses) || extensions::shipped_all().iter().any(|(_, m)| uses(m))
}

/// Set (or, with an empty value, clear) one of KIQR's own keys. `slug` picks
/// the project's `.env`; none means the machine's. An ability's keys belong
/// to one of the project's accounts (`set_account_secret`). Never read back.
pub(crate) async fn set_secret(app: &App, slug: Option<&str>, name: &str, value: &str) -> Result<bool, ApiError> {
    let value = check_secret(app, name, value).await?;
    if !crate::api::SECRETS.contains(&name) {
        return Err(bad(format!("{name} belongs to an ability: its value lives in a credential in the Vault (Setup → Vault)")));
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


/// Whether a call still needs a setting: its request names it (`${NAME}` or `{{name}}`) somewhere the
/// call's own arguments do not cover. `{{repo|${GITHUB_REPO}}}` with a `repo` given does not need it.
fn uses_setting(shape: &str, key: &str, args: &serde_json::Map<String, Value>) -> bool {
    let var = format!("${{{}}}", key.to_ascii_uppercase());
    let mut rest = shape.to_string();
    for (name, v) in args {
        if v.is_null() || v.as_str() == Some("") {
            continue;
        }
        rest = rest.replace(&format!("{{{{{name}|{var}}}}}"), "");
    }
    rest.contains(&var) || rest.contains(&format!("{{{{{key}}}")) || rest.contains(&format!("{{{{{key}|"))
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
///
/// An account is shared by every ability of its service, so a call through
/// one waits its turn when the ability is `exclusive` (a browser session),
/// is refused when it would repeat an action on the same target or go over
/// the service's `daily_cap`, and is logged on the account when it acts.
pub async fn execute(app: &App, slug: &str, m: &Manifest, provide: &Provide, ability: Option<&Ability>, tool: Option<&str>, args: &Value, conn: Option<&str>) -> Result<ExecOut> {
    // The project's settings for this ability (a property, a site); `pick` honours the credential they name.
    let setting = (!m.settings.is_empty()).then(|| Project::load(&app.store, slug).ok().and_then(|p| p.ability_settings.get(&m.id).cloned())).flatten();
    let named = conn.or(args.get("credential").and_then(Value::as_str));
    let mut conn = crate::creds::pick(app, slug, m, named).await?;
    let mut args = args.clone();
    if let Some(o) = args.as_object_mut() {
        // The settings fill what the call left out; a call may name its own (another property).
        for (k, v) in setting.iter().flat_map(|s| s.values.iter()) {
            o.entry(k.clone()).or_insert_with(|| v.clone());
        }
        let shape = serde_json::to_string(provide).unwrap_or_default();
        let missing: Vec<&str> = m
            .settings
            .iter()
            .filter(|(k, spec)| !spec["optional"].as_bool().unwrap_or(false) && o.get(*k).is_none_or(|v| v.is_null() || v.as_str() == Some("")) && uses_setting(&shape, k, o))
            .map(|(k, spec)| spec["label"].as_str().unwrap_or(k))
            .collect();
        if !missing.is_empty() {
            bail!("{} has no {} picked yet; the user picks it on the ability (Setup → Abilities)", m.name, missing.join(", ").to_lowercase());
        }
    }
    // The channel the call happens in decides: a role it switched off is refused, whoever asks.
    // And a credential on a hosted program that is not signed in does nothing, reading included
    // (it would read the sign-in page).
    let role = role_of(m, tool);
    let kinds: Vec<(Manifest, extensions::Kind)> = m.kinds.iter().map(|k| (m.clone(), k.clone())).collect();
    let spot = match &conn {
        Some(c) => crate::channels::spot_of(&app.store, slug, c.id(), &crate::creds::name_of(c.primary()), &kinds, tool.unwrap_or_default(), &args, false).ok().flatten(),
        None => None,
    };
    if let Some(s) = &spot {
        if s.mode(&role) == kiqr_core::channels::Mode::Off {
            bail!("Seemo may not {role} in {}; the user can allow it on the channel", s.label);
        }
    }
    // A call in a website channel is made for that site: its address is `${SITE_URL}`.
    if let (Some(c), Some(ch)) = (conn.as_mut(), spot.as_ref().and_then(|s| s.channel.as_ref()).filter(|ch| ch.is_website())) {
        c.site = Some(crate::channels::site_url(ch));
    }
    if let Some(c) = &conn {
        if m.transport == Transport::Mcp {
            let st = crate::creds::state(app, slug, c.primary()).await;
            if !st.ready() {
                bail!("{}; the user sees to it in the Vault before Seemo works there", crate::creds::state_text(c.label(), &st));
            }
        }
    }
    let _turn = match (&conn, m.exclusive) {
        (Some(c), true) => Some(turn(c.id()).await),
        _ => None,
    };
    // The channel the call happens in, for the log.
    let channel = spot.and_then(|s| s.channel).map(|e| e.id).unwrap_or_default();
    let act = conn.as_ref().and_then(|c| action(slug, m, tool, &args, &target_env(app, slug, m, c), channel));
    if let (Some(c), Some(a)) = (&conn, &act) {
        if let Err((why, e)) = check_action(app, m, c, a) {
            app.bus.emit(Some(slug), EventKind::ActionRefused { credential: c.id().to_string(), ability: m.id.clone(), why: why.to_string(), message: e.to_string() });
            return Err(e);
        }
    }
    let out = run(app, slug, m, provide, ability, tool, &args, conn.as_ref()).await?;
    if let (Some(c), Some(mut a)) = (conn, act) {
        a.url = out.text.clone().filter(|t| t.starts_with("http")).or_else(|| ["url", "link"].iter().find_map(|k| out.json.get(*k).and_then(|u| u.as_str()).map(str::to_string)));
        if let Err(e) = c.record(app.store.home(), &a) {
            tracing::warn!(credential = %c.id(), "could not log an action: {e:#}");
        }
        app.bus.emit(Some(slug), EventKind::CredentialActed { credential: c.id().to_string(), ability: a.ability, role: a.role, target: a.target, channel: a.channel, url: a.url });
    }
    Ok(out)
}

/// The values a key can take, read through the account once it is signed in
/// (a key's `choices`: the properties it can see), as `{value, label}`.
pub(crate) async fn key_choices(app: &App, slug: &str, m: &Manifest, conn: &Conn, name: &str) -> Result<Vec<Value>> {
    let ch = extensions::choices_of(m, name).ok_or_else(|| anyhow!("{name} has no list to choose from"))?;
    let m = m.clone();
    if m.transport != Transport::Http {
        bail!("{} reads choices over http only", m.name);
    }
    let env = kiqr_core::conn::env_for(app.store.home(), slug, Some(conn), &m, &crate::creds::settings_of(app, slug, &m.id));
    let mut seen = std::collections::BTreeSet::new();
    let mut found = Vec::new();
    for page in 1..=ch.pages() {
        let out = http_exec(&m, &ch.request, &json!({ "page": page }), &ResultMap::default(), &env, &Files::default()).await?;
        let items: Vec<Value> = extensions::pointer_all(&out.json, &ch.list).into_iter().cloned().collect();
        if items.is_empty() {
            break;
        }
        found.extend(items);
    }
    Ok(found
        .iter()
        .filter_map(|item| {
            let raw = extensions::pointer(item, &ch.value)?;
            let raw = raw.as_str().map(str::to_string).unwrap_or_else(|| raw.to_string());
            let value = raw.strip_prefix(ch.strip.as_str()).unwrap_or(&raw).to_string();
            let label = (!ch.label.is_empty()).then(|| extensions::pointer(item, &ch.label).and_then(|l| l.as_str()).map(str::to_string)).flatten().unwrap_or_else(|| value.clone());
            seen.insert(value.clone()).then(|| json!({ "value": value, "label": label }))
        })
        .collect())
}

/// What an `action` approval shows before the yes: which ability's tool does
/// what, through which account, with which arguments, and on what.
pub(crate) async fn action_preview(app: &App, slug: &str, payload: &Value) -> Result<Value, ApiError> {
    let id = payload["ability"].as_str().unwrap_or_default();
    let tool = payload["tool"].as_str().unwrap_or_default();
    let m = {
        let reg = app.registry.read().await;
        reg.connection(slug, id).map(|(_, m)| m.clone())
    }
    .ok_or_else(|| bad(format!("the ability '{id}' is not installed or is off here")))?;
    if project_off(app, slug).contains(&m.id) {
        return Err(bad(format!("{} is switched off for this project", m.name)));
    }
    let t = m.tools.iter().find(|t| t.name == tool).filter(|t| t.acts()).ok_or_else(|| bad(format!("{} has no tool '{tool}' that acts", m.name)))?;
    let args = payload["args"].clone();
    let conn = crate::creds::pick(app, slug, &m, args["credential"].as_str()).await.map_err(|e| bad(e.to_string()))?.ok_or_else(|| bad(format!("{} acts as nobody here yet; allow it a credential first", m.name)))?;
    // Where it happens: the channel's mode decides; somewhere that is no channel asks.
    let kinds: Vec<(Manifest, extensions::Kind)> = m.kinds.iter().map(|k| (m.clone(), k.clone())).collect();
    let spot = crate::channels::spot_of(&app.store, slug, conn.id(), &crate::creds::name_of(conn.primary()), &kinds, tool, &args, true).map_err(|e| bad(e.to_string()))?;
    let mode = spot.as_ref().map(|s| s.mode(&t.role)).unwrap_or(kiqr_core::channels::Mode::Ask);
    if mode == kiqr_core::channels::Mode::Off {
        return Err(bad(format!("Seemo may not {} in {}; the user can allow it on the channel", t.role, spot.as_ref().map(|s| s.label.as_str()).unwrap_or("this channel"))));
    }
    let shown: Vec<Value> = args.as_object().map(|o| o.iter().filter(|(k, _)| k.as_str() != "credential" && k.as_str() != "by").map(|(k, v)| json!({ "key": k, "value": v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()) })).collect()).unwrap_or_default();
    Ok(json!({
        "ability": m.id,
        "ability_name": m.name,
        "tool": t.name,
        "label": if t.label.is_empty() { t.name.clone() } else { t.label.clone() },
        "description": t.description,
        "role": t.role,
        "target": extensions::fill(&t.target, &args, target_env(app, slug, &m, &conn)).trim(),
        "credential": conn.id(),
        "account": crate::creds::name_of(conn.primary()),
        "args": args,
        "fields": shown,
        // Where it happens; somewhere new becomes a channel with the yes.
        "spot": spot.as_ref().map(|s| s.view()),
        // On its own only where the channel lets Seemo act on its own for this role: the card is the record.
        "asks_first": mode != kiqr_core::channels::Mode::Auto,
    }))
}

/// Run an approved action: the ability's tool through the account the card named.
pub(crate) async fn do_action(app: &App, slug: &str, preview: &Value) -> Result<Value, ApiError> {
    let id = preview["ability"].as_str().unwrap_or_default();
    let tool = preview["tool"].as_str().unwrap_or_default();
    let m = {
        let reg = app.registry.read().await;
        reg.connection(slug, id).map(|(_, m)| m.clone())
    }
    .ok_or_else(|| bad(format!("the ability '{id}' is not installed or is off here")))?;
    let t = m.tools.iter().find(|t| t.name == tool).cloned().ok_or_else(|| bad(format!("{} has no tool '{tool}'", m.name)))?;
    let out = execute(app, slug, &m, &t.provide, None, Some(tool), &preview["args"], preview["credential"].as_str()).await.map_err(|e| bad(format!("{e:#}")))?;
    // A spot the user said yes to is one Seemo goes to again: it becomes a channel, published to as this credential.
    if preview["spot"]["new"].as_bool() == Some(true) {
        if let Some(c) = preview["credential"].as_str().and_then(|id| app.vault.get(id).ok().flatten()) {
            crate::channels::add_spot(app, slug, &m, &c, &preview["spot"], "seemo").await;
        }
    }
    let url = out.text.clone().filter(|x| x.starts_with("http")).or_else(|| ["html_url", "url", "link"].iter().find_map(|k| out.json.get(*k).and_then(|u| u.as_str()).map(str::to_string)));
    Ok(json!({ "done": true, "url": url, "text": out.text, "result": out.json, "note": "Done. Say in one line what was done where, with the link." }))
}

/// Credentials being driven right now, by id: machine-wide, so two projects never drive one browser profile at once.
static TURNS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>> = std::sync::LazyLock::new(Default::default);

/// Wait until no other exclusive call is using the credential.
pub(crate) async fn turn(id: &str) -> tokio::sync::OwnedMutexGuard<()> {
    let lock = TURNS.lock().unwrap_or_else(|e| e.into_inner()).entry(id.to_string()).or_default().clone();
    lock.lock_owned().await
}

/// What a call does on its account, when it does more than read: the tool's
/// `role` (or `publish` for a channel kind's publish tool) and its `target`.
fn action(slug: &str, m: &Manifest, tool: Option<&str>, args: &Value, env: &impl Fn(&str) -> Option<String>, channel: String) -> Option<Activity> {
    let role = role_of(m, tool);
    if role == "read" {
        return None;
    }
    let t = tool.and_then(|n| m.tools.iter().find(|t| t.name == n));
    let target = t.map(|t| extensions::fill(&t.target, args, env)).unwrap_or_default().trim().to_string();
    Some(Activity { at: chrono::Utc::now(), project: slug.to_string(), ability: m.id.clone(), role, tool: tool.unwrap_or_default().to_string(), target, channel, url: None })
}

/// What a call of a tool does on its account: the tool's `role`, `publish` for a kind's publish tool, else reading.
pub(crate) fn role_of(m: &Manifest, tool: Option<&str>) -> String {
    let t = tool.and_then(|n| m.tools.iter().find(|t| t.name == n));
    match t.map(|t| t.role.as_str()).filter(|r| !r.is_empty()) {
        Some(r) => r.to_string(),
        None if tool.is_some_and(|n| m.kinds.iter().any(|k| k.publish == n)) => "publish".into(),
        None => "read".into(),
    }
}

/// What a target may name (`${GITHUB_REPO}`): settings and plain fields, never a secret.
/// Targets are logged and shown, so a token, a key or a password stays out even when a manifest names it.
pub(crate) fn target_env(app: &App, slug: &str, m: &Manifest, c: &Conn) -> impl Fn(&str) -> Option<String> + Clone {
    let env = kiqr_core::conn::env_for(app.store.home(), slug, Some(c), m, &crate::creds::settings_of(app, slug, &m.id));
    let secret = c.secret_names();
    move |k: &str| {
        let u = k.to_ascii_uppercase();
        let looks = ["TOKEN", "SECRET", "PASSWORD", "PASS", "KEY", "FILE"].iter().any(|w| u.contains(w));
        if looks || secret.iter().any(|s| s == k) || kiqr_core::provider::is_core_key(k) { None } else { env(k) }
    }
}

/// Refuse an action already done on the same target as this credential (in
/// any project), or one over the ability's `daily_cap` for the credential today.
fn check_action(app: &App, m: &Manifest, c: &Conn, a: &Activity) -> std::result::Result<(), (&'static str, anyhow::Error)> {
    let home = app.store.home();
    if !a.target.is_empty() {
        if let Some(p) = c.acted_on(home, &a.role, &a.target) {
            return Err(("duplicate", anyhow!("already done: {} ({}) on {} as {} at {}; not doing it twice", p.role, p.ability, a.target, c.label(), p.at.format("%Y-%m-%d %H:%M UTC"))));
        }
    }
    if let Some(cap) = m.daily_cap {
        if c.actions_today(home) >= cap as usize {
            return Err(("cap", anyhow!("{} has done its {cap} actions for today (the limit counts every project that uses it); it can act again tomorrow", c.label())));
        }
    }
    Ok(())
}

/// One call through its transport with no checks or log of its own: for a
/// send to a list, where the caller holds the account and logs the whole
/// send as one action.
pub(crate) async fn run_through(app: &App, slug: &str, m: &Manifest, provide: &Provide, ability: Option<&Ability>, tool: Option<&str>, args: &Value, conn: Option<&Conn>) -> Result<ExecOut> {
    run(app, slug, m, provide, ability, tool, args, conn).await
}

/// One call through its transport, after the account's checks.
async fn run(app: &App, slug: &str, m: &Manifest, provide: &Provide, ability: Option<&Ability>, tool: Option<&str>, args: &Value, conn: Option<&Conn>) -> Result<ExecOut> {
    let mut args = provide.mapped_args(args);
    if let Some(o) = args.as_object_mut() {
        o.remove("credential");
    }
    let files = call_files(app, slug, provide, &mut args)?;
    let env = kiqr_core::conn::env_for(app.store.home(), slug, conn, m, &crate::creds::settings_of(app, slug, &m.id));
    // A credential's values go only to its own domains, whatever the manifest's hosts allow (`send`).
    let guard: CredHosts = std::sync::Arc::new(conn.map(|c| c.env().into_iter().map(|(k, (_, d))| (k, d)).collect()).unwrap_or_default());
    CRED_HOSTS.scope(guard, run_with(app, slug, m, provide, ability, tool, args, files, conn, env)).await
}

/// Which names carry a credential's value, and the domains each may go to.
type CredHosts = std::sync::Arc<std::collections::BTreeMap<String, Vec<String>>>;
tokio::task_local! {
    static CRED_HOSTS: CredHosts;
}

#[allow(clippy::too_many_arguments)]
async fn run_with(app: &App, slug: &str, m: &Manifest, provide: &Provide, ability: Option<&Ability>, tool: Option<&str>, args: Value, files: Files, conn: Option<&Conn>, env: impl Fn(&str) -> Option<String> + Clone) -> Result<ExecOut> {
    let args = &args;
    let files = &files;
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
        // A goal a model reaches in the browser, page by page.
        Transport::Mcp if provide.hand.is_some() => crate::hand::run(app, slug, m, provide.hand.as_ref().unwrap(), args, conn, &env).await,
        Transport::Mcp if !provide.steps.is_empty() => {
            let program = program_of(app, Some(slug), m).await?;
            let launch = mcp_env(app, slug, m, conn, &env);
            let (json, text) = mcp_steps(app, &program, &launch, m, &provide.steps, &args, &provide.result, &env).await?;
            Ok(ExecOut { json, text, file: None })
        }
        Transport::Mcp => {
            let server_tool = provide.tool.clone().or_else(|| tool.map(str::to_string)).ok_or_else(|| anyhow!("{} does not say which of its tools to call", m.name))?;
            let call_args = match &provide.args {
                Some(t) => extensions::fill_value(t, &args, &env),
                None => args.clone(),
            };
            // A URL argument of an exposed tool stays on the ability's hosts.
            for name in &m.url_args {
                if let Some(u) = call_args.get(name).and_then(Value::as_str) {
                    check_host(m, u, &env)?;
                }
            }
            let program = program_of(app, Some(slug), m).await?;
            let r = app.mcp.call(app.store.home().root(), &program, &mcp_env(app, slug, m, conn, &env), &server_tool, &call_args).await?;
            let mut out = crate::mcp_host::read_result(&r);
            out.text = crate::redact(app, &out.text);
            if out.is_error {
                bail!("{}: {}", m.name, kiqr_core::profile::truncate_chars(out.text.trim(), 300));
            }
            let mut json = out.structured.clone().map(|v| crate::redact_value(app, v)).unwrap_or_else(|| json!({ "text": out.text }));
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

/// The launch of a hosted program as the credential a project's calls of it go through.
pub async fn launch_for(app: &App, slug: &str, m: &Manifest) -> crate::mcp_host::Launch {
    let conn = crate::creds::pick(app, slug, m, None).await.ok().flatten();
    let env = kiqr_core::conn::env_for(app.store.home(), slug, conn.as_ref(), m, &crate::creds::settings_of(app, slug, &m.id));
    mcp_env(app, slug, m, conn.as_ref(), &env)
}

/// The names a program is given: the ability's credential names, its own `env`, and its commands' `env`.
fn program_names(m: &Manifest) -> Vec<String> {
    let mut v = m.credential_env();
    v.extend(m.env.iter().cloned());
    for c in m.tools.iter().filter_map(|t| t.provide.command.as_ref()).chain(m.provides.values().filter_map(|p| p.command.as_ref())) {
        v.extend(c.env.iter().cloned());
    }
    v.retain(|n| !n.is_empty() && !kiqr_core::provider::is_core_key(n));
    v
}

/// What a hosted program is started with: the credential (one server per
/// credential and program, shared by every ability and project on that
/// program), the values of the names the ability and the program ask for,
/// the program's `env_set` filled, and the credential's own session folder to work in.
pub fn mcp_env(app: &App, slug: &str, m: &Manifest, conn: Option<&Conn>, env: &impl Fn(&str) -> Option<String>) -> crate::mcp_host::Launch {
    let program = program_of_sync(app, Some(slug), m).unwrap_or_else(|| m.clone());
    let home = app.store.home();
    let dir = conn.map(|c| c.data_dir(home));
    let project_dir = home.project_dir(slug);
    let mut names = program_names(m);
    names.extend(program_names(&program));
    names.sort();
    names.dedup();
    let mut vars: Vec<(String, String)> = names.into_iter().filter_map(|k| env(&k).map(|v| (k, v))).collect();
    let dir_text = dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default();
    let project_text = project_dir.display().to_string();
    for (name, template) in program.env_set.iter().chain(m.env_set.iter()) {
        let lookup = |k: &str| match k {
            "KIQR_ACCOUNT_DIR" => Some(dir_text.clone()),
            "KIQR_PROJECT_DIR" => Some(project_text.clone()),
            _ => env(k),
        };
        let v = extensions::fill(template, &Value::Null, lookup);
        if !v.contains("${") {
            vars.retain(|(n, _)| n != name);
            vars.push((name.clone(), v));
        }
    }
    let lookup = |k: &str| match k {
        "KIQR_ACCOUNT_DIR" => Some(dir_text.clone()),
        "KIQR_PROJECT_DIR" => Some(project_text.clone()),
        _ => env(k),
    };
    let warmup = program.warmup.iter().map(|st| (st.call.clone(), st.args.as_ref().map(|t| extensions::fill_value(t, &Value::Null, &lookup)).unwrap_or_else(|| json!({})))).collect();
    crate::mcp_host::Launch { key: crate::mcp_host::Launch::key_for(slug, conn.map(|c| c.id()), &program.id), vars, dir, project_dir: Some(project_dir), warmup }
}

/// How long a step waits for a busy program (a long text typed at a person's pace) before it gives up.
const BUSY_WAIT: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// One call on a hosted program, made again while the program says it is busy with an earlier action
/// (its manifest's `busy_when`), up to `BUSY_WAIT`.
pub(crate) async fn mcp_call_when_free(app: &App, program: &Manifest, launch: &crate::mcp_host::Launch, call: &str, args: &Value) -> Result<crate::mcp_host::McpOut> {
    let started = std::time::Instant::now();
    loop {
        let (out, said) = match app.mcp.call(app.store.home().root(), program, launch, call, args).await {
            Ok(r) => {
                let out = crate::mcp_host::read_result(&r);
                let said = if out.is_error { out.text.clone() } else { String::new() };
                (Ok(out), said)
            }
            Err(e) => {
                let said = format!("{e:#}");
                (Err(e), said)
            }
        };
        let busy = !said.is_empty() && program.busy_when.iter().any(|w| said.contains(w.as_str()));
        if !busy || started.elapsed() > BUSY_WAIT {
            return out;
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

/// Steps on a hosted program, in order: each calls one of the program's
/// tools with `args` filled from the arguments and what earlier steps saved
/// (`{{name.var}}`); `when` skips one; `for_each` runs one per entry of an
/// earlier step's list (or `files`). A step's answer is its structured
/// content, else `{text}`; `save` points into it. The result is the last
/// answer, or, for a `text` that is a template, anything saved.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn mcp_steps(app: &App, program: &Manifest, launch: &crate::mcp_host::Launch, m: &Manifest, steps: &[extensions::Step], args: &Value, map: &ResultMap, env: &(impl Fn(&str) -> Option<String> + Clone)) -> Result<(Value, Option<String>)> {
    // A step names the folders the program works in as the program does: `${KIQR_PROJECT_DIR}`, `${KIQR_ACCOUNT_DIR}`.
    let folder = |d: &Option<std::path::PathBuf>| d.as_ref().map(|d| d.display().to_string());
    let (account_dir, project_dir) = (folder(&launch.dir), folder(&launch.project_dir));
    let outer = env.clone();
    let env = &move |k: &str| match k {
        "KIQR_ACCOUNT_DIR" => account_dir.clone(),
        "KIQR_PROJECT_DIR" => project_dir.clone(),
        _ => outer(k),
    };
    let mut ctx = if args.is_object() { args.clone() } else { json!({}) };
    let mut last = Value::Null;
    for st in steps {
        if st.skipped(&ctx, env) {
            let skipped = if st.for_each.is_some() { json!([]) } else { Value::Null };
            ctx[&st.name] = skipped;
            continue;
        }
        let runs: Vec<Option<(usize, Value)>> = match st.for_each.as_deref() {
            Some(list) => {
                let items = extensions::lookup(&ctx, list).and_then(|v| v.as_array().cloned()).unwrap_or_default();
                if items.len() > extensions::MAX_EACH {
                    bail!("step '{}': {} entries, at most {}", st.name, items.len(), extensions::MAX_EACH);
                }
                items.into_iter().enumerate().map(Some).collect()
            }
            None => vec![None],
        };
        let mut kept = Vec::new();
        for run in runs {
            let mut c = ctx.clone();
            if let Some((i, item)) = run {
                c["file"] = ctx["files"].get(i).cloned().unwrap_or(Value::Null);
                c["item"] = item;
                c["index"] = json!(i);
            }
            let call_args = match &st.args {
                Some(t) => extensions::fill_value(t, &c, env),
                None => json!({}),
            };
            // A URL argument stays on the ability's hosts.
            for name in &m.url_args {
                if let Some(u) = call_args.get(name).and_then(Value::as_str) {
                    check_host(m, u, env)?;
                }
            }
            let out = mcp_call_when_free(app, program, launch, &st.call, &call_args).await.with_context(|| format!("step '{}'", st.name))?;
            if out.is_error {
                bail!("step '{}': {}", st.name, kiqr_core::profile::truncate_chars(out.text.trim(), 300));
            }
            let body = out.structured.clone().unwrap_or_else(|| json!({ "text": out.text }));
            let value = if st.save.is_empty() {
                body.clone()
            } else {
                let mut o = serde_json::Map::new();
                for (var, p) in &st.save {
                    let v = extensions::pointer(&body, p).cloned().ok_or_else(|| anyhow!("step '{}': the answer has nothing at {p}", st.name))?;
                    o.insert(var.clone(), v);
                }
                Value::Object(o)
            };
            kept.push(value);
            last = body;
        }
        let value = if st.for_each.is_some() { Value::Array(kept) } else { kept.pop().unwrap_or(Value::Null) };
        ctx[&st.name] = value;
    }
    let text = match &map.text {
        Some(t) if t.contains("{{") => Some(extensions::fill(t, &ctx, env.clone())).filter(|t| !t.trim().is_empty()),
        Some(p) => extensions::pointer(&last, p).and_then(|v| v.as_str().map(str::to_string)),
        None => last.get("text").and_then(Value::as_str).map(str::to_string),
    };
    Ok((last, text))
}

/// A URL an ability's call names has to be on one of its `hosts` (a `${VAR}`
/// host is the account's own site, read from its keys); an ability with no
/// hosts may go anywhere.
pub(crate) fn check_host(m: &Manifest, url: &str, env: &dyn Fn(&str) -> Option<String>) -> Result<()> {
    if m.hosts.is_empty() {
        return Ok(());
    }
    let u = url::Url::parse(url).map_err(|_| anyhow!("'{url}' is not a URL"))?;
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    let ok = m.hosts.iter().any(|h| {
        let h = extensions::fill(h.trim(), &Value::Null, env);
        // A host, or the account's site as a URL.
        let h = url::Url::parse(&h).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or(h).to_ascii_lowercase();
        !h.is_empty() && !h.starts_with("${") && (host == h || host.ends_with(&format!(".{h}")))
    });
    if !ok {
        bail!("{} may not open {host}; it stays on {}", m.name, m.hosts.join(", "));
    }
    Ok(())
}

/// A credential's value goes only to its own domains: a request that names
/// one (`${FAL_KEY}` anywhere in it) to a host outside them is refused, even
/// when the manifest's `hosts` allow the host.
fn check_credential_hosts(m: &Manifest, req: &HttpRequest, host: &str) -> Result<()> {
    let Ok(map) = CRED_HOSTS.try_with(|m| m.clone()) else { return Ok(()) };
    if map.is_empty() {
        return Ok(());
    }
    let text = serde_json::to_string(req).unwrap_or_default();
    for (name, domains) in map.iter() {
        if text.contains(&format!("${{{name}}}")) && !domains.iter().any(|d| kiqr_core::vault::same_site(d, host)) {
            bail!("{} may not send {name} to {host}: the credential is for {} only (add {host} to its domains in the Vault if it belongs there)", m.name, domains.join(", "));
        }
    }
    Ok(())
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

/// A template filled, or the refusal that names the variable it needs. Only the template is read
/// for variables: what an argument holds (a file whose code says `${price}`) is never looked at.
fn filled(template: &str, args: &Value, env: &(impl Fn(&str) -> Option<String> + Clone), what: &str) -> Result<String> {
    extensions::fill_checked(template, args, env.clone()).map_err(|name| anyhow!("{what} needs ${{{name}}} which is not set; add it under Setup → Accounts on the account, or under Setup → Abilities for KIQR's own keys"))
}

fn filled_value(template: &Value, args: &Value, env: &(impl Fn(&str) -> Option<String> + Clone), what: &str) -> Result<Value> {
    extensions::fill_value_checked(template, args, env).map_err(|name| anyhow!("{what} needs ${{{name}}} which is not set; add it under Setup → Accounts on the account, or under Setup → Abilities for KIQR's own keys"))
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
    let url_s = filled(&req.url, args, env, "the request URL")?;
    let mut url = url::Url::parse(url_s.trim()).with_context(|| format!("{url_s} is not a URL"))?;
    if !extensions::host_allowed(&url, &extensions::hosts_for(m, env)) {
        bail!("{} may not call {}", m.name, url.host_str().unwrap_or(""));
    }
    check_credential_hosts(m, req, url.host_str().unwrap_or(""))?;
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
        let v = filled(v, args, env, &format!("the header {k}"))?;
        has_ctype |= k.eq_ignore_ascii_case("content-type");
        r = r.header(k, v);
    }
    // Added after the manifest's own headers, so no manifest can leave it out: the daemon refuses its few human-only routes (the factory reset) to anything carrying it.
    r = r.header(crate::reset::AGENT_HEADER, m.id.as_str());
    if let Some(pair) = &req.basic_auth {
        let user = filled(pair.first().map(String::as_str).unwrap_or(""), args, env, "the user name")?;
        let pass = pair.get(1).map(|p| filled(p, args, env, "the password")).transpose()?;
        r = r.basic_auth(user, pass);
    }
    if let Some(body) = &req.json {
        let b = filled_value(body, args, env, "the request body")?;
        r = r.json(&b);
    } else if !req.form.is_empty() {
        let mut pairs = Vec::new();
        for (k, v) in &req.form {
            let v = filled(v, args, env, &format!("the form field {k}"))?;
            pairs.push((k.clone(), v));
        }
        r = r.form(&pairs);
    } else if !req.multipart.is_empty() {
        let mut form = reqwest::multipart::Form::new();
        for part in &req.multipart {
            if let Some(v) = &part.value {
                let v = filled(v, args, env, &format!("the part {}", part.name))?;
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
    // A vendor's raw text (GitHub's `application/vnd.github.raw`) is text, not a file.
    let raw_text = a.ctype.starts_with("application/vnd.") && a.ctype.contains("raw") && std::str::from_utf8(&a.bytes).is_ok();
    if !raw_text && !a.ctype.starts_with("application/json") && !a.ctype.starts_with("text/") && !a.bytes.is_empty() && map.file_url.is_none() && map.file_b64.is_none() {
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
        if st.skipped(&ctx, env) {
            let skipped = if st.for_each.is_some() { json!([]) } else { Value::Null };
            ctx[&st.name] = skipped.clone();
            saved.insert(st.name.clone(), skipped);
            continue;
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

pub use kiqr_core::fetch::{extension_for, sniff_mime};

async fn command_exec(spec: &CommandSpec, args: &Value, map: &ResultMap, env: &(impl Fn(&str) -> Option<String> + Clone)) -> Result<ExecOut> {
    let program = extensions::fill(&spec.command, args, env.clone());
    if !on_path(&program) {
        bail!("the program {program} is not installed on this computer");
    }
    let mut cmd = kiqr_core::process::command(&program);
    for a in &spec.args {
        cmd.arg(extensions::fill(a, args, env.clone()));
    }
    cmd.env_clear();
    for k in kiqr_core::process::BASE_ENV {
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
fn normalize_media_args(mut args: Value, kinds: &[kiqr_core::extensions::Kind]) -> Value {
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

/// Is this call a design to render rather than something to generate: the
/// `image` ability with a `source` and no `prompt`?
pub fn renders_locally(ability: &str, args: &Value) -> bool {
    let has = |k: &str| args.get(k).and_then(Value::as_str).is_some_and(|s| !s.trim().is_empty());
    ability == "image" && has("source") && !has("prompt")
}

/// The core connection that provides an ability (the renderer behind `image`), when it is on.
fn core_provider<'a>(reg: &'a kiqr_core::extensions::Registry, slug: &str, ability: &str, off: &[String]) -> Option<(&'a kiqr_core::extensions::Entry, &'a Manifest)> {
    reg.connections(slug).into_iter().find(|(e, m)| e.is_core() && m.provides.contains_key(ability) && !off.contains(&m.id))
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
        // A design always renders here: the built-in provider takes `source` whichever generator is picked.
        let local = if renders_locally(a.id, &args) { core_provider(&reg, &slug, a.id, &off) } else { None };
        let Some((_, m)) = local.or_else(|| reg.provider(&slug, a.id, cfg.abilities.get(a.id).map(String::as_str), |m| !off.contains(&m.id) && ready_in(&app, Some(&slug), m))) else {
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

#[cfg(test)]
mod credential_host_tests {
    use super::*;

    #[test]
    fn a_setting_is_needed_only_where_the_call_does_not_cover_it() {
        let args = |v: Value| v.as_object().cloned().unwrap();
        let shape = r#"{"url":"https://api.github.com/repos/{{repo|${GITHUB_REPO}}}/git/trees/HEAD"}"#;
        assert!(!uses_setting(shape, "github_repo", &args(json!({ "repo": "a/b" }))));
        assert!(uses_setting(shape, "github_repo", &args(json!({}))));
        assert!(uses_setting(r#"{"url":"https://api.github.com/repos/${GITHUB_REPO}/releases"}"#, "github_repo", &args(json!({ "repo": "a/b" }))));
        assert!(!uses_setting(r#"{"url":"https://api.github.com/user/repos"}"#, "github_repo", &args(json!({}))));
    }

    #[tokio::test]
    async fn a_credential_value_goes_only_to_its_domains() {
        let m = extensions::parse_manifest("id: t\nname: T\ntransport: http\nhosts: [fal.run, other.example]\ncredentials:\n  - { template: api_key, env: { FAL_KEY: key } }\n").unwrap();
        let mut req = HttpRequest { url: "https://other.example/x".into(), ..Default::default() };
        req.headers.insert("Authorization".into(), "Key ${FAL_KEY}".into());
        let map: CredHosts = std::sync::Arc::new([("FAL_KEY".to_string(), vec!["fal.ai".to_string(), "fal.run".to_string()])].into_iter().collect());
        let (to_other, to_own) = CRED_HOSTS.scope(map, async { (check_credential_hosts(&m, &req, "other.example"), check_credential_hosts(&m, &req, "queue.fal.run")) }).await;
        assert!(to_other.unwrap_err().to_string().contains("may not send FAL_KEY"), "the manifest's hosts do not widen the credential's");
        assert!(to_own.is_ok());
        // A request that names no credential goes anywhere the manifest allows.
        let plain = HttpRequest { url: "https://other.example/x".into(), ..Default::default() };
        let map: CredHosts = std::sync::Arc::new([("FAL_KEY".to_string(), vec!["fal.ai".to_string()])].into_iter().collect());
        assert!(CRED_HOSTS.scope(map, async { check_credential_hosts(&m, &plain, "other.example") }).await.is_ok());
    }
}
