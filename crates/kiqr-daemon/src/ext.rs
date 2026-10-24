//! Connections, abilities and skills in the daemon: the registry of folders
//! under `skills/`, the merged tool list a session gets, the executor that
//! turns a manifest's `provides` into a request, a command or built-in Rust,
//! and the routes behind Settings → Connections and Seemo → Skills.

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
        .route("/api/skills", get(list_skills).post(install_skill))
        .route("/api/skills/{name}", get(get_skill).put(put_skill).delete(delete_skill))
        .route("/api/projects/{slug}/skills", get(list_project_skills).post(install_project_skill))
        .route("/api/projects/{slug}/skills/{name}", get(get_project_skill).put(put_project_skill).delete(delete_project_skill))
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
    *app.registry.write().await = Registry::load(app.store.home());
}

/// Is a connection usable right now: keys present, program found.
pub fn ready(m: &Manifest) -> bool {
    extensions::missing_keys(m, provider::has_env).is_empty() && (m.transport != Transport::Mcp || on_path(&m.command))
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
    let mut out: Vec<Group> = Group::BUILTIN.to_vec();
    for (_, m) in reg.connections(slug) {
        if !m.tools.is_empty() || (m.transport == Transport::Mcp && !m.expose.is_empty()) {
            out.push(Group::Ext(m.id.clone()));
        }
    }
    out
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

/// The tools a session gets: built-ins in `allowed`, the abilities that have a
/// ready connection, and the connections' own tools.
pub async fn tool_specs(app: &App, slug: &str, allowed: &[Group]) -> Vec<ToolSpec> {
    let reg = app.registry.read().await;
    let cfg = app.config.read().await.clone();
    let mut out = tools::builtin_specs(allowed);
    for a in abilities::all() {
        if !allowed.contains(&a.group) {
            continue;
        }
        if let Some((_, m)) = reg.provider(slug, a.id, cfg.abilities.get(a.id).map(String::as_str), ready) {
            let cost = m.provides.get(a.id).and_then(|p| p.cost.as_deref());
            out.push(a.spec(&m.name, cost));
        }
    }
    for (_, m) in reg.connections(slug) {
        let g = Group::Ext(m.id.clone());
        if !allowed.contains(&g) {
            continue;
        }
        if m.transport == Transport::Mcp && !m.expose.is_empty() && ready(m) {
            // The server's own tools, raw, under the connection's group.
            match app.mcp.tools(app.store.home().root(), m).await {
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
        }
    }
    out
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

fn connection_view(e: &Entry, m: &Manifest) -> Value {
    let missing = extensions::missing_keys(m, provider::has_env);
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
        "ready": ready(m),
        "missing": missing,
        "auth": m.auth.iter().map(|a| json!({ "env": a.env, "label": a.label, "help": a.help, "optional": a.optional, "set": provider::has_env(&a.env) })).collect::<Vec<_>>(),
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
            (Some(m), None) => list.push(connection_view(e, m)),
            (_, Some(err)) => broken.push(json!({ "folder": e.name, "scope": e.scope, "error": err })),
            _ => {}
        }
    }
    let abilities: Vec<Value> = abilities::all()
        .into_iter()
        .map(|a| {
            let configured = cfg.abilities.get(a.id).cloned();
            let candidates: Vec<Value> = reg.entries.iter().filter_map(|e| e.manifest.as_ref().filter(|m| m.provides.contains_key(a.id) && e.error.is_none())).map(|m| json!({ "id": m.id, "name": m.name, "ready": ready(m) })).collect();
            let active = reg.provider("", a.id, configured.as_deref(), ready).map(|(_, m)| json!({ "id": m.id, "name": m.name }));
            json!({ "id": a.id, "label": a.label, "tool": a.tool, "configured": configured, "active": active, "connections": candidates })
        })
        .collect();
    Ok(Json(json!({ "connections": list, "broken": broken, "abilities": abilities })))
}

async fn get_extension(State(app): State<App>, Path(id): Path<String>) -> ApiResult<Value> {
    let reg = app.registry.read().await;
    let found = reg.entries.iter().find_map(|e| e.manifest.as_ref().filter(|m| m.id == id && e.error.is_none()).map(|m| connection_view(e, m)));
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

/// Remove a user-installed connection (its folder, skill included).
async fn delete_extension(State(app): State<App>, Path(id): Path<String>, Query(q): Query<ScopeQuery>) -> Result<StatusCode, ApiError> {
    let (dir, name, scope) = {
        let reg = app.registry.read().await;
        let scope = q.project.clone().map(Scope::Project).unwrap_or(Scope::Global);
        let e = reg.entries.iter().find(|e| e.scope == scope && e.manifest.as_ref().is_some_and(|m| m.id == id)).ok_or_else(|| not_found(format!("no connection '{id}'")))?;
        if e.builtin {
            return Err(bad("built-in connections cannot be removed; leave their keys empty to keep them off"));
        }
        (e.dir.clone(), e.name.clone(), e.scope.clone())
    };
    remove_folder(&app, &dir, &name, &scope).await?;
    app.mcp.stop(&id).await;
    app.bus.emit(None, EventKind::ExtensionRemoved { id });
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_folder(app: &App, dir: &std::path::Path, name: &str, scope: &Scope) -> Result<(), ApiError> {
    std::fs::remove_dir_all(dir).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("could not remove {}: {e}", dir.display())))?;
    let label = match scope {
        Scope::Global => "skills".to_string(),
        Scope::Project(s) => format!("skills({s})"),
    };
    app.store.commit(&format!("{label}: remove {name}")).await?;
    app.bus.emit(None, EventKind::SkillDeleted { name: name.to_string(), scope: scope_id(scope) });
    reload(app).await;
    Ok(())
}

fn scope_id(s: &Scope) -> String {
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

/// Set (or, with an empty value, clear) one secret in `.env`. Only names a
/// connection or provider declares; the value is never read back.
async fn put_secret(State(app): State<App>, Path(name): Path<String>, Json(b): Json<SecretBody>) -> ApiResult<Value> {
    if !extensions::valid_env_name(&name) {
        return Err(bad("a secret name is upper-case letters, digits and underscores"));
    }
    let known = {
        let reg = app.registry.read().await;
        crate::api::SECRETS.contains(&name.as_str())
            || reg.entries.iter().filter_map(|e| e.manifest.as_ref()).any(|m| m.auth.iter().any(|a| a.env == name) || m.env.contains(&name) || m.tools.iter().any(|t| t.provide.command.as_ref().is_some_and(|c| c.env.contains(&name))))
    };
    if !known {
        return Err(bad(format!("no connection or provider uses {name}")));
    }
    let value = b.value.trim().to_string();
    if value.contains('\n') {
        return Err(bad("a secret is one line"));
    }
    write_env(&app.store.home().env_file(), &name, &value).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if value.is_empty() {
        std::env::remove_var(&name);
    } else {
        std::env::set_var(&name, &value);
    }
    app.bus.emit(None, EventKind::SecretSet { name: name.clone(), set: !value.is_empty() });
    Ok(Json(json!({ "name": name, "set": !value.is_empty() })))
}

/// Replace or append `NAME=value` in the env file, keeping everything else and mode 0600.
fn write_env(path: &std::path::Path, name: &str, value: &str) -> Result<()> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    let mut done = false;
    for l in existing.lines() {
        let t = l.trim_start();
        let is_ours = t.strip_prefix(name).is_some_and(|r| r.starts_with('=')) || t.strip_prefix("export ").and_then(|r| r.trim_start().strip_prefix(name)).is_some_and(|r| r.starts_with('='));
        let is_commented = t.strip_prefix('#').map(|r| r.trim_start()).and_then(|r| r.strip_prefix(name)).is_some_and(|r| r.starts_with('='));
        if is_ours || (is_commented && !done && !value.is_empty()) {
            if !done && !value.is_empty() {
                lines.push(format!("{name}={value}"));
                done = true;
            }
            continue;
        }
        lines.push(l.to_string());
    }
    if !done && !value.is_empty() {
        lines.push(format!("{name}={value}"));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

// ------------------------------------------------------------------ skills

fn skill_view(e: &Entry) -> Value {
    let meta = e.skill.clone().unwrap_or_default();
    json!({
        "name": e.name,
        "title": meta.name,
        "description": meta.description,
        "abilities": meta.abilities,
        "channels": meta.channels,
        "scope": e.scope,
        "builtin": e.builtin,
        "has_skill": e.skill.is_some(),
        "pipeline": crate::agent::PIPELINE_SKILLS.contains(&e.name.as_str()),
        "connection": e.manifest.as_ref().map(|m| json!({ "id": m.id, "name": m.name, "ready": ready(m) })),
        "error": e.error,
        "folder": e.dir.display().to_string(),
    })
}

/// Every skill folder, global and per project. `?project=` lists what that project sees.
async fn list_skills(State(app): State<App>, Query(q): Query<ScopeQuery>) -> ApiResult<Vec<Value>> {
    let reg = app.registry.read().await;
    let entries: Vec<&Entry> = match &q.project {
        Some(slug) => reg.visible(slug),
        None => reg.entries.iter().collect(),
    };
    Ok(Json(entries.into_iter().map(skill_view).collect()))
}

async fn list_project_skills(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Vec<Value>> {
    let _ = Project::load(&app.store, &slug)?;
    let reg = app.registry.read().await;
    Ok(Json(reg.visible(&slug).into_iter().map(skill_view).collect()))
}

fn valid_skill_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && !name.starts_with(['-', '.'])
}

/// A skill's text: the project's copy, then the user's, then the built-in one.
fn skill_text(app: &App, slug: Option<&str>, name: &str) -> Result<String, ApiError> {
    if !valid_skill_name(name) {
        return Err(bad("a skill name is letters, digits and dashes"));
    }
    if let Some(s) = slug {
        if let Ok(t) = std::fs::read_to_string(app.store.home().project_skills_dir(s).join(name).join("SKILL.md")) {
            return Ok(t);
        }
    }
    if let Ok(t) = std::fs::read_to_string(app.store.home().skills_dir().join(name).join("SKILL.md")) {
        return Ok(t);
    }
    kiqr_core::extract::builtin_skill(name)
        .map(str::to_string)
        .or_else(|| extensions::BUILTIN_EXTENSIONS.iter().find(|(n, _, _)| *n == name).and_then(|(_, s, _)| s.map(str::to_string)))
        .ok_or_else(|| not_found(format!("no skill '{name}'")))
}

async fn get_skill(State(app): State<App>, Path(name): Path<String>) -> Result<String, ApiError> {
    skill_text(&app, None, &name)
}

async fn get_project_skill(State(app): State<App>, Path((slug, name)): Path<(String, String)>) -> Result<String, ApiError> {
    skill_text(&app, Some(&slug), &name)
}

#[derive(Deserialize)]
struct SkillBody {
    text: String,
}

pub(crate) async fn save_skill(app: &App, scope: Scope, name: &str, text: &str) -> ApiResult<Value> {
    if !valid_skill_name(name) {
        return Err(bad("a skill name is letters, digits and dashes"));
    }
    let text = text.trim();
    if text.is_empty() {
        return Err(bad("a skill needs some text"));
    }
    let dir = match &scope {
        Scope::Global => app.store.home().skills_dir(),
        Scope::Project(s) => {
            Project::load(&app.store, s)?;
            app.store.home().project_skills_dir(s)
        }
    };
    let body = if text.starts_with("---") { format!("{text}\n") } else { format!("---\nname: {name}\ndescription: {}\n---\n\n{text}\n", first_line(text)) };
    app.store.write_text(&dir.join(name).join("SKILL.md"), &body)?;
    let label = match &scope {
        Scope::Global => "skills".to_string(),
        Scope::Project(s) => format!("skills({s})"),
    };
    app.store.commit(&format!("{label}: {name}")).await?;
    reload(app).await;
    app.bus.emit(None, EventKind::SkillSaved { name: name.to_string(), scope: scope_id(&scope) });
    let reg = app.registry.read().await;
    let view = reg.entries.iter().find(|e| e.name == name && e.scope == scope).map(skill_view).unwrap_or(Value::Null);
    Ok(Json(view))
}

fn first_line(text: &str) -> String {
    let l = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim_start_matches('#').trim();
    kiqr_core::profile::truncate_chars(l, 140)
}

async fn put_skill(State(app): State<App>, Path(name): Path<String>, Json(b): Json<SkillBody>) -> ApiResult<Value> {
    save_skill(&app, Scope::Global, &name, &b.text).await
}

async fn put_project_skill(State(app): State<App>, Path((slug, name)): Path<(String, String)>, Json(b): Json<SkillBody>) -> ApiResult<Value> {
    save_skill(&app, Scope::Project(slug), &name, &b.text).await
}

async fn delete_skill_in(app: &App, scope: Scope, name: &str) -> Result<StatusCode, ApiError> {
    let (dir, builtin) = {
        let reg = app.registry.read().await;
        let e = reg.entries.iter().find(|e| e.name == name && e.scope == scope).ok_or_else(|| not_found(format!("no skill '{name}'")))?;
        (e.dir.clone(), e.builtin)
    };
    if builtin && scope == Scope::Global {
        return Err(bad("built-in skills cannot be removed; edit the copy instead"));
    }
    remove_folder(app, &dir, name, &scope).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_skill(State(app): State<App>, Path(name): Path<String>) -> Result<StatusCode, ApiError> {
    delete_skill_in(&app, Scope::Global, &name).await
}

async fn delete_project_skill(State(app): State<App>, Path((slug, name)): Path<(String, String)>) -> Result<StatusCode, ApiError> {
    delete_skill_in(&app, Scope::Project(slug), &name).await
}

#[derive(Deserialize)]
struct Install {
    /// A folder on this machine, or a git URL.
    source: String,
    /// Folder name to install under; default: the manifest id, else the source's last path part.
    #[serde(default)]
    name: Option<String>,
}

/// Copy a skill folder (SKILL.md and/or extension.yaml, templates) into `skills/`.
async fn install_into(app: &App, scope: Scope, b: Install) -> Result<(StatusCode, Json<Value>), ApiError> {
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
    let has_skill = from.join("SKILL.md").exists();
    let manifest_text = std::fs::read_to_string(from.join("extension.yaml")).ok();
    if !has_skill && manifest_text.is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(bad("the folder has neither SKILL.md nor extension.yaml"));
    }
    let manifest = match &manifest_text {
        Some(t) => Some(extensions::parse_manifest(t).map_err(|e| bad(format!("extension.yaml: {e:#}")))?),
        None => None,
    };
    let name = b.name.clone().filter(|n| !n.trim().is_empty()).or_else(|| manifest.as_ref().map(|m| m.id.clone())).or_else(|| from.file_name().map(|n| n.to_string_lossy().trim_end_matches(".git").to_string())).unwrap_or_default();
    if !valid_skill_name(&name) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(bad(format!("'{name}' is not a usable folder name")));
    }
    let dest_root = match &scope {
        Scope::Global => app.store.home().skills_dir(),
        Scope::Project(s) => {
            Project::load(&app.store, s)?;
            app.store.home().project_skills_dir(s)
        }
    };
    let dest = dest_root.join(&name);
    if dest.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(ApiError(StatusCode::CONFLICT, format!("a skill named '{name}' is already installed; remove it first")));
    }
    copy_dir(&from, &dest).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("could not copy: {e}")))?;
    let _ = std::fs::remove_dir_all(&tmp);
    let label = match &scope {
        Scope::Global => "skills".to_string(),
        Scope::Project(s) => format!("skills({s})"),
    };
    app.store.commit(&format!("{label}: install {name}")).await?;
    reload(app).await;
    app.bus.emit(None, EventKind::SkillSaved { name: name.clone(), scope: scope_id(&scope) });
    if let Some(m) = &manifest {
        app.bus.emit(None, EventKind::ExtensionInstalled { id: m.id.clone(), name: m.name.clone() });
    }
    let reg = app.registry.read().await;
    let view = reg.entries.iter().find(|e| e.name == name && e.scope == scope).map(skill_view).unwrap_or(Value::Null);
    Ok((StatusCode::CREATED, Json(view)))
}

