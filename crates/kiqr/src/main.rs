//! `kiqr`: the command-line client. Every command talks to the daemon over
//! HTTP, so the CLI and the web UI always see the same state. `kiqr daemon`
//! runs the daemon itself.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use kiqr_core::events::{Event, EventKind};
use kiqr_core::Home;

#[derive(Parser)]
#[command(name = "kiqr", version, about = "KIQR — your self-hosted CMO", long_about = None)]
struct Cli {
    /// Daemon address. Defaults to config.yaml, then http://127.0.0.1:7777
    #[arg(long = "daemon-url", env = "KIQR_URL", global = true)]
    daemon_url: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the daemon in the foreground (the web UI is served from it)
    Daemon,
    /// Show daemon status, providers and the active home directory
    Status,
    /// Manage projects
    Project {
        #[command(subcommand)]
        cmd: ProjectCmd,
    },
    /// Crawl a project's website and build its profile
    Crawl {
        slug: String,
        /// Override the page limit for this crawl
        #[arg(long)]
        max_pages: Option<usize>,
        /// Stream progress until the crawl finishes
        #[arg(long, short)]
        watch: bool,
    },
    /// Show a project's business profile
    Profile {
        slug: String,
        /// Print the profile as YAML sections instead of Markdown
        #[arg(long)]
        yaml: bool,
    },
    /// Rebuild the profile from the facts already extracted
    Synthesize { slug: String },
    /// Show what changed, newest first
    History { slug: String },
    /// Follow live events (all projects, or one with --project)
    Events {
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum ProjectCmd {
    /// Add a project from its website URL and start crawling
    Add {
        url: String,
        /// Create the project without crawling
        #[arg(long)]
        no_crawl: bool,
        /// Stream the crawl until it finishes
        #[arg(long, short)]
        watch: bool,
    },
    /// List projects
    List,
    /// Remove a project and everything under it
    Remove { slug: String },
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Daemon) => {
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,kiqr_core=info".into()))
                .init();
            kiqr_daemon::run(Home::resolve()).await
        }
        None => {
            Cli::parse_from(["kiqr", "--help"]);
            Ok(())
        }
        Some(cmd) => {
            let client = Client::new(cli.daemon_url)?;
            client.run(cmd).await
        }
    }
}

struct Client {
    base: String,
    http: reqwest::Client,
}

impl Client {
    fn new(url: Option<String>) -> Result<Self> {
        let base = url.unwrap_or_else(|| {
            let home = Home::resolve();
            let cfg = std::fs::read_to_string(home.config_file())
                .ok()
                .and_then(|t| serde_yaml_lite_port(&t));
            cfg.unwrap_or_else(|| "http://127.0.0.1:7777".into())
        });
        // The daemon is local; never route to it through a system proxy.
        let http = reqwest::Client::builder().no_proxy().build()?;
        Ok(Self { base: base.trim_end_matches('/').to_string(), http })
    }

    async fn run(&self, cmd: Command) -> Result<()> {
        match cmd {
            Command::Daemon => unreachable!(),
            Command::Status => {
                let v: serde_json::Value = self.get("/api/health").await?;
                println!("KIQR {}  home {}", v["version"].as_str().unwrap_or("?"), v["home"].as_str().unwrap_or("?"));
                println!("UnblockingAPI: {}", if v["unblockingapi"].as_bool().unwrap_or(false) { "connected" } else { "not connected" });
                for p in v["providers"].as_array().cloned().unwrap_or_default() {
                    println!(
                        "{:<11} {:<32} {}",
                        p["task"].as_str().unwrap_or(""),
                        p["model"].as_str().unwrap_or(""),
                        if p["available"].as_bool().unwrap_or(false) { "available" } else { "not available" }
                    );
                }
            }
            Command::Project { cmd } => match cmd {
                ProjectCmd::Add { url, no_crawl, watch } => {
                    let v: serde_json::Value = self.post("/api/projects", &serde_json::json!({ "url": url, "crawl": !no_crawl })).await?;
                    let slug = v["slug"].as_str().unwrap_or("").to_string();
                    println!("Added {slug} ({})", v["url"].as_str().unwrap_or(""));
                    if !no_crawl && watch {
                        self.watch(&slug).await?;
                    } else if !no_crawl {
                        println!("Crawling in the background. Follow it with: kiqr crawl {slug} --watch");
                    }
                }
                ProjectCmd::List => {
                    let v: Vec<serde_json::Value> = self.get("/api/projects").await?;
                    if v.is_empty() {
                        println!("No projects yet. Add one: kiqr project add https://example.com");
                    }
                    for p in v {
                        let last = p["last_crawl"]["pages"].as_u64().map(|n| format!("{n} pages")).unwrap_or_else(|| "not crawled".into());
                        println!("{:<24} {:<40} {last}", p["slug"].as_str().unwrap_or(""), p["url"].as_str().unwrap_or(""));
                    }
                }
                ProjectCmd::Remove { slug } => {
                    self.delete(&format!("/api/projects/{slug}")).await?;
                    println!("Removed {slug}");
                }
            },
            Command::Crawl { slug, max_pages, watch } => {
                let q = max_pages.map(|n| format!("?max_pages={n}")).unwrap_or_default();
                let _: serde_json::Value = self.post(&format!("/api/projects/{slug}/crawl{q}"), &serde_json::json!({})).await?;
                if watch {
                    self.watch(&slug).await?;
                } else {
                    println!("Crawling {slug} in the background. Follow it with: kiqr crawl {slug} --watch");
                }
            }
            Command::Profile { slug, yaml } => {
                if yaml {
                    let v: serde_json::Value = self.get(&format!("/api/projects/{slug}/profile")).await?;
                    println!("{}", serde_json::to_string_pretty(&v)?);
                } else {
                    let text = self.http.get(format!("{}/api/projects/{slug}/profile.md", self.base)).send().await?.error_for_status()?.text().await?;
                    println!("{text}");
                }
            }
            Command::Synthesize { slug } => {
                let _: serde_json::Value = self.post(&format!("/api/projects/{slug}/synthesize"), &serde_json::json!({})).await?;
                println!("Building the profile for {slug}. Watch: kiqr events --project {slug}");
            }
            Command::History { slug } => {
                let v: Vec<serde_json::Value> = self.get(&format!("/api/projects/{slug}/history")).await?;
                for c in v {
                    println!("{}  {}  {}", c["hash"].as_str().unwrap_or(""), &c["at"].as_str().unwrap_or("")[..16.min(c["at"].as_str().unwrap_or("").len())], c["message"].as_str().unwrap_or(""));
                }
            }
            Command::Events { project } => {
                self.stream_events(project.as_deref(), |_| false).await?;
            }
        }
        Ok(())
    }

