//! Abilities as the user sees them, behind Setup → Abilities: one list of
//! what Seemo can do here (core abilities that are code, shipped and
//! installed folders, the ones the user taught), and the marketplace below
//! it, from an index kiqr.cloud hosts (cached, with the shipped list as the
//! offline fallback). Installing clones the ability's repository into
//! `abilities/` (or writes a shipped one); keys go to the machine's `.env` or
//! a project's. Nothing but writing, the renderer and web reading is on
//! without a choice; core abilities switch off but never leave.

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use kiqr_core::abilities;
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{self, AuthKey, Manifest, Scope, Transport};
use kiqr_core::project::Project;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/abilities", get(list))
        .route("/api/projects/{slug}/abilities", get(list_project))
        .route("/api/projects/{slug}/abilities/brief", get(brief))
        .route("/api/abilities/install", post(install_source))
        .route("/api/abilities/refresh-index", post(refresh_index))
        .route("/api/abilities/{id}/avatar", get(avatar))
        .route("/api/abilities/{id}/install", post(install))
        .route("/api/abilities/{id}/update", post(update))
        .route("/api/abilities/{id}/pause", post(pause))
        .route("/api/abilities/{id}/resume", post(resume))
        .route("/api/abilities/{id}", axum::routing::delete(remove))
        .route("/api/projects/{slug}/abilities/{id}/pause", post(project_pause))
        .route("/api/projects/{slug}/abilities/{id}/resume", post(project_resume))
}

/// A project switches an ability off, or on again, for itself.
async fn set_project_paused(app: &App, slug: &str, id: &str, paused: bool) -> ApiResult<Value> {
    let mut p = Project::load(&app.store, slug)?;
    let known = extensions::is_core_id(id) || !app.registry.read().await.installed(id).is_empty();
    if !known {
        return Err(ApiError(StatusCode::NOT_FOUND, format!("no ability '{id}'")));
    }
    p.abilities_off.retain(|x| x != id);
    if paused {
        p.abilities_off.push(id.to_string());
    }
    p.save(&app.store)?;
    app.store.commit(&format!("project({slug}): ability {id} {}", if paused { "off" } else { "on" })).await?;
    app.bus.emit(Some(slug), EventKind::ExtensionPaused { id: id.to_string(), paused });
    Ok(Json(json!({ "id": id, "project": slug, "paused": paused })))
}

async fn project_pause(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    set_project_paused(&app, &slug, &id, true).await
}

async fn project_resume(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    set_project_paused(&app, &slug, &id, false).await
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

/// One entry of the index: enough to show a card and to install.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexItem {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub version: String,
    /// A git repository to clone; none for a shipped ability.
    pub repo: Option<String>,
    /// A folder inside that repository, when the repository holds several abilities (the marketplace's own does).
    pub path: Option<String>,
    pub homepage: String,
    pub categories: Vec<String>,
    pub icon: String,
    /// A URL to a 64×64 mark, or empty.
    #[serde(default)]
    pub avatar: String,
    pub cost: Option<String>,
    /// Ability contracts it provides: image, video, music, email.
    pub provides: Vec<String>,
    /// Channel kinds it publishes to: a kind, or what the card and the brand
    /// crawl need before it is installed (its name, where profiles live).
    #[serde(deserialize_with = "index_channels")]
    pub channels: Vec<IndexChannel>,
    /// Metrics sources it reads.
    #[serde(default)]
    pub sources: Vec<String>,
    pub auth: Vec<AuthKey>,
    /// How the keys can be got with one button (the manifest's `connect`).
    pub connect: Option<kiqr_core::connect::Connect>,
    /// `official` (KIQR's own), `verified`, `community`.
    pub trust: String,
    pub transport: String,
    /// The vendor: the GitHub user or organisation behind the repository (`kiqr` for KIQR's own).
    pub author: String,
    #[serde(skip_deserializing)]
    pub shipped: bool,
}

/// A channel kind as the index lists it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexChannel {
    pub kind: String,
    pub name: String,
    pub icon: String,
    pub profile_hosts: Vec<String>,
}

impl IndexChannel {
    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.kind
        } else {
            &self.name
        }
    }
}

/// `channels: [wordpress]` or `channels: [{kind: linkedin, name: LinkedIn, profile_hosts: [linkedin.com]}]`.
fn index_channels<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<IndexChannel>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum One {
        Kind(String),
        Full(IndexChannel),
    }
    Ok(Option::<Vec<One>>::deserialize(d)?.unwrap_or_default().into_iter().map(|o| match o {
        One::Kind(kind) => IndexChannel { kind, ..Default::default() },
        One::Full(c) => c,
    }).collect())
}

/// `github.com/<author>/<repo>` → `author`.
fn author_from_repo(repo: &str) -> Option<String> {
    let u = url::Url::parse(repo).ok()?;
    let mut parts = u.path().trim_matches('/').split('/');
    let a = parts.next()?.trim_end_matches(".git");
    (!a.is_empty()).then(|| a.to_lowercase())
}

fn author_of(item: &IndexItem) -> String {
    if !item.author.trim().is_empty() {
        return item.author.trim().to_lowercase();
    }
    if let Some(a) = item.repo.as_deref().and_then(author_from_repo) {
        return a;
    }
    if item.shipped { "KIQR".into() } else { "you".into() }
}

