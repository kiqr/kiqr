use crate::App;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post, put},
    Json, Router,
};
use futures::stream::Stream;
use kiqr_core::{
    assets::{self, Asset, AssetType, NewAsset, TypeCount},
    brand::{self, Brand},
    crawl::{CrawlControl, Crawler},
    events::EventKind,
    profile::{self, Brief, BriefSource, Section},
    project::Project,
    provider,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

pub fn router(app: App) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/config", get(get_config).put(put_config))
        .route("/api/projects", get(list_projects).post(create_project))
        .route("/api/projects/{slug}", get(get_project).delete(delete_project))
        .route("/api/projects/{slug}/crawl", post(start_crawl))
        .route("/api/projects/{slug}/crawl/limit", put(put_crawl_limit))
        .route("/api/projects/{slug}/settings", put(put_settings))
        .route("/api/projects/{slug}/synthesize", post(start_synthesize))
        .route("/api/projects/{slug}/profile", get(get_profile))
        .route("/api/projects/{slug}/profile/{section}", put(put_section))
        .route("/api/projects/{slug}/profile.md", get(get_profile_md))
        .route("/api/projects/{slug}/brief", get(get_brief).put(put_brief).post(start_brief))
        .route("/api/projects/{slug}/brand", get(get_brand).post(rebuild_brand))
        .route("/api/projects/{slug}/history", get(history))
        .route("/api/asset-types", get(list_asset_types).post(save_asset_type))
        .route("/api/projects/{slug}/assets", get(list_assets).post(create_asset))
        .route("/api/projects/{slug}/assets/summary", get(asset_summary))
        .route("/api/projects/{slug}/assets/{kind}/{id}", get(get_asset).put(put_asset).delete(delete_asset))
        .route("/api/projects/{slug}/assets/{kind}/{id}/files/{name}", get(get_asset_file).put(put_asset_file).layer(axum::extract::DefaultBodyLimit::max(MAX_ASSET_FILE)))
        .route("/api/events", get(events))
        .with_state(app)
}

type ApiResult<T> = Result<Json<T>, ApiError>;

pub struct ApiError(StatusCode, String);

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        let msg = e.to_string();
        let code = if INVALID.iter().any(|p| msg.starts_with(p)) {
            StatusCode::BAD_REQUEST
        } else if msg.starts_with("no project") || msg.starts_with("no asset") {
            StatusCode::NOT_FOUND
        } else if msg.contains("already exists") || msg.contains("only http") || INVALID.iter().any(|p| msg.starts_with(p)) {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        ApiError(code, msg)
    }
}

/// Largest file one upload may carry: room for a few minutes of video.
const MAX_ASSET_FILE: usize = 1024 * 1024 * 1024;

/// Messages that mean the request itself was wrong.
const INVALID: &[&str] = &["no asset type", "a type id", "a type needs", "an asset needs", "a file name"];

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
    }
}

#[derive(Serialize)]
struct Health {
    ok: bool,
    version: &'static str,
    home: String,
    providers: Vec<ProviderStatus>,
    unblockingapi: bool,
    /// The secrets KIQR knows about and whether each is set. Names only, never values.
    secrets: Vec<SecretStatus>,
    /// Model command-line tools found on this machine.
    tools: Vec<ToolStatus>,
    env_file: String,
    address: String,
}

#[derive(Serialize)]
struct SecretStatus {
    name: &'static str,
    set: bool,
}

#[derive(Serialize)]
struct ToolStatus {
    name: &'static str,
    installed: bool,
}

/// Every secret KIQR reads from `.env`.
const SECRETS: &[&str] = &["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN", "UNBLOCKINGAPI_KEY", "KIQR_CLOUD_TOKEN"];

#[derive(Serialize)]
struct ProviderStatus {
    task: &'static str,
    model: String,
    available: bool,
}

async fn health(State(app): State<App>) -> Json<Health> {
    let cfg = app.config.read().await.clone();
    let mut providers = Vec::new();
    for (task, model) in [("extract", &cfg.models.extract), ("synthesize", &cfg.models.synthesize), ("chat", &cfg.models.chat)] {
        providers.push(ProviderStatus { task, model: model.clone(), available: provider::available(model).await });
    }
    Json(Health {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
        home: app.store.home().root().display().to_string(),
        providers,
        unblockingapi: std::env::var("UNBLOCKINGAPI_KEY").map(|k| !k.is_empty()).unwrap_or(false),
        secrets: SECRETS.iter().map(|&name| SecretStatus { name, set: provider::has_env(name) }).collect(),
        tools: vec![ToolStatus { name: "claude", installed: provider::which("claude").await }, ToolStatus { name: "codex", installed: provider::which("codex").await }],
        env_file: app.store.home().env_file().display().to_string(),
        address: format!(
            "{}:{}",
            std::env::var("KIQR_HOST").unwrap_or_else(|_| cfg.daemon.host.clone()),
            std::env::var("KIQR_PORT").ok().unwrap_or_else(|| cfg.daemon.port.to_string())
        ),
    })
}