    async fn watch(&self, slug: &str) -> Result<()> {
        self.stream_events(Some(slug), |ev| matches!(ev.kind, EventKind::CrawlFinished { .. })).await
    }

    async fn stream_events(&self, project: Option<&str>, stop_when: impl Fn(&Event) -> bool) -> Result<()> {
        let url = match project {
            Some(p) => format!("{}/api/events?project={p}", self.base),
            None => format!("{}/api/events", self.base),
        };
        let resp = self.http.get(&url).send().await.with_context(|| format!("connecting to {url}"))?.error_for_status()?;
        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        while let Some(chunk) = stream.next().await {
            buf.push_str(&String::from_utf8_lossy(&chunk?));
            while let Some(i) = buf.find("\n\n") {
                let frame = buf[..i].to_string();
                buf.drain(..i + 2);
                for line in frame.lines() {
                    if let Some(data) = line.strip_prefix("data:") {
                        if let Ok(ev) = serde_json::from_str::<Event>(data.trim()) {
                            print_event(&ev);
                            if stop_when(&ev) {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self.http.get(format!("{}{path}", self.base)).send().await.map_err(connect_hint)?;
        Self::json(resp).await
    }
    async fn post<T: serde::de::DeserializeOwned>(&self, path: &str, body: &serde_json::Value) -> Result<T> {
        let resp = self.http.post(format!("{}{path}", self.base)).json(body).send().await.map_err(connect_hint)?;
        Self::json(resp).await
    }
    async fn delete(&self, path: &str) -> Result<()> {
        let resp = self.http.delete(format!("{}{path}", self.base)).send().await.map_err(connect_hint)?;
        if resp.status().is_success() {
            Ok(())
        } else {
            let v: serde_json::Value = resp.json().await.unwrap_or_default();
            Err(anyhow!("{}", v["error"].as_str().unwrap_or("request failed")))
        }
    }
    async fn json<T: serde::de::DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            return Err(anyhow!("{}", v["error"].as_str().unwrap_or(&text)));
        }
        serde_json::from_str(&text).context("unexpected response from daemon")
    }
}

fn connect_hint(e: reqwest::Error) -> anyhow::Error {
    if e.is_connect() {
        anyhow!("can't reach the daemon. Start it with: kiqr daemon")
    } else {
        e.into()
    }
}

/// Read `daemon.host`/`daemon.port` without pulling a YAML parser into the CLI.
fn serde_yaml_lite_port(text: &str) -> Option<String> {
    let mut host = None;
    let mut port = None;
    let mut in_daemon = false;
    for line in text.lines() {
        if line.starts_with("daemon:") {
            in_daemon = true;
            continue;
        }
        if in_daemon && !line.starts_with(' ') {
            in_daemon = false;
        }
        if in_daemon {
            let l = line.trim();
            if let Some(v) = l.strip_prefix("host:") {
                host = Some(v.trim().trim_matches('"').to_string());
            } else if let Some(v) = l.strip_prefix("port:") {
                port = v.trim().parse::<u16>().ok();
            }
        }
    }
    match (host, port) {
        (Some(h), Some(p)) => Some(format!("http://{h}:{p}")),
        _ => None,
    }
}

fn print_event(ev: &Event) {
    let t = ev.at.format("%H:%M:%S");
    let line = match &ev.kind {
        EventKind::ProjectCreated { slug, url } => format!("project {slug} added ({url})"),
        EventKind::CrawlStarted { max_pages } => format!("reading up to {max_pages} pages"),
        EventKind::PageDiscovered { url } => format!("found    {url}"),
        EventKind::PageFetching { url, via } => format!("reading  {url}{}", if via == "unblockingapi" { "  via UnblockingAPI" } else { "" }),
        EventKind::PageFetched { url, status, ms, via, .. } => format!("read     {url}  {status} {ms} ms{}", if via == "unblockingapi" { " via UnblockingAPI" } else { "" }),
        EventKind::PageBlocked { url, reason } => format!("blocked  {url}  {reason}"),
        EventKind::PageParsed { url, facts } => format!("parsed   {url}  {facts} facts"),
        EventKind::PageFailed { url, error } => format!("failed   {url}  {error}"),
        EventKind::CrawlFinished { pages, blocked, failed } => format!("done: {pages} pages read, {blocked} blocked, {failed} failed"),
        EventKind::ProfileUpdated { section, by } => format!("profile  {section} updated by {by}"),
        EventKind::SeemoSays { text } => format!("Seemo: {text}"),
        EventKind::Committed { message } => format!("saved    {message}"),
    };
    println!("{t}  {line}");
}