/// A folder name or a few of them, nothing that climbs out.
fn safe_path(p: &str) -> bool {
    !p.is_empty() && p.len() <= 120 && p.split('/').all(|seg| !seg.is_empty() && seg != "." && seg != ".." && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'))
}

/// Clone a repository (shallow) and return the folder to install from, plus the temp dir to remove.
async fn fetch_repo(app: &App, repo: &str, path: Option<&str>) -> Result<(std::path::PathBuf, std::path::PathBuf), ApiError> {
    let tmp = app.store.home().root().join("tmp").join("install").join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&tmp).map_err(anyhow::Error::from)?;
    let status = tokio::process::Command::new("git").args(["clone", "--depth", "1", "--quiet", repo, "repo"]).current_dir(&tmp).status().await.map_err(|err| bad(format!("could not run git: {err}")))?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(bad(format!("git could not clone {repo}")));
    }
    let from = match path {
        Some(p) => tmp.join("repo").join(p),
        None => tmp.join("repo"),
    };
    if !from.join("extension.yaml").exists() && !from.join("SKILL.md").exists() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(bad(format!("{repo}{} has no extension.yaml or SKILL.md", path.map(|p| format!(" ({p})")).unwrap_or_default())));
    }
    Ok((from, tmp))
}

fn source_note(repo: &str, path: Option<&str>) -> String {
    match path {
        Some(p) => format!("{repo}#{p}\n"),
        None => format!("{repo}\n"),
    }
}

/// Where the vendor name links: the repository the ability came from (its
/// folder inside it when the index says), KIQR's own repository for what
/// ships with it, and only as a last resort the vendor's page.
/// Where the avatar is: the installed folder's file, served by the daemon;
/// else what the manifest or the index names as a URL.
fn avatar_url(item: &IndexItem, m: Option<&Manifest>, e: Option<&extensions::Entry>, stamp: u64) -> Option<String> {
    let named = m.map(|m| m.avatar.trim().to_string()).filter(|a| !a.is_empty()).unwrap_or_else(|| item.avatar.trim().to_string());
    if named.is_empty() {
        return None;
    }
    if named.starts_with("http://") || named.starts_with("https://") {
        // The index's fetch time as a version, so a browser refetches a changed mark at the same URL.
        return Some(format!("{named}{}v={stamp}", if named.contains('?') { "&" } else { "?" }));
    }
    let e = e?;
    if !e.dir.join(&named).is_file() {
        return None;
    }
    let project = match &e.scope {
        extensions::Scope::Project(s) => format!("?project={s}"),
        _ => String::new(),
    };
    Some(format!("/api/abilities/{}/avatar{project}", e.id()))
}

fn author_url(item: &IndexItem, author: &str) -> Option<String> {
    if author == "you" {
        return None;
    }
    if let Some(repo) = item.repo.as_deref().filter(|r| url::Url::parse(r).is_ok()) {
        let root = repo.trim_end_matches('/').trim_end_matches(".git").to_string();
        return Some(match item.path.as_deref().map(|p| p.trim_matches('/')).filter(|p| !p.is_empty()) {
            Some(path) => format!("{root}/tree/HEAD/{path}"),
            None => root,
        });
    }
    if item.shipped || author.eq_ignore_ascii_case("kiqr") {
        return Some("https://github.com/kiqr/kiqr".into());
    }
    Some(format!("https://github.com/{author}"))
}

fn item_from_manifest(m: &Manifest) -> IndexItem {
    let cost = m.provides.values().find_map(|p| p.cost.clone()).or_else(|| m.tools.iter().find_map(|t| t.provide.cost.clone()));
    IndexItem {
        id: m.id.clone(),
        name: m.name.clone(),
        summary: m.summary.clone(),
        version: m.version.clone(),
        repo: None,
        path: None,
        homepage: m.homepage.clone(),
        categories: if m.categories.is_empty() { default_categories(m) } else { m.categories.clone() },
        icon: m.icon.clone(),
        avatar: m.avatar.clone(),
        cost,
        provides: m.provides.keys().cloned().collect(),
        channels: m.channels.iter().map(|c| IndexChannel { kind: c.kind.clone(), name: c.display_name().to_string(), icon: c.icon.clone(), profile_hosts: c.profile_hosts.clone() }).collect(),
        sources: m.sources.keys().cloned().collect(),
        auth: m.auth.clone(),
        connect: m.connect.clone(),
        trust: "official".into(),
        transport: serde_json::to_value(m.transport).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
        author: String::new(),
        shipped: false,
    }
}

fn default_categories(m: &Manifest) -> Vec<String> {
    let mut out: Vec<String> = m
        .provides
        .keys()
        .map(|k| match k.as_str() {
            "image" => "images",
            "video" => "video",
            "music" => "music",
            "email" => "email",
            other => other,
        })
        .map(str::to_string)
        .collect();
    if !m.channels.is_empty() {
        out.push("publishing".into());
    }
    if !m.tools.is_empty() || (m.transport == Transport::Mcp && !m.expose.is_empty()) {
        out.push("tools".into());
    }
    out.sort();
    out.dedup();
    out
}

fn core_item(c: &extensions::CoreAbility) -> IndexItem {
    IndexItem {
        id: c.id.into(),
        name: c.name.into(),
        summary: c.summary.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        repo: None,
        path: None,
        homepage: c.homepage.into(),
        categories: c.categories.iter().map(|s| s.to_string()).collect(),
        icon: c.icon.into(),
        avatar: String::new(),
        cost: Some(c.cost.into()),
        provides: vec![],
        channels: vec![],
        sources: vec![],
        auth: c.auth.iter().map(|(env, label, help)| AuthKey { env: env.to_string(), label: label.to_string(), help: help.to_string(), optional: false, options: vec![], placeholder: String::new() }).collect(),
        connect: None,
        trust: "official".into(),
        transport: "builtin".into(),
        author: "KIQR".into(),
        shipped: true,
    }
}

