//! The KIQR daemon: one process that owns `~/.kiqr`, runs crawls and profile
//! synthesis, serves the HTTP API and SSE events, and serves the web UI.

mod api;
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
    if kiqr_core::extract::install_builtin_skills(&store)? {
        store.commit("skills: install built-in skills").await?;
    }
    seed_persona(&store).await?;

    let app = App {
        store,
        bus: EventBus::new(),
        config: Arc::new(RwLock::new(config.clone())),
        jobs: Default::default(),
        crawls: Default::default(),
    };

    let router = api::router(app.clone()).fallback(web::serve);
    // KIQR_HOST / KIQR_PORT override config.yaml, so Docker can bind 0.0.0.0 without editing files.
    let host = std::env::var("KIQR_HOST").unwrap_or_else(|_| config.daemon.host.clone());
    let port = std::env::var("KIQR_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(config.daemon.port);
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("KIQR daemon listening on http://{addr}  (home: {})", home.root().display());
    axum::serve(listener, router).await?;
    Ok(())
}

const PERSONA: &str = include_str!("../../../seemo/PERSONA.md");

async fn seed_persona(store: &Store) -> Result<()> {
    let path = store.home().seemo_dir().join("PERSONA.md");
    if !path.exists() {
        store.write_text(&path, PERSONA)?;
        store.commit("seemo: install persona").await?;
    }
    Ok(())
}
