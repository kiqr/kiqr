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
    /// Serialises saving the brand of projects crawled before brand collection.
    pub brand_backfill: Arc<tokio::sync::Mutex<()>>,
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
        brand_backfill: Default::default(),
    };

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