/// The index: shipped abilities, then whatever kiqr.cloud lists (cached for
/// an hour; a remote entry with the same id wins on words, never on source).
pub async fn catalogue(app: &App) -> Vec<IndexItem> {
    let mut items: Vec<IndexItem> = extensions::shipped_all()
        .into_iter()
        .map(|(_, m)| {
            let mut i = item_from_manifest(&m);
            i.shipped = true;
            i
        })
        .collect();
    for r in remote_index(app).await {
        if r.id.is_empty() || !kiqr_core::tools::valid_ext_id(&r.id) {
            continue;
        }
        let ok_repo = r.repo.as_deref().map(|u| u.starts_with("https://")).unwrap_or(false);
        if let Some(i) = items.iter_mut().find(|i| i.id == r.id) {
            // A shipped ability keeps its shipped source; the index may refresh its words and point at a repo.
            let shipped = i.shipped;
            let repo = if ok_repo { r.repo.clone() } else { None };
            let path = if ok_repo { r.path.clone().filter(|p| safe_path(p)) } else { None };
            *i = IndexItem { shipped, repo, path, ..r };
            if i.trust.is_empty() {
                i.trust = "official".into();
            }
        } else if ok_repo {
            let mut r = r;
            if r.trust.is_empty() {
                r.trust = "community".into();
            }
            r.path = r.path.filter(|p| safe_path(p));
            items.push(r);
        }
    }
    items
}

/// The index cache's modification time in seconds, 0 when there is none.
pub fn index_stamp(app: &App) -> u64 {
    let cache = app.store.home().root().join("tmp").join("marketplace.json");
    std::fs::metadata(&cache).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

/// Drop the cached index and read it again, in the background: the UI asks
/// for this when it opens, and listens for `marketplace_refreshed`.
pub fn refresh_index_later(app: &App) {
    let app = app.clone();
    tokio::spawn(async move {
        let url = app.config.read().await.marketplace.url.clone();
        let _ = std::fs::remove_file(app.store.home().root().join("tmp").join("marketplace.json"));
        tracing::info!(%url, "marketplace: reading the index again");
        let items = remote_index(&app).await;
        let fetched = index_fetched(&app);
        if fetched {
            tracing::info!(abilities = items.len(), ids = ?items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), "marketplace: index read");
        } else {
            tracing::warn!(%url, "marketplace: the index could not be read; only what ships with KIQR is listed");
        }
        app.bus.emit(None, EventKind::MarketplaceRefreshed { items: items.len(), fetched });
    });
}

/// `POST /api/abilities/refresh-index`: 202, the fetch runs on its own.
async fn refresh_index(State(app): State<App>) -> StatusCode {
    refresh_index_later(&app);
    StatusCode::ACCEPTED
}

/// Whether the marketplace index was read in the last day (its cache is on disk).
pub fn index_fetched(app: &App) -> bool {
    let cache = app.store.home().root().join("tmp").join("marketplace.json");
    std::fs::metadata(&cache).and_then(|m| m.modified()).map(|t| t.elapsed().unwrap_or_default() < Duration::from_secs(86_400)).unwrap_or(false)
}

