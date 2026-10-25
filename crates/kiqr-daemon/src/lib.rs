#![recursion_limit = "256"]
//! The KIQR daemon: one process that owns `~/.kiqr`, runs crawls and profile
//! synthesis, serves the HTTP API and SSE events, and serves the web UI.

mod agent;
mod api;
mod builtin;
mod ext;
mod images;
mod market;
mod marketing;
mod mcp_host;
mod metrics_src;
mod performance;
mod publish;
mod scheduler;
mod seemo;
mod web;

use anyhow::Result;
use kiqr_core::{extract::Extractor, Config, EventBus, Home, Store};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct App {
    pub store: Store,
    pub bus: EventBus,
    pub config: Arc<RwLock<Config>>,
    pub jobs: Arc<RwLock<std::collections::HashSet<String>>>,
    /// Page-limit handles of the crawls running now, by project.
    pub crawls: Arc<RwLock<std::collections::HashMap<String, Arc<kiqr_core::crawl::CrawlControl>>>>,
    /// Serialises saving the brand of projects crawled before brand collection.
    pub brand_backfill: Arc<tokio::sync::Mutex<()>>,
    /// Chat turns in progress, by chat id.
    pub turns: Arc<RwLock<std::collections::HashMap<String, Arc<agent::Turn>>>>,
    /// Approvals Seemo is waiting for (and recent decisions), by approval id.
    pub approvals: Arc<RwLock<std::collections::HashMap<String, Arc<seemo::Approval>>>>,
    /// Caps the agent processes running at once.
    pub agents: Arc<tokio::sync::Semaphore>,
    /// Follow-up notes waiting for a chat's turn to end, by chat id.
    pub queued: Arc<RwLock<std::collections::HashMap<String, std::collections::VecDeque<String>>>>,
    /// Where `kiqr mcp` reaches this daemon.
    pub daemon_url: String,
    /// This binary, resolved once: after an upgrade or rebuild the running
    /// process would otherwise see its own path as deleted.
    pub exe: std::path::PathBuf,
    /// Abilities: every folder under `abilities/`, reloaded on change.
    pub registry: Arc<RwLock<kiqr_core::extensions::Registry>>,
    /// The built-in image renderer: fonts loaded once, browser found once.
    pub renderer: Arc<kiqr_core::render::Renderer>,
    /// MCP servers the daemon hosts for `transport: mcp` connections.
    pub mcp: Arc<mcp_host::McpHost>,
}

impl App {
    pub fn extractor(&self, config: &Config) -> Arc<Extractor> {
        Arc::new(Extractor::new(self.store.clone(), self.bus.clone(), config.clone()))
    }
}

pub async fn run(home: Home) -> Result<()> {
    let store = Store::open(home.clone())?;
    let _ = dotenvy::from_path(home.env_file());
    let config = Config::load(&store)?;
    // Homes from before the rename keep their folders, under the new name.
    let moved = home.move_ability_folders()?;
    if moved > 0 {
        tracing::info!(folders = moved, "ability folders moved from skills/ to abilities/");
        store.commit("[auto] abilities: folders moved from skills/").await?;
    }
    if kiqr_core::extract::install_builtin_instructions(&store)? {
        store.commit("abilities: install the built-in instructions").await?;
    }
    if kiqr_core::assets::install_builtin_types(&store)? {
        store.commit("asset types: install built-in types").await?;
    }
    if kiqr_core::extensions::install_builtin(&store)? {
        store.commit("abilities: install the built-in abilities").await?;
    }
    // Older versions installed every shipped ability; now the marketplace leaves that to the user.
    // The marketplace index is cached for an hour; a start reads it fresh, so a
    // restart is a sure way to see a change in the index.
    let _ = std::fs::remove_file(home.root().join("tmp").join("marketplace.json"));
    let retired = kiqr_core::extensions::retire_auto_installed(&store, kiqr_core::provider::has_env)?;
    if !retired.is_empty() {
        tracing::info!(folders = ?retired, "abilities that were never set up now wait in the marketplace");
        store.commit(&format!("abilities: {} wait in the marketplace until installed", retired.join(", "))).await?;
    }
    seed_persona(&store).await?;

    // KIQR_HOST / KIQR_PORT override config.yaml, so Docker can bind 0.0.0.0 without editing files.
    let host = std::env::var("KIQR_HOST").unwrap_or_else(|_| config.daemon.host.clone());
    let port = std::env::var("KIQR_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(config.daemon.port);
    let loopback = if host == "0.0.0.0" || host == "::" || host.is_empty() { "127.0.0.1".to_string() } else { host.clone() };
    let app = App {
        store,
        bus: EventBus::new(),
        config: Arc::new(RwLock::new(config.clone())),
        jobs: Default::default(),
        crawls: Default::default(),
        brand_backfill: Default::default(),
        turns: Default::default(),
        approvals: Default::default(),
        agents: Arc::new(tokio::sync::Semaphore::new(config.agent.max_concurrent.clamp(1, 10))),
        queued: Default::default(),
        daemon_url: format!("http://{loopback}:{port}"),
        exe: own_exe(),
        registry: Arc::new(RwLock::new(kiqr_core::extensions::Registry::load(&home, &config.core_off))),
        renderer: Arc::new(kiqr_core::render::Renderer::new(home.render_dir())),
        mcp: Arc::new(mcp_host::McpHost::default()),
    };
    match app.renderer.chrome() {
        Some(c) => tracing::info!("images: SVG via resvg, HTML via {}", c.display()),
        None => tracing::info!("images: SVG via resvg; no browser found, so no HTML rendering"),
    }
    scheduler::recover(&app);
    tokio::spawn(scheduler::run(app.clone()));

    // Projects crawled before brand collection existed, or whose brand was
    // built by older rules, get it saved now, in the background, so the
    // profile page doesn't wait for it.
    {
        let app = app.clone();
        tokio::spawn(async move {
            let Ok(projects) = kiqr_core::project::Project::list(&app.store) else { return };
            for p in projects {
                if matches!(kiqr_core::brand::Brand::load(&app.store, &p.slug), Ok(None)) || matches!(kiqr_core::brand::Brand::load(&app.store, &p.slug), Ok(Some(b)) if b.outdated()) {
                    if let Err(e) = api::backfill_brand(&app, &p).await {
                        tracing::warn!(slug = %p.slug, error = %e, "could not save the brand");
                    }
                }
            }
        });
    }

    let router = api::router(app.clone()).fallback(web::serve);
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("KIQR daemon listening on http://{addr}  (home: {})", home.root().display());
    axum::serve(listener, router).await?;
    Ok(())
}

pub(crate) const PERSONA: &str = include_str!("../../../seemo/PERSONA.md");

async fn seed_persona(store: &Store) -> Result<()> {
    let path = store.home().seemo_dir().join("PERSONA.md");
    if !path.exists() {
        store.write_text(&path, PERSONA)?;
        store.commit("seemo: install persona").await?;
    }
    Ok(())
}

/// The path of this binary as it should be spawned later.
fn own_exe() -> std::path::PathBuf {
    let raw = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("kiqr"));
    let cleaned = raw.to_string_lossy().trim_end_matches(" (deleted)").to_string();
    let p = std::path::PathBuf::from(cleaned);
    p.canonicalize().unwrap_or(p)
}
