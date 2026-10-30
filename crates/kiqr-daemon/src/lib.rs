#![recursion_limit = "256"]
//! The KIQR daemon: one process that owns `~/.kiqr`, runs crawls and profile
//! synthesis, serves the HTTP API and SSE events, and serves the web UI.

mod accounts;
mod agent;
mod api;
mod builtin;
mod connect;
mod ext;
mod images;
mod market;
mod marketing;
mod mcp_host;
mod metrics_src;
mod networks;
mod performance;
mod publish;
mod scheduler;
mod seemo;
mod signins;
mod smtp;
mod update;
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
    /// Sign-ins waiting for their callback, and tokens minted for service accounts.
    pub connects: connect::Shared,
    /// KIQR's own sign-ins (kiqr.cloud, UnblockingAPI): flows in progress, servers found.
    pub signins: signins::Shared,
    /// New KIQR versions: the last check, and the one update allowed at a time.
    pub updates: Arc<update::Updates>,
}

impl App {
    pub fn extractor(&self, config: &Config) -> Arc<Extractor> {
        Arc::new(Extractor::new(self.store.clone(), self.bus.clone(), config.clone()))
    }
}

pub async fn run(home: Home) -> Result<()> {
    let store = Store::open(home.clone())?;
    let _ = dotenvy::from_path(home.env_file());
    let mut config = Config::load(&store)?;
    // The default port was 7777 before 0.2 and nothing could change it, so a home on it moves with the default.
    if config.daemon.port == kiqr_core::config::OLD_DEFAULT_PORT {
        config.daemon.port = kiqr_core::config::DEFAULT_PORT;
        config.save(&store)?;
        store.commit(&format!("[auto] config: KIQR moves to port {}", config.daemon.port)).await?;
    }
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
    migrate_to_connections(&store, &config).await?;
    seed_persona(&store).await?;

    // KIQR_HOST / KIQR_PORT override config.yaml, so Docker can bind 0.0.0.0 without editing files.
    let (host, port) = listen_addr(&config.daemon);
    let loopback = if host == "0.0.0.0" || host == "::" || host.is_empty() { "127.0.0.1".to_string() } else { host.clone() };
    let exe = own_exe();
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
        exe: exe.clone(),
        registry: Arc::new(RwLock::new(kiqr_core::extensions::Registry::load(&home, &config.core_off))),
        renderer: Arc::new(kiqr_core::render::Renderer::new(home.render_dir())),
        mcp: Arc::new(mcp_host::McpHost::default()),
        connects: Default::default(),
        signins: Default::default(),
        updates: Arc::new(update::Updates::new(&exe)),
    };
    match app.renderer.chrome() {
        Some(c) => tracing::info!("images: SVG via resvg, HTML via {}", c.display()),
        None => tracing::info!("images: SVG via resvg; no browser found, so no HTML rendering"),
    }
    scheduler::recover(&app);
    tokio::spawn(scheduler::run(app.clone()));
    tokio::spawn(update::run(app.clone()));

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
    let listener = bind(&addr).await?;
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
/// Start this binary again with the same arguments: after an update (the new version)
/// or an address change (the new host and port). It replaces the process in place on Unix. On Unix the process is
/// replaced in place, so systemd and launchd see the same service. On Windows
/// the installer's scheduled task starts the new version (it allows parallel
/// instances, and the new one waits for the port), so `kiqr stop` and
/// Task Scheduler still own it; without the task a detached process starts.
/// Either way this one leaves.
pub(crate) fn restart_self(exe: &std::path::Path) {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(exe).args(&args).exec();
        tracing::error!(error = %err, "could not restart; exiting so the service manager starts KIQR again");
        std::process::exit(1);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const TASK: &str = "KIQR daemon";
        let quiet = |args: &[&str]| std::process::Command::new("schtasks").args(args).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success());
        if quiet(&["/Query", "/TN", TASK]) && quiet(&["/Run", "/TN", TASK]) {
            std::process::exit(0);
        }
        match std::process::Command::new(exe).args(&args).creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP).spawn() {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                tracing::error!(error = %e, "could not start KIQR again");
                std::process::exit(1);
            }
        }
    }
}

/// The address the daemon listens on: config.yaml, unless `KIQR_HOST` / `KIQR_PORT` override it.
pub(crate) fn listen_addr(cfg: &kiqr_core::config::DaemonConfig) -> (String, u16) {
    let host = std::env::var("KIQR_HOST").ok().filter(|h| !h.is_empty()).unwrap_or_else(|| cfg.host.clone());
    let port = std::env::var("KIQR_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(cfg.port);
    (host, port)
}

/// Restart on the new address once nothing is running: a chat turn, a task run
/// or a crawl would be cut off. Seemo changing the address from a chat is
/// itself a turn, so this waits for it to finish.
pub(crate) fn restart_when_idle(app: &App) {
    let app = app.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            if app.turns.read().await.is_empty() && app.jobs.read().await.is_empty() {
                break;
            }
        }
        tracing::info!("restarting on the new address");
        restart_self(&app.exe);
    });
}

/// Bind the API port, waiting a few seconds when it is still taken: after a
/// self-update on Windows the old process may not have let go of it yet.
async fn bind(addr: &str) -> std::io::Result<tokio::net::TcpListener> {
    let mut tries = 0;
    loop {
        match tokio::net::TcpListener::bind(addr).await {
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && tries < 20 => {
                tries += 1;
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            r => return r,
        }
    }
}

fn own_exe() -> std::path::PathBuf {
    let raw = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("kiqr"));
    let cleaned = raw.to_string_lossy().trim_end_matches(" (deleted)").to_string();
    let p = std::path::PathBuf::from(cleaned);
    p.canonicalize().unwrap_or(p)
}

/// Once per home: channels and the keys of installed abilities become each
/// project's connections (`kiqr_core::connections::migrate`), and an ability
/// switched off for the whole machine becomes switched off in every project,
/// since the per-project switch is the only one an installed ability has now.
async fn migrate_to_connections(store: &kiqr_core::Store, config: &Config) -> anyhow::Result<()> {
    use kiqr_core::extensions::{Registry, Scope};
    let reg = Registry::load(store.home(), &config.core_off);
    let manifests: Vec<kiqr_core::extensions::Manifest> = reg.entries.iter().filter(|e| e.error.is_none()).filter_map(|e| e.manifest.clone()).collect();
    let Some(done) = kiqr_core::connections::migrate(store, &manifests)? else { return Ok(()) };
    let paused: Vec<&kiqr_core::extensions::Entry> = reg.entries.iter().filter(|e| e.paused && e.scope == Scope::Global && !e.is_core()).collect();
    if !paused.is_empty() {
        for mut p in kiqr_core::project::Project::list(store).unwrap_or_default() {
            let before = p.abilities_off.len();
            for e in &paused {
                if !p.abilities_off.iter().any(|x| x == e.id()) {
                    p.abilities_off.push(e.id().to_string());
                }
            }
            if p.abilities_off.len() != before {
                p.save(store)?;
            }
        }
        for e in &paused {
            let _ = std::fs::remove_file(e.dir.join(".off"));
        }
    }
    tracing::info!(channels = done.channels, accounts = done.connections, keys = done.keys_moved, switched_off = paused.len(), "abilities now keep their keys on each project's accounts");
    store.commit("[auto] connections: one account per channel and per project").await?;
    Ok(())
}