async fn remote_index(app: &App) -> Vec<IndexItem> {
    let url = app.config.read().await.marketplace.url.clone();
    if url.trim().is_empty() {
        return vec![];
    }
    let cache = app.store.home().root().join("tmp").join("marketplace.json");
    let fresh = std::fs::metadata(&cache).and_then(|m| m.modified()).map(|t| t.elapsed().unwrap_or_default() < Duration::from_secs(3600)).unwrap_or(false);
    let text = if fresh {
        std::fs::read_to_string(&cache).ok()
    } else {
        let fetched = async {
            let client = reqwest::Client::builder().timeout(Duration::from_secs(6)).user_agent(kiqr_core::fetch::USER_AGENT).build().ok()?;
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => resp.text().await.ok(),
                Ok(resp) => {
                    tracing::warn!(%url, status = %resp.status(), "the marketplace index could not be read");
                    None
                }
                Err(e) => {
                    tracing::warn!(%url, error = %e, "the marketplace index could not be fetched");
                    None
                }
            }
        }
        .await;
        match fetched {
            Some(t) if serde_json::from_str::<Value>(&t).is_ok() => {
                let _ = std::fs::create_dir_all(cache.parent().unwrap());
                let _ = std::fs::write(&cache, &t);
                Some(t)
            }
            _ => {
                // No network or no index yet: the last copy, if any.
                std::fs::read_to_string(&cache).ok()
            }
        }
    };
    let Some(v) = text.and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { return vec![] };
    match serde_json::from_value::<Vec<IndexItem>>(v["abilities"].clone()) {
        Ok(items) => items,
        Err(e) => {
            tracing::warn!(error = %e, "the marketplace index has entries KIQR cannot read");
            vec![]
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ListQuery {
    project: Option<String>,
}

/// One ability as the page, the CLI and Seemo see it: the index's words, the
/// installed folder's facts, the core definition, and the switches and keys
/// for one project (or the machine when `slug` is none).
struct Ctx<'a> {
    app: &'a App,
    slug: Option<&'a str>,
    cfg: kiqr_core::Config,
    reg: &'a extensions::Registry,
    project_off: Vec<String>,
    /// (contract, ability id) active for this project.
    active: Vec<(String, String)>,
    /// When the index was last fetched, as a cache-busting version for avatar URLs.
    stamp: u64,
}

fn item_from_instructions(e: &extensions::Entry) -> IndexItem {
    let i = e.instructions.clone().unwrap_or_default();
    IndexItem {
        id: e.name.clone(),
        name: i.name.clone(),
        summary: if i.summary.trim().is_empty() { i.description.clone() } else { i.summary.clone() },
        version: String::new(),
        repo: None,
        path: None,
        homepage: String::new(),
        categories: if i.categories.is_empty() { vec!["writing".into()] } else { i.categories.clone() },
        icon: if i.icon.is_empty() { "book-open".into() } else { i.icon.clone() },
        avatar: String::new(),
        cost: Some("model usage".into()),
        provides: vec![],
        channels: vec![],
        sources: vec![],
        auth: vec![],
        connect: None,
        trust: if e.builtin { "official".into() } else { "local".into() },
        transport: "instructions".into(),
        author: if !i.author.trim().is_empty() { i.author.clone() } else if e.builtin { "KIQR".into() } else { String::new() },
        shipped: e.builtin,
    }
}

/// Where the text of one instruction file is overridden: the project's copy, the machine's edit of a shipped file.
fn copies(ctx: &Ctx, folder: &str, builtin: bool) -> (bool, bool) {
    let home = ctx.app.store.home();
    let project = ctx.slug.map(|s| home.project_abilities_dir(s).join(folder).join("SKILL.md").exists()).unwrap_or(false);
    let path = home.abilities_dir().join(folder).join("SKILL.md");
    let machine = if builtin {
        match (std::fs::read_to_string(&path), std::fs::read_to_string(path.with_file_name(".builtin"))) {
            (Ok(c), Ok(l)) => c != l,
            (Ok(c), Err(_)) => Some(c.as_str()) != kiqr_core::extract::builtin_instructions(folder).or_else(|| extensions::BUILTIN_EXTENSIONS.iter().find(|(n, _, _)| *n == folder).and_then(|(_, t, _)| *t)),
            _ => false,
        }
    } else {
        path.exists()
    };
    (project, machine)
}

fn ability_view(ctx: &Ctx, item: &IndexItem, m: Option<&Manifest>, e: Option<&extensions::Entry>, core: Option<&extensions::CoreAbility>) -> Value {
    let home = ctx.app.store.home();
    let auth: Vec<&AuthKey> = m.map(|m| m.auth.iter().collect()).unwrap_or_else(|| item.auth.iter().collect());
    // A core ability's keys are KIQR's own (the machine's); an installed one's live on the project's accounts of it.
    let missing: Vec<String> = match (m, core) {
        (Some(m), None) => crate::ext::missing_in(ctx.app, ctx.slug, m),
        _ => auth.iter().filter(|a| !a.optional && !crate::signins::has_core_key(home, ctx.slug, &a.env)).map(|a| a.env.clone()).collect(),
    };
    // This project's accounts of it: never their values, only whether they are complete.
    let accounts: Vec<Value> = match (m, ctx.slug) {
        (Some(m), Some(slug)) if core.is_none() => kiqr_core::connections::Connection::of(&ctx.app.store, slug, m.service())
            .iter()
            .map(|c| {
                let keys = c.keys(home, slug);
                json!({
                    "id": c.id,
                    "label": c.label,
                    "default": c.default,
                    "channel": c.is_channel(),
                    "kind": c.kind,
                    "destination_text": c.destination_text(),
                    "missing": c.missing(home, slug, m),
                    "set": m.auth.iter().filter(|a| keys.contains_key(&a.env)).map(|a| a.env.clone()).collect::<Vec<_>>(),
                })
            })
            .collect(),
        _ => vec![],
    };
    let program_found = m.filter(|m| m.transport == Transport::Mcp).map(|m| crate::ext::on_path(&m.command));
    let installed = core.is_some() || e.is_some();
    let ready = installed && missing.is_empty() && program_found.unwrap_or(true);
    let provides: Vec<Value> = m.map(|m| m.provides.keys().cloned().collect::<Vec<_>>()).unwrap_or_else(|| item.provides.clone()).iter().map(|id| json!({ "ability": id, "label": abilities::find(id).map(|a| a.label).unwrap_or("") })).collect();
    let mut item = item.clone();
    if item.repo.is_none() {
        item.repo = e.and_then(|e| e.source.clone());
    }
    let item = &item;
    let author = author_of(item);
    // The instruction files: a core ability's shipped ones, else the folder's own SKILL.md.
    let files: Vec<Value> = match (core, e) {
        (Some(c), _) => c
            .instructions
            .iter()
            .map(|(folder, label)| {
                let (project, machine) = copies(ctx, folder, true);
                json!({ "file": folder, "label": label, "project_copy": project, "machine_copy": machine })
            })
            .collect(),
        (None, Some(e)) if e.instructions.is_some() => {
            let (project, machine) = copies(ctx, &e.name, e.builtin);
            vec![json!({ "file": e.name, "label": e.instructions.as_ref().map(|i| i.name.clone()).unwrap_or_default(), "project_copy": project, "machine_copy": machine })]
        }
        _ => vec![],
    };
    let description = e.and_then(|e| e.instructions.as_ref()).map(|i| i.description.clone()).or_else(|| {
        core.and_then(|c| c.instructions.first()).and_then(|(folder, _)| ctx.reg.entry(ctx.slug, folder).and_then(|e| e.instructions.as_ref()).map(|i| i.description.clone()))
    });
    let in_prompt = core.map(|c| c.in_prompt).unwrap_or(!files.is_empty());
    // Only a core ability has a machine-wide switch (config.yaml); an installed one is switched per project.
    let machine_off = match core {
        Some(c) => !ctx.cfg.core_on(c.id),
        None => false,
    };
    let is_core = core.is_some() || e.is_some_and(|e| e.is_core());
    let source = if core.is_some() {
        "core"
    } else if item.shipped || e.is_some_and(|e| e.builtin) {
        "shipped"
    } else if item.repo.is_some() || e.is_some_and(|e| e.git) {
        "marketplace"
    } else if m.is_none() {
        "yours"
    } else {
        "local"
    };
    json!({
        "id": item.id,
        "name": item.name,
        "author": if source == "yours" { "you".to_string() } else if author.eq_ignore_ascii_case("kiqr") { "KIQR".to_string() } else { author.clone() },
        "author_url": if source == "yours" { None } else { author_url(item, &author) },
        "summary": item.summary,
        "description": description.unwrap_or_default(),
        "version": e.and_then(|_| m.map(|m| m.version.clone())).unwrap_or_else(|| item.version.clone()),
        "latest": item.version,
        "homepage": item.homepage,
        "categories": if item.categories.is_empty() { m.map(default_categories).unwrap_or_default() } else { item.categories.clone() },
        "icon": item.icon,
        "avatar": avatar_url(item, m, e, ctx.stamp),
        "cost": item.cost,
        "provides": provides,
        "channels": m.map(|m| m.channels.iter().map(|c| c.label.clone()).collect::<Vec<_>>()).unwrap_or_else(|| item.channels.iter().map(|c| c.display_name().to_string()).collect()),
        "sources": m.map(|m| m.sources.keys().cloned().collect::<Vec<_>>()).unwrap_or_else(|| item.sources.clone()),
        "tools": m.map(|m| m.tools.len() + if m.transport == Transport::Mcp { m.expose.len() } else { 0 }).unwrap_or(0),
        "trust": item.trust,
        "transport": m.map(|m| serde_json::to_value(m.transport).unwrap_or(Value::Null)).unwrap_or_else(|| json!(item.transport)),
        "runs_program": m.map(|m| matches!(m.transport, Transport::Mcp | Transport::Command)).unwrap_or(item.transport == "mcp" || item.transport == "command"),
        "command": m.filter(|m| matches!(m.transport, Transport::Mcp | Transport::Command)).map(|m| format!("{} {}", m.command, m.args.join(" ")).trim().to_string()),
        "program_found": program_found,
        "hosts": m.map(|m| m.hosts.clone()).unwrap_or_default(),
        "source": source,
        "repo": item.repo,
        "core": is_core,
        "builtin": core.is_some() || e.is_some_and(|e| e.builtin),
        "installed": installed,
        "scope": e.map(|e| json!(e.scope)).unwrap_or_else(|| if core.is_some() { json!({ "scope": "global" }) } else { Value::Null }),
        "folder": e.map(|e| e.dir.display().to_string()),
        "git": e.is_some_and(|e| e.git),
        // What `update` can refresh: the repository it came from (its source note, its own git, else the
        // index's repository for it, for folders installed before the note was kept), or a shipped copy.
        "updatable": !is_core && e.is_some_and(|e| e.source.is_some() || e.git || item.repo.is_some() || extensions::shipped(&item.id).is_some()),
        "machine_off": machine_off,
        "paused": ctx.project_off.iter().any(|x| x == &item.id),
        "ready": ready,
        "missing": missing,
        "auth": auth.iter().map(|a| json!({ "env": a.env, "label": a.label, "help": a.help, "optional": a.optional, "options": a.options, "placeholder": a.placeholder, "set": if core.is_some() { json!(kiqr_core::secrets::scope_of(home, ctx.slug, &a.env)) } else { Value::Null } })).collect::<Vec<_>>(),
        "connect": kiqr_core::connect::view(m.and_then(|m| m.connect.as_ref()).or(item.connect.as_ref())),
        "accounts": accounts,
        "kinds": m.map(|m| m.channels.iter().map(|c| json!({ "kind": c.kind, "label": c.label, "name": c.display_name(), "destination": c.destination, "does": c.roles() })).collect::<Vec<_>>()).unwrap_or_default(),
        "service": m.map(|m| m.service().to_string()),
        "active_for": ctx.active.iter().filter(|(_, id)| *id == item.id).map(|(a, _)| a.clone()).collect::<Vec<_>>(),
        "instructions": files,
        "in_prompt": in_prompt,
        "error": e.and_then(|e| e.error.clone()),
    })
}

/// What the page shows: what is here, what could be, and the rest.
pub(crate) struct Listing {
    pub abilities: Vec<Value>,
    pub market: Vec<Value>,
    pub contracts: Vec<Value>,
    pub broken: Vec<Value>,
    /// Seemo's own guide (`seemo-agent`), in the same shape, for Advanced.
    pub guide: Option<Value>,
}

/// Every ability for a project (or the machine): core first, then installed
/// folders and the index, then local folders. Attached folders (a core
/// ability's instruction files, Seemo's guide) are never their own row.
pub(crate) async fn listing(app: &App, slug: Option<&str>) -> Listing {
    let cfg = app.config.read().await.clone();
    let items = catalogue(app).await;
    let reg = app.registry.read().await;
    let project_off = slug.map(|s| crate::ext::project_off(app, s)).unwrap_or_default();
    let active: Vec<(String, String)> = abilities::all().iter().filter_map(|a| reg.provider(slug.unwrap_or(""), a.id, cfg.abilities.get(a.id).map(String::as_str), |m| crate::ext::ready_in(app, slug, m)).map(|(_, m)| (a.id.to_string(), m.id.clone()))).collect();
    let ctx = Ctx { app, slug, cfg: cfg.clone(), reg: &reg, project_off, active, stamp: index_stamp(app) };
    let attached = extensions::attached_folders();
    let mut abilities_out: Vec<Value> = Vec::new();
    let mut market: Vec<Value> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for c in extensions::CORE_ABILITIES {
        abilities_out.push(ability_view(&ctx, &core_item(c), None, None, Some(c)));
        seen.push(c.id.to_string());
    }
    for item in &items {
        let e = reg.entry(slug, &item.id);
        let v = ability_view(&ctx, item, e.and_then(|e| e.manifest.as_ref()), e, None);
        if e.is_some() {
            abilities_out.push(v);
        } else {
            market.push(v);
        }
        seen.push(item.id.clone());
    }
    // Installed from a folder, a URL or taught here: not in any index.
    for e in reg.entries.iter().filter(|e| e.error.is_none()) {
        if seen.contains(&e.id().to_string()) || attached.contains(&e.name.as_str()) {
            continue;
        }
        if let Scope::Project(p) = &e.scope {
            if slug != Some(p.as_str()) {
                continue;
            }
        }
        // A machine folder the project shadows with its own copy: the project's wins.
        if e.scope == Scope::Global && slug.is_some_and(|s| reg.entries.iter().any(|o| o.scope == Scope::Project(s.to_string()) && o.id() == e.id())) {
            continue;
        }
        let item = match &e.manifest {
            Some(m) => item_from_manifest(m),
            None => item_from_instructions(e),
        };
        let mut v = ability_view(&ctx, &item, e.manifest.as_ref(), Some(e), None);
        if e.manifest.is_some() {
            v["trust"] = json!("local");
        }
        abilities_out.push(v);
        seen.push(e.id().to_string());
    }
    let contracts: Vec<Value> = abilities::all()
        .iter()
        .map(|a| {
            let configured = cfg.abilities.get(a.id).cloned();
            let candidates: Vec<Value> = reg.connections(slug.unwrap_or("")).into_iter().filter(|(_, m)| m.provides.contains_key(a.id)).map(|(_, m)| json!({ "id": m.id, "name": m.name, "ready": crate::ext::ready_in(app, slug, m) })).collect();
            let act = ctx.active.iter().find(|(x, _)| x == a.id).map(|(_, id)| id.clone());
            let name = act.as_ref().and_then(|id| candidates.iter().find(|c| c["id"] == *id).and_then(|c| c["name"].as_str().map(str::to_string)));
            json!({ "id": a.id, "label": a.label, "tool": a.tool, "configured": configured, "active": act.map(|id| json!({ "id": id, "name": name })), "candidates": candidates })
        })
        .collect();
    let broken: Vec<Value> = reg.entries.iter().filter_map(|e| e.error.as_ref().map(|err| json!({ "folder": e.name, "scope": e.scope, "error": err }))).collect();
    let guide = reg.entry(slug, extensions::GUIDE_FOLDER).map(|e| {
        let mut item = item_from_instructions(e);
        item.name = "Seemo's own guide".into();
        item.summary = "How Seemo works inside KIQR: the tools, when to act and when to make a task, how to ask for a yes, how to report. Read at the start of every chat and run.".into();
        item.icon = "book-open".into();
        item.author = "KIQR".into();
        let mut v = ability_view(&ctx, &item, None, Some(e), None);
        v["core"] = json!(true);
        v["source"] = json!("core");
        v["in_prompt"] = json!(false);
        v["instructions"][0]["label"] = json!("How Seemo works inside KIQR");
        v
    });
    Listing { abilities: abilities_out, market, contracts, broken, guide }
}

/// One ability by id, for a project.
pub(crate) async fn one(app: &App, slug: Option<&str>, id: &str) -> Option<Value> {
    let l = listing(app, slug).await;
    l.abilities.into_iter().chain(l.market).find(|v| v["id"] == id)
}

/// Whether an ability is on: for the machine, and for this project.
pub(crate) async fn on_for(app: &App, slug: &str, id: &str) -> (bool, bool) {
    let project_on = !crate::ext::project_off(app, slug).iter().any(|x| x == id);
    if let Some(c) = extensions::core_ability(id) {
        return (app.config.read().await.core_on(c.id), project_on);
    }
    let reg = app.registry.read().await;
    (reg.entry(Some(slug), id).is_some_and(|e| !e.paused), project_on)
}

async fn list_json(app: &App, slug: Option<&str>) -> Value {
    let cfg = app.config.read().await.clone();
    let home = app.store.home();
    let l = listing(app, slug).await;
    json!({
        "abilities": l.abilities,
        "market": l.market,
        "contracts": l.contracts,
        "broken": l.broken,
        "guide": l.guide,
        "index": cfg.marketplace.url,
        "index_fetched": index_fetched(app),
        "env_file": home.env_file().display().to_string(),
        "project_env_file": slug.map(|s| home.project_env_file(s).display().to_string()),
    })
}

async fn list(State(app): State<App>, Query(q): Query<ListQuery>) -> ApiResult<Value> {
    let slug = q.project.clone().filter(|s| !s.is_empty());
    if let Some(s) = &slug {
        Project::load(&app.store, s)?;
    }
    Ok(Json(list_json(&app, slug.as_deref()).await))
}

async fn list_project(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    Project::load(&app.store, &slug)?;
    Ok(Json(list_json(&app, Some(&slug)).await))
}

/// For Seemo (`list_abilities`): what it can do here, in a few fields.
async fn brief(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    Project::load(&app.store, &slug)?;
    let l = listing(&app, Some(&slug)).await;
    let rows: Vec<Value> = l
        .abilities
        .iter()
        .map(|v| {
            let on = !v["machine_off"].as_bool().unwrap_or(false) && !v["paused"].as_bool().unwrap_or(false);
            json!({
                "id": v["id"],
                "name": v["name"],
                "on": on,
                "ready": v["ready"],
                "needs": v["missing"],
                "when": v["description"],
                "instructions": v["instructions"].as_array().map(|a| a.iter().filter_map(|f| f["file"].as_str().map(str::to_string)).collect::<Vec<_>>()).unwrap_or_default(),
            })
        })
        .collect();
    let suggestions = crate::networks::suggestions(&app, &slug).await;
    Ok(Json(json!({ "abilities": rows, "suggestions": suggestions, "note": "Read an ability's instructions with read_instructions before doing that kind of work. Off or not ready means you cannot use it here; say so instead of improvising. `suggestions` are profiles the business has where an ability in the marketplace could publish; the user installs it under Setup → Abilities." })))
}

fn scope_of(b_scope: &str, project: &Option<String>) -> Result<Scope, ApiError> {
    match (b_scope, project.as_deref().filter(|p| !p.is_empty())) {
        ("project", Some(p)) => Ok(Scope::Project(p.to_string())),
        ("project", None) => Err(bad("installing for a project needs its slug")),
        _ => Ok(Scope::Global),
    }
}

/// Write a shipped ability's folder into `abilities/`, like a first run would.
async fn install_shipped(app: &App, id: &str, scope: &Scope) -> Result<String, ApiError> {
    let (folder, skill, manifest, _) = extensions::shipped(id).ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no ability '{id}' to install")))?;
    let root = match scope {
        Scope::Global => app.store.home().abilities_dir(),
        Scope::Project(s) => {
            Project::load(&app.store, s)?;
            app.store.home().project_abilities_dir(s)
        }
    };
    let dir = root.join(folder);
    if let Some(s) = skill {
        extensions::install_builtin_text(&app.store, &dir.join("SKILL.md"), s).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    app.store.write_text(&dir.join("extension.yaml"), manifest)?;
    let _ = std::fs::remove_file(dir.join(".off"));
    Ok(folder.to_string())
}

/// Install is always for the machine: one copy, one version, no keys. A
/// project adds its own accounts of it afterwards (`/api/projects/{slug}/connections`,
/// or a channel under `/channels`).
async fn install(State(app): State<App>, Path(id): Path<String>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let scope = Scope::Global;
    {
        let reg = app.registry.read().await;
        if reg.installed(&id).iter().any(|e| e.scope == scope) {
            return Err(ApiError(StatusCode::CONFLICT, format!("'{id}' is already installed there")));
        }
    }
    let item = catalogue(&app).await.into_iter().find(|i| i.id == id).ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("the marketplace has no ability '{id}'")))?;
    let folder = match (&item.repo, item.shipped) {
        (Some(repo), _) => {
            let (from, tmp) = fetch_repo(&app, repo, item.path.as_deref()).await?;
            let r = crate::ext::install_into(&app, scope.clone(), crate::ext::Install { source: from.display().to_string(), name: Some(id.clone()) }).await;
            let _ = std::fs::remove_dir_all(&tmp);
            let (_, Json(v)) = r?;
            let folder = v["name"].as_str().unwrap_or(&id).to_string();
            // Remember where it came from, so Update can fetch it again. The folder itself is committed to ~/.kiqr.
            let _ = std::fs::write(app.store.home().abilities_dir().join(&folder).join(".source"), source_note(repo, item.path.as_deref()));
            folder
        }
        (None, true) => {
            let folder = install_shipped(&app, &id, &scope).await?;
            let label = match &scope {
                Scope::Global => "abilities".to_string(),
                Scope::Project(s) => format!("abilities({s})"),
            };
            app.store.commit(&format!("{label}: install {folder}")).await?;
            crate::ext::reload(&app).await;
            app.bus.emit(None, EventKind::ExtensionInstalled { id: id.clone(), name: item.name.clone() });
            folder
        }
        (None, false) => return Err(bad(format!("'{id}' has no repository to install from"))),
    };
    adopt_keys(&app, &id).await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "folder": folder, "scope": scope }))))
}