/// Copy files (not symlinks, not `.git`), at most 20 MB in all.
fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> Result<()> {
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

async fn install_skill(State(app): State<App>, Json(b): Json<Install>) -> Result<(StatusCode, Json<Value>), ApiError> {
    install_into(&app, Scope::Global, b).await
}

async fn install_project_skill(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<Install>) -> Result<(StatusCode, Json<Value>), ApiError> {
    install_into(&app, Scope::Project(slug), b).await
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

fn env_value(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

/// Run one `provides` entry or extra tool of a connection.
pub async fn execute(app: &App, slug: &str, m: &Manifest, provide: &Provide, ability: Option<&Ability>, tool: Option<&str>, args: &Value) -> Result<ExecOut> {
    let args = provide.mapped_args(args);
    match m.transport {
        Transport::Http => {
            let req = provide.request.as_ref().ok_or_else(|| anyhow!("the connection {} has no request for this", m.name))?;
            http_exec(m, req, &args, &provide.result).await
        }
        Transport::Command => {
            let cmd = provide.command.as_ref().ok_or_else(|| anyhow!("the connection {} has no command for this", m.name))?;
            command_exec(cmd, &args, &provide.result).await
        }
        Transport::Mcp => {
            let server_tool = provide.tool.clone().or_else(|| tool.map(str::to_string)).ok_or_else(|| anyhow!("the connection {} does not say which of its tools to call", m.name))?;
            let call_args = match &provide.args {
                Some(t) => extensions::fill_value(t, &args, &env_value),
                None => args.clone(),
            };
            let r = app.mcp.call(app.store.home().root(), m, &server_tool, &call_args).await?;
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
        Transport::Builtin => crate::builtin::run(app, slug, m, ability, tool, &args).await,
    }
}

fn no_leftover(s: &str, what: &str) -> Result<()> {
    if let Some(i) = s.find("${") {
        let end = s[i..].find('}').map(|e| i + e + 1).unwrap_or(s.len());
        bail!("{what} needs {} which is not set; add it under Settings → Connections", &s[i..end]);
    }
    Ok(())
}

async fn http_exec(m: &Manifest, req: &HttpRequest, args: &Value, map: &ResultMap) -> Result<ExecOut> {
    let url_s = extensions::fill(&req.url, args, env_value);
    no_leftover(&url_s, "the request URL")?;
    let mut url = url::Url::parse(&url_s).with_context(|| format!("{url_s} is not a URL"))?;
    if !extensions::host_allowed(&url, &m.hosts) {
        bail!("{} may not call {}", m.name, url.host_str().unwrap_or(""));
    }
    for (k, v) in &req.query {
        let v = extensions::fill(v, args, env_value);
        if !v.is_empty() {
            url.query_pairs_mut().append_pair(k, &v);
        }
    }
    let client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(req.timeout_s.unwrap_or(120).clamp(5, 900))).user_agent(kiqr_core::fetch::USER_AGENT).build()?;
    let method = reqwest::Method::from_bytes(req.method.to_ascii_uppercase().as_bytes())?;
    let mut r = client.request(method, url.clone());
    for (k, v) in &req.headers {
        let v = extensions::fill(v, args, env_value);
        no_leftover(&v, &format!("the header {k}"))?;
        r = r.header(k, v);
    }
    if let Some(body) = &req.json {
        let b = extensions::fill_value(body, args, &env_value);
        no_leftover(&b.to_string(), "the request body")?;
        r = r.json(&b);
    }
    let resp = r.send().await.with_context(|| format!("calling {}", url.host_str().unwrap_or("")))?;
    let status = resp.status();
    let ctype = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let bytes = resp.bytes().await?;
    if !status.is_success() {
        let text = String::from_utf8_lossy(&bytes);
        bail!("{} answered {}: {}", m.name, status.as_u16(), kiqr_core::profile::truncate_chars(text.trim(), 300));
    }
    // A file straight back.
    if !ctype.starts_with("application/json") && !ctype.starts_with("text/") && !bytes.is_empty() && map.file_url.is_none() && map.file_b64.is_none() {
        return Ok(ExecOut { json: json!({ "bytes": bytes.len(), "mime": ctype }), text: None, file: Some((bytes.to_vec(), map.mime.clone().unwrap_or(ctype))) });
    }
    let body: Value = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({ "text": String::from_utf8_lossy(&bytes) }));
    read_result(m, &client, body, map).await
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

async fn command_exec(spec: &CommandSpec, args: &Value, map: &ResultMap) -> Result<ExecOut> {
    let program = extensions::fill(&spec.command, args, env_value);
    if !on_path(&program) {
        bail!("the program {program} is not installed on this computer");
    }
    let mut cmd = tokio::process::Command::new(&program);
    for a in &spec.args {
        cmd.arg(extensions::fill(a, args, env_value));
    }
    cmd.env_clear();
    for k in ["PATH", "HOME", "LANG", "LC_ALL", "TMPDIR", "USER"] {
        if let Ok(v) = std::env::var(k) {
            cmd.env(k, v);
        }
    }
    for k in &spec.env {
        if let Some(v) = env_value(k) {
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
    let a = assets::write_file(&app.store, slug, &kind, &id, "media", &file, &bytes)?;
    crate::api::emit_saved(app, slug, &a, false);
    app.bus.emit(Some(slug), EventKind::MediaCreated { ability: ability.to_string(), asset_type: kind.clone(), id: id.clone(), file: file.clone(), provider: connection.to_string() });
    Ok(json!({
        "ok": true,
        "asset": { "type": kind, "id": id, "title": a.title },
        "file": file,
        "url": format!("/api/projects/{slug}/assets/{kind}/{id}/files/{file}"),
        "bytes": bytes.len(),
        "mime": mime,
        "connection": connection,
    }))
}

/// Preset sizes become width and height, and an orientation word, so every
/// connection sees the same arguments.
fn normalize_media_args(mut args: Value) -> Value {
    if let Some(o) = args.as_object_mut() {
        let preset = o.get("size").and_then(|v| v.as_str()).and_then(abilities::size_preset);
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
        let Some((_, m)) = reg.provider(&slug, a.id, cfg.abilities.get(a.id).map(String::as_str), ready) else {
            return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("Nothing provides \"{}\" yet. Add a connection under Settings → Connections.", a.label)));
        };
        (m.clone(), m.provides.get(a.id).cloned().unwrap_or_default())
    };
    let by = by_of(&headers, &args);
    let args = normalize_media_args(args);
    let out = execute(&app, &slug, &m, &provide, Some(&a), None, &args).await.map_err(|e| bad(format!("{e:#}")))?;
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
    if !ready(&m) {
        let missing = extensions::missing_keys(&m, provider::has_env);
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("{} is not connected yet (missing {}). Add it under Settings → Connections.", m.name, missing.join(", "))));
    }
    let by = by_of(&headers, &args);
    let out = execute(&app, &slug, &m, &t.provide, None, Some(&t.name), &args).await.map_err(|e| bad(format!("{e:#}")))?;
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