async fn get_config(State(app): State<App>) -> Json<kiqr_core::Config> {
    Json(app.config.read().await.clone())
}

async fn put_config(State(app): State<App>, Json(cfg): Json<kiqr_core::Config>) -> ApiResult<kiqr_core::Config> {
    cfg.save(&app.store)?;
    app.store.commit("config: update").await?;
    *app.config.write().await = cfg.clone();
    Ok(Json(cfg))
}

async fn list_projects(State(app): State<App>) -> ApiResult<Vec<Project>> {
    Ok(Json(Project::list(&app.store)?))
}

#[derive(Deserialize)]
struct CreateProject {
    url: String,
    #[serde(default = "default_true")]
    crawl: bool,
}
fn default_true() -> bool {
    true
}

async fn create_project(State(app): State<App>, Json(body): Json<CreateProject>) -> Result<(StatusCode, Json<Project>), ApiError> {
    let project = Project::create(&app.store, &app.bus, &body.url).await?;
    if body.crawl {
        let key = claim(&app, "crawl", &project.slug).await?;
        spawn_crawl(app.clone(), project.clone(), key).await;
    }
    Ok((StatusCode::CREATED, Json(project)))
}

/// The project, plus `jobs`: what is running on it now (`crawl`, `synthesize`, `brief`).
async fn get_project(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<serde_json::Value> {
    let project = Project::load(&app.store, &slug)?;
    let suffix = format!(":{slug}");
    let jobs: Vec<String> = app.jobs.read().await.iter().filter_map(|k| k.strip_suffix(&suffix).map(str::to_string)).collect();
    let mut v = serde_json::to_value(project).map_err(anyhow::Error::from)?;
    v["jobs"] = serde_json::json!(jobs);
    Ok(Json(v))
}

async fn delete_project(State(app): State<App>, Path(slug): Path<String>) -> Result<StatusCode, ApiError> {
    let _ = Project::load(&app.store, &slug)?;
    std::fs::remove_dir_all(app.store.home().project_dir(&slug)).map_err(anyhow::Error::from)?;
    app.bus.forget_log(&slug);
    app.store.commit(&format!("project({slug}): remove")).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn put_settings(State(app): State<App>, Path(slug): Path<String>, Json(q): Json<CrawlOpts>) -> ApiResult<Project> {
    let mut project = Project::load(&app.store, &slug)?;
    project.crawl.max_pages = q.max_pages;
    project.crawl.concurrency = q.concurrency.map(|n| n.clamp(1, 10));
    project.crawl.ai_concurrency = q.ai_concurrency.map(|n| n.clamp(1, 10));
    project.save(&app.store)?;
    app.store.commit(&format!("project({slug}): crawl settings")).await?;
    Ok(Json(project))
}

#[derive(Deserialize, Default)]
struct CrawlOpts {
    max_pages: Option<usize>,
    concurrency: Option<usize>,
    ai_concurrency: Option<usize>,
}

async fn start_crawl(State(app): State<App>, Path(slug): Path<String>, Query(q): Query<CrawlOpts>) -> ApiResult<serde_json::Value> {
    let mut project = Project::load(&app.store, &slug)?;
    if q.max_pages.is_some() || q.concurrency.is_some() || q.ai_concurrency.is_some() {
        if let Some(n) = q.max_pages {
            project.crawl.max_pages = Some(n);
        }
        if let Some(n) = q.concurrency {
            project.crawl.concurrency = Some(n);
        }
        if let Some(n) = q.ai_concurrency {
            project.crawl.ai_concurrency = Some(n);
        }
        project.save(&app.store)?;
        app.store.commit(&format!("project({slug}): crawl settings")).await?;
    }
    let key = claim(&app, "crawl", &slug).await?;
    spawn_crawl(app.clone(), project, key).await;
    Ok(Json(serde_json::json!({ "started": true })))
}

#[derive(Deserialize)]
struct CrawlLimit {
    max_pages: usize,
}

/// Set the project's page limit. If a crawl is still fetching, it takes effect
/// in that crawl right away (`live: true`); otherwise from the next crawl.
async fn put_crawl_limit(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<CrawlLimit>) -> ApiResult<serde_json::Value> {
    let n = b.max_pages.clamp(1, MAX_PAGES_LIMIT);
    let mut project = Project::load(&app.store, &slug)?;
    project.crawl.max_pages = Some(n);
    project.save(&app.store)?;
    app.store.commit(&format!("project({slug}): page limit {n}")).await?;
    let live = match app.crawls.read().await.get(&slug) {
        Some(c) => c.set_limit(n),
        None => false,
    };
    if live {
        app.bus.emit(Some(&slug), EventKind::CrawlLimitChanged { max_pages: n });
    }
    Ok(Json(serde_json::json!({ "max_pages": n, "live": live })))
}

/// The highest page limit a project can have.
const MAX_PAGES_LIMIT: usize = 5000;

/// Claim a project's job slot in one step, so two requests can't both start.
/// A crawl ends by rebuilding the profile, so it can't overlap a profile job
/// (`synthesize`, `brief`) on the same project, nor they it.
async fn claim(app: &App, kind: &str, slug: &str) -> Result<String, ApiError> {
    let mut jobs = app.jobs.write().await;
    let busy = |k: &str| jobs.contains(&format!("{k}:{slug}"));
    let crawling = busy("crawl");
    let rebuilding = busy("synthesize") || busy("brief");
    let same = busy(kind);
    if crawling {
        return Err(ApiError(StatusCode::CONFLICT, "a crawl is already running for this project".into()));
    }
    if kind == "crawl" && rebuilding {
        return Err(ApiError(StatusCode::CONFLICT, "the profile is being rebuilt for this project; start the crawl when it is done".into()));
    }
    if same {
        return Err(ApiError(StatusCode::CONFLICT, format!("{kind} is already running for this project")));
    }
    let key = format!("{kind}:{slug}");
    jobs.insert(key.clone());
    Ok(key)
}

/// Where a project's last crawl log is kept, so the crawl screen can show it
/// after a daemon restart. The cache folder is not committed.
fn crawl_log_file(app: &App, slug: &str) -> std::path::PathBuf {
    app.store.home().cache_dir(slug).join("crawl-log.jsonl")
}

/// Run a crawl job (crawl, then profile synthesis) holding the claimed `key`.
/// Every event it emits is logged for replay; see `events`.
async fn spawn_crawl(app: App, project: Project, key: String) {
    app.bus.open_log(&project.slug);
    let cfg = app.config.read().await.clone();
    let max_pages = project.crawl.max_pages.unwrap_or(cfg.crawl.max_pages);
    let control = CrawlControl::new(max_pages);
    app.crawls.write().await.insert(project.slug.clone(), control.clone());
    tokio::spawn(async move {
        let extractor = if provider::available(&cfg.models.extract).await {
            Some(app.extractor(&cfg))
        } else {
            app.bus.emit(
                Some(&project.slug),
                EventKind::SeemoSays {
                    text: format!(
                        "I'll read the site now, but I can't extract facts yet: the model `{}` isn't available. Add an API key to .env or install the CLI, then run the profile build.",
                        cfg.models.extract
                    ),
                },
            );
            None
        };
        let result = async {
            let crawler = Crawler::new(
                app.store.clone(),
                app.bus.clone(),
                project.crawl.concurrency.unwrap_or(cfg.crawl.concurrency),
                project.crawl.ai_concurrency.unwrap_or(cfg.crawl.ai_concurrency),
                cfg.crawl.escalate_to_unblockingapi,
                extractor.clone(),
            )?
            .with_control(control);
            crawler.run(&project, max_pages).await?;
            anyhow::Ok(())
        }
        .await;
        match result {
            Err(e) => {
                tracing::error!(slug = %project.slug, error = %e, "crawl failed");
                app.bus.emit(Some(&project.slug), EventKind::SeemoSays { text: format!("The crawl stopped: {e}") });
            }
            Ok(()) => {
                if let Some(ex) = extractor {
                    if provider::available(&cfg.models.synthesize).await {
                        // synthesize() tells the user itself when it fails.
                        match ex.synthesize(&project, true).await {
                            Ok(()) => app.bus.emit(
                                Some(&project.slug),
                                EventKind::SeemoSays { text: "The profile and the business brief are ready. Have a look and lock anything I got wrong.".into() },
                            ),
                            Err(e) => tracing::error!(slug = %project.slug, error = %e, "synthesis failed"),
                        }
                    }
                }
            }
        }
        if let Some(events) = app.bus.close_log(&project.slug) {
            let lines: String = events.iter().filter_map(|e| serde_json::to_string(e).ok()).map(|l| l + "\n").collect();
            if let Err(e) = app.store.write_text(&crawl_log_file(&app, &project.slug), &lines) {
                tracing::warn!(slug = %project.slug, error = %e, "could not keep the crawl log");
            }
        }
        app.crawls.write().await.remove(&project.slug);
        app.jobs.write().await.remove(&key);
        app.bus.emit(Some(&project.slug), EventKind::JobFinished { job: "crawl".into() });
    });
}

async fn start_synthesize(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<serde_json::Value> {
    let project = Project::load(&app.store, &slug)?;
    let cfg = app.config.read().await.clone();
    if !provider::available(&cfg.models.synthesize).await {
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("model `{}` is not available", cfg.models.synthesize)));
    }
    spawn_job(app, project, "synthesize", |ex, project| async move { ex.synthesize(&project, false).await }).await
}

/// Run a model job in the background, at most one per kind and project. The
/// job emits its own events, including Seemo's message on failure.
async fn spawn_job<F, Fut>(app: App, project: Project, kind: &'static str, job: F) -> ApiResult<serde_json::Value>
where
    F: FnOnce(std::sync::Arc<kiqr_core::extract::Extractor>, Project) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = anyhow::Result<()>> + Send,
{
    let key = claim(&app, kind, &project.slug).await?;
    let cfg = app.config.read().await.clone();
    let ex = app.extractor(&cfg);
    tokio::spawn(async move {
        let slug = project.slug.clone();
        if let Err(e) = job(ex, project).await {
            tracing::error!(%slug, error = %e, "{kind} failed");
        }
        app.jobs.write().await.remove(&key);
        app.bus.emit(Some(&slug), EventKind::JobFinished { job: kind.into() });
    });
    Ok(Json(serde_json::json!({ "started": true })))
}

async fn get_brief(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Option<Brief>> {
    let _ = Project::load(&app.store, &slug)?;
    Ok(Json(Brief::load(&app.store, &slug)?))
}

async fn put_brief(State(app): State<App>, Path(slug): Path<String>, Json(mut brief): Json<Brief>) -> ApiResult<Brief> {
    let project = Project::load(&app.store, &slug)?;
    let previous = Brief::load(&app.store, &slug)?;
    brief.normalize();
    brief.version = previous.map(|b| b.version).unwrap_or(0) + 1;
    brief.source = BriefSource::Edited;
    brief.updated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    brief.save(&app.store, &slug)?;
    profile::write_markdown(&app.store, &slug, &project.name, &project.url)?;
    app.store.commit(&format!("profile({slug}): edit business brief v{}", brief.version)).await?;
    app.bus.emit(
        Some(&slug),
        EventKind::ProfileUpdated {
            section: "brief".into(),
            by: "you".into(),
            title: "Business brief".into(),
            items: kiqr_core::extract::brief_items(&brief),
        },
    );
    Ok(Json(brief))
}

async fn start_brief(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<serde_json::Value> {
    let project = Project::load(&app.store, &slug)?;
    let cfg = app.config.read().await.clone();
    if !provider::available(&cfg.models.synthesize).await {
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("model `{}` is not available", cfg.models.synthesize)));
    }
    spawn_job(app, project, "brief", |ex, project| async move { ex.brief(&project, false).await }).await
}

/// The saved brand, or one built on the fly from the crawl cache (without
/// writing) for projects crawled before brand collection existed.
async fn get_brand(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Brand> {
    let project = Project::load(&app.store, &slug)?;
    if let Some(b) = Brand::load(&app.store, &slug)?.filter(|b| !b.outdated()) {
        return Ok(Json(b));
    }
    match backfill_brand(&app, &project).await? {
        Some(b) => Ok(Json(b)),
        None => Err(no_brand()),
    }
}

/// Build and save the brand of a project that has none, or one built by older
/// collection rules, once: reading every cached page takes seconds, so it
/// must not happen on each page view. One at a time, so two views don't both
/// build it. Without a usable cache an outdated brand is kept as it is.
pub(crate) async fn backfill_brand(app: &App, project: &Project) -> anyhow::Result<Option<Brand>> {
    let _one = app.brand_backfill.lock().await;
    let saved = Brand::load(&app.store, &project.slug)?;
    if let Some(b) = saved.as_ref().filter(|b| !b.outdated()) {
        return Ok(Some(b.clone()));
    }
    Ok(brand::refresh(&app.store, &app.bus, project, true).await?.or(saved))
}

async fn rebuild_brand(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Brand> {
    let project = Project::load(&app.store, &slug)?;
    match brand::refresh(&app.store, &app.bus, &project, false).await? {
        Some(b) => Ok(Json(b)),
        None => Err(no_brand()),
    }
}

fn no_brand() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "no brand yet: crawl the site first".into())
}

async fn get_profile(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Vec<Section>> {
    let _ = Project::load(&app.store, &slug)?;
    Ok(Json(profile::load_all(&app.store, &slug)?))
}

async fn get_profile_md(State(app): State<App>, Path(slug): Path<String>) -> Result<String, ApiError> {
    let project = Project::load(&app.store, &slug)?;
    let sections = profile::load_all(&app.store, &slug)?;
    let brief = Brief::load(&app.store, &slug)?;
    let brand = Brand::load(&app.store, &slug)?;
    Ok(profile::render_markdown(&project.name, &project.url, brief.as_ref(), brand.as_ref(), &sections))
}

async fn put_section(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(mut section): Json<Section>) -> ApiResult<Section> {
    let project = Project::load(&app.store, &slug)?;
    section.id = id.clone();
    let now = chrono::Utc::now();
    for f in &mut section.fields {
        if f.locked && f.edited_at.is_none() {
            f.edited_at = Some(now);
        }
    }
    section.updated_at = Some(now);
    section.save(&app.store, &slug)?;
    profile::write_markdown(&app.store, &slug, &project.name, &project.url)?;
    app.store.commit(&format!("profile({slug}): edit {id}")).await?;
    app.bus.emit(
        Some(&slug),
        EventKind::ProfileUpdated { section: id, by: "you".into(), title: section.title.clone(), items: profile::section_items(&section) },
    );
    Ok(Json(section))
}

#[derive(Serialize)]
struct Commit {
    hash: String,
    at: String,
    message: String,
}

async fn history(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Vec<Commit>> {
    let _ = Project::load(&app.store, &slug)?;
    let filter = format!("projects/{slug}");
    let rows = app.store.history(Some(&filter), 100).await?;
    Ok(Json(rows.into_iter().map(|(hash, at, message)| Commit { hash, at, message }).collect()))
}

// Assets: generated content lives outside git, so these routes emit events
// and never commit. Asset types are configuration and are committed.

async fn list_asset_types(State(app): State<App>) -> ApiResult<Vec<AssetType>> {
    Ok(Json(assets::types(&app.store)))
}

async fn save_asset_type(State(app): State<App>, Json(t): Json<AssetType>) -> ApiResult<AssetType> {
    let saved = assets::save_type(&app.store, &t)?;
    app.store.commit(&format!("asset type: {} ({})", saved.name, saved.id)).await?;
    app.bus.emit(None, EventKind::AssetTypeSaved { id: saved.id.clone(), name: saved.name.clone() });
    Ok(Json(saved))
}

#[derive(Deserialize)]
struct AssetFilter {
    #[serde(rename = "type")]
    kind: Option<String>,
    status: Option<String>,
    tag: Option<String>,
}

async fn list_assets(State(app): State<App>, Path(slug): Path<String>, Query(f): Query<AssetFilter>) -> ApiResult<Vec<Asset>> {
    let _ = Project::load(&app.store, &slug)?;
    let status = |a: &Asset| serde_json::to_value(a.status).ok().and_then(|v| v.as_str().map(str::to_string));
    let list = assets::list(&app.store, &slug)?
        .into_iter()
        .filter(|a| f.kind.as_ref().is_none_or(|k| &a.kind == k))
        .filter(|a| f.status.as_ref().is_none_or(|s| status(a).as_ref() == Some(s)))
        .filter(|a| f.tag.as_ref().is_none_or(|t| a.tags.contains(t)))
        .collect();
    Ok(Json(list))
}

async fn asset_summary(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Vec<TypeCount>> {
    let _ = Project::load(&app.store, &slug)?;
    Ok(Json(assets::summary(&app.store, &slug)?))
}

fn emit_saved(app: &App, slug: &str, a: &Asset, created: bool) {
    let status = serde_json::to_value(a.status).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    app.bus.emit(Some(slug), EventKind::AssetSaved { asset_type: a.kind.clone(), id: a.id.clone(), title: a.title.clone(), status, created });
}

async fn create_asset(State(app): State<App>, Path(slug): Path<String>, Json(new): Json<NewAsset>) -> Result<(StatusCode, Json<Asset>), ApiError> {
    let _ = Project::load(&app.store, &slug)?;
    let a = assets::create(&app.store, &slug, new)?;
    emit_saved(&app, &slug, &a, true);
    Ok((StatusCode::CREATED, Json(a)))
}

async fn get_asset(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>) -> ApiResult<Asset> {
    Ok(Json(assets::load(&app.store, &slug, &kind, &id)?))
}

/// Change title, status, tags, summary, source or publications. The type, id,
/// origin and file list stay as they are.
async fn put_asset(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>, Json(body): Json<Asset>) -> ApiResult<Asset> {
    let current = assets::load(&app.store, &slug, &kind, &id)?;
    if body.title.trim().is_empty() {
        return Err(ApiError(StatusCode::BAD_REQUEST, "an asset needs a title".into()));
    }
    let next = Asset { id: current.id, kind: current.kind, created: current.created, files: current.files, title: body.title.trim().to_string(), ..body };
    let saved = assets::save(&app.store, &slug, &next)?;
    emit_saved(&app, &slug, &saved, false);
    Ok(Json(saved))
}

async fn delete_asset(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>) -> Result<StatusCode, ApiError> {
    assets::delete(&app.store, &slug, &kind, &id)?;
    app.bus.emit(Some(&slug), EventKind::AssetDeleted { asset_type: kind, id });
    Ok(StatusCode::NO_CONTENT)
}

async fn get_asset_file(State(app): State<App>, Path((slug, kind, id, name)): Path<(String, String, String, String)>) -> Result<axum::response::Response, ApiError> {
    let path = assets::file_path(&app.store, &slug, &kind, &id, &name)?;
    let bytes = tokio::fs::read(&path).await.map_err(|e| ApiError(StatusCode::NOT_FOUND, format!("no asset file {name}: {e}")))?;
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    Ok(([(axum::http::header::CONTENT_TYPE, mime.to_string())], bytes).into_response())
}

#[derive(Deserialize)]
struct FileRole {
    role: Option<String>,
}

/// Upload one file as the raw request body. `?role=` says what it is (body, image, video…).
async fn put_asset_file(
    State(app): State<App>,
    Path((slug, kind, id, name)): Path<(String, String, String, String)>,
    Query(q): Query<FileRole>,
    body: axum::body::Bytes,
) -> ApiResult<Asset> {
    let a = assets::write_file(&app.store, &slug, &kind, &id, q.role.as_deref().unwrap_or("file"), &name, &body)?;
    emit_saved(&app, &slug, &a, false);
    Ok(Json(a))
}

#[derive(Deserialize)]
struct EventFilter {
    project: Option<String>,
    /// With `project`: first replay the project's latest crawl job (each frame
    /// marked `"replay": true`), then send `replay_done` with `running`, then
    /// live events. Lets a page opened mid-crawl, or after it, show the crawl.
    replay: Option<String>,
}

async fn events(State(app): State<App>, Query(q): Query<EventFilter>) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let mut replayed = Vec::new();
    let rx = match (&q.project, q.replay.is_some()) {
        (Some(p), true) => {
            if !app.bus.has_log(p) {
                if let Ok(text) = std::fs::read_to_string(crawl_log_file(&app, p)) {
                    app.bus.restore_log(p, text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect());
                }
            }
            let (log, rx) = app.bus.subscribe_with_log(p);
            for ev in &log.events {
                if let Ok(mut v) = serde_json::to_value(ev) {
                    v["replay"] = serde_json::Value::Bool(true);
                    replayed.push(v);
                }
            }
            replayed.push(serde_json::json!({ "type": "replay_done", "project": p, "at": chrono::Utc::now(), "running": log.open }));
            rx
        }
        _ => app.bus.subscribe(),
    };
    let filter = q.project;
    let replay = tokio_stream::iter(replayed).map(|v| Ok(SseEvent::default().data(v.to_string())));
    let live = BroadcastStream::new(rx).filter_map(move |ev| {
        let ev = ev.ok()?;
        if let Some(p) = &filter {
            if ev.project.as_deref() != Some(p.as_str()) {
                return None;
            }
        }
        Some(Ok(SseEvent::default().json_data(&ev).unwrap_or_else(|_| SseEvent::default().data("{}"))))
    });
    Sse::new(replay.chain(live)).keep_alive(KeepAlive::default())
}