/// Keys a project already had for this ability (from before accounts) become its first account.
async fn adopt_keys(app: &App, id: &str) -> Result<(), ApiError> {
    let m = {
        let reg = app.registry.read().await;
        reg.installed(id).into_iter().find_map(|e| e.manifest.clone())
    };
    let Some(m) = m else { return Ok(()) };
    let adopted = kiqr_core::connections::adopt(&app.store, &m)?;
    if !adopted.is_empty() {
        app.store.commit(&format!("[auto] accounts: {} keeps the {} keys it had", adopted.join(", "), m.name)).await?;
        for slug in &adopted {
            if let Some(c) = kiqr_core::connections::Connection::default_of(&app.store, slug, m.service()) {
                app.bus.emit(Some(slug), EventKind::ConnectionSaved { connection: c.id, label: c.label, ability: c.ability, kind: c.kind });
            }
        }
    }
    Ok(())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SourceBody {
    source: String,
    scope: String,
    project: Option<String>,
    name: Option<String>,
}

/// Advanced: a git URL or a folder on this machine that is not in any index.
async fn install_source(State(app): State<App>, Json(b): Json<SourceBody>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let scope = scope_of(&b.scope, &b.project)?;
    let r = crate::ext::install_into(&app, scope, crate::ext::Install { source: b.source, name: b.name }).await?;
    if let Some(id) = r.1["id"].as_str() {
        adopt_keys(&app, id).await?;
    }
    Ok(r)
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ScopeQuery {
    project: Option<String>,
}

fn find_installed(reg: &extensions::Registry, id: &str, project: &Option<String>) -> Result<extensions::Entry, ApiError> {
    let scope = project.clone().filter(|p| !p.is_empty()).map(Scope::Project).unwrap_or(Scope::Global);
    reg.installed(id).into_iter().find(|e| e.scope == scope).cloned().ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("'{id}' is not installed there")))
}

/// `git pull` for a cloned ability; a fresh copy for a shipped one.
async fn update(State(app): State<App>, Path(id): Path<String>, Query(q): Query<ScopeQuery>) -> ApiResult<Value> {
    let e = {
        let reg = app.registry.read().await;
        find_installed(&reg, &id, &q.project)?
    };
    // No source note (installed before it was kept): the index's repository for this id.
    let source = match (&e.source, e.git) {
        (Some(s), _) => Some(s.clone()),
        (None, true) => None,
        (None, false) => catalogue(&app).await.into_iter().find(|i| i.id == id).and_then(|i| i.repo.map(|r| source_note(&r, i.path.as_deref()).trim().to_string())),
    };
    let detail = if let Some(src) = source {
        // Fetch the repository again and replace the folder's files; the switch and the source note stay.
        let (repo, path) = match src.split_once('#') {
            Some((r, p)) if safe_path(p) => (r.to_string(), Some(p.to_string())),
            _ => (src.clone(), None),
        };
        let (from, tmp) = fetch_repo(&app, &repo, path.as_deref()).await?;
        let paused = e.paused;
        std::fs::remove_dir_all(&e.dir).map_err(anyhow::Error::from)?;
        let copied = crate::ext::copy_dir(&from, &e.dir);
        let _ = std::fs::remove_dir_all(&tmp);
        copied.map_err(|err| ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("could not copy: {err}")))?;
        let _ = std::fs::write(e.dir.join(".source"), source_note(&repo, path.as_deref()));
        if paused {
            let _ = std::fs::write(e.dir.join(".off"), "paused by the user\n");
        }
        "fetched again".to_string()
    } else if e.git {
        let out = tokio::process::Command::new("git").args(["pull", "--ff-only", "--quiet"]).current_dir(&e.dir).output().await.map_err(|err| bad(format!("could not run git: {err}")))?;
        if !out.status.success() {
            return Err(bad(format!("git pull failed: {}", String::from_utf8_lossy(&out.stderr).trim())));
        }
        "pulled".to_string()
    } else if extensions::shipped(&id).is_some() {
        install_shipped(&app, &id, &e.scope).await?;
        "refreshed from this release".to_string()
    } else {
        return Err(bad("this ability came from a folder, not a repository; install it again to update it"));
    };
    let label = match &e.scope {
        Scope::Global => "abilities".to_string(),
        Scope::Project(s) => format!("abilities({s})"),
    };
    app.store.commit(&format!("{label}: update {}", e.name)).await?;
    crate::ext::reload(&app).await;
    app.mcp.stop(&id).await;
    app.bus.emit(None, EventKind::ExtensionInstalled { id: id.clone(), name: e.manifest.as_ref().map(|m| m.name.clone()).unwrap_or_else(|| id.clone()) });
    Ok(Json(json!({ "id": id, "updated": true, "detail": detail })))
}

async fn set_paused(app: &App, id: &str, project: &Option<String>, paused: bool) -> ApiResult<Value> {
    // Core abilities, code or folder, never leave: their switch lives in config.yaml.
    if extensions::is_core_id(id) {
        if let Some(c) = extensions::core_ability(id) {
            if !paused {
                let missing: Vec<&str> = c.auth.iter().filter(|(env, _, _)| !crate::signins::has_core_key(app.store.home(), None, env)).map(|(env, _, _)| *env).collect();
                if !missing.is_empty() {
                    return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("{} needs {} before it can be on", c.name, missing.join(", "))));
                }
            }
        }
        let mut cfg = app.config.read().await.clone();
        cfg.core_off.retain(|x| x != id);
        if paused {
            cfg.core_off.push(id.to_string());
        }
        crate::api::apply_config(app, cfg, "you", false).await?;
        crate::ext::reload(app).await;
        app.bus.emit(None, EventKind::ExtensionPaused { id: id.to_string(), paused });
        return Ok(Json(json!({ "id": id, "paused": paused })));
    }
    let _ = project;
    Err(bad(format!("'{id}' is switched on and off per project (POST /api/projects/{{slug}}/abilities/{id}/pause or resume)")))
}

async fn pause(State(app): State<App>, Path(id): Path<String>, Query(q): Query<ScopeQuery>) -> ApiResult<Value> {
    set_paused(&app, &id, &q.project, true).await
}

async fn resume(State(app): State<App>, Path(id): Path<String>, Query(q): Query<ScopeQuery>) -> ApiResult<Value> {
    set_paused(&app, &id, &q.project, false).await
}

/// Remove an installed ability's folder. Keys stay in `.env` until cleared.
async fn remove(State(app): State<App>, Path(id): Path<String>, Query(q): Query<ScopeQuery>) -> Result<StatusCode, ApiError> {
    if extensions::is_core_id(&id) {
        return Err(bad("this is one of KIQR's core abilities: it can be switched off, not removed"));
    }
    let e = {
        let reg = app.registry.read().await;
        find_installed(&reg, &id, &q.project)?
    };
    crate::ext::remove_folder(&app, &e.dir, &id, &e.scope).await?;
    app.mcp.stop(&id).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/abilities/{id}/avatar?project=`: the mark from the installed folder.
async fn avatar(State(app): State<App>, Path(id): Path<String>, Query(q): Query<ListQuery>) -> Result<axum::response::Response, ApiError> {
    use axum::response::IntoResponse;
    let reg = app.registry.read().await;
    let e = reg.entry(q.project.as_deref(), &id).ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "no such ability".into()))?;
    let name = e.manifest.as_ref().map(|m| m.avatar.trim().to_string()).filter(|a| !a.is_empty() && !a.contains('/') && !a.starts_with('.')).ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "no avatar".into()))?;
    let path = e.dir.join(&name);
    let bytes = std::fs::read(&path).map_err(|_| ApiError(StatusCode::NOT_FOUND, "no avatar".into()))?;
    let mime = mime_guess::from_path(&path).first_or_octet_stream().to_string();
    Ok(([(axum::http::header::CONTENT_TYPE, mime), (axum::http::header::CACHE_CONTROL, "private, max-age=3600".to_string())], bytes).into_response())
}
