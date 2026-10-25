//! `kiqr`: the command-line client. Every command talks to the daemon over
//! HTTP, so the CLI and the web UI always see the same state. `kiqr daemon`
//! runs the daemon itself.

mod mcp;

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
    /// Serve KIQR's tools to a model over MCP on stdin/stdout. The daemon
    /// starts this for Seemo's sessions; you rarely run it yourself.
    #[command(hide = true)]
    Mcp,
    /// Talk to Seemo. Without a message, lists the project's chats.
    Chat {
        slug: String,
        /// What to say. Seemo's reply streams here.
        message: Option<String>,
        /// Continue this chat instead of starting a new one
        #[arg(long)]
        chat: Option<String>,
    },
    /// Read a web page the way Seemo does (KIQR's fetcher, UnblockingAPI fallback); prints Markdown
    Read {
        slug: String,
        url: String,
    },
    /// Seemo's scheduled tasks
    Tasks {
        #[command(subcommand)]
        cmd: TasksCmd,
    },
    /// Abilities: what Seemo can do; list, read and teach instructions, install, update, pause, remove
    Abilities {
        #[command(subcommand)]
        cmd: AbilitiesCmd,
    },
    /// Render an SVG or HTML file to a PNG on an asset, as Seemo's create_image does
    Render {
        slug: String,
        /// File to render (.svg or .html)
        file: std::path::PathBuf,
        /// Put it on this asset (`type/id`); default: a new image asset
        #[arg(long)]
        asset: Option<String>,
        /// File name without extension (default: image)
        #[arg(long)]
        name: Option<String>,
        /// x | linkedin | instagram | instagram_portrait | og | cover | WxH
        #[arg(long)]
        size: Option<String>,
        /// Title for a new image asset
        #[arg(long)]
        title: Option<String>,
    },
    /// Channels: where a project publishes (an email address, a blog…)
    Channels { slug: String },
    /// Publish an asset through a channel: `kiqr publish <slug> <type>/<id> <channel>`
    Publish {
        slug: String,
        asset: String,
        channel: String,
        #[arg(long, default_value = "")]
        note: String,
    },
    /// Send an email through the mail connection (the text is Markdown)
    Email {
        slug: String,
        to: String,
        subject: String,
        text: String,
        /// Attach this asset's files: `type/id`
        #[arg(long)]
        asset: Option<String>,
    },
    /// Secrets in .env, by name; values are typed, never shown
    Secrets {
        #[command(subcommand)]
        cmd: SecretsCmd,
    },
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
        /// Page limit for this crawl (and later ones); raises a running crawl's limit
        #[arg(long)]
        max_pages: Option<usize>,
        /// Pages fetched in parallel (1–10)
        #[arg(long)]
        concurrency: Option<usize>,
        /// Pages read by the model in parallel (1–10)
        #[arg(long)]
        ai_concurrency: Option<usize>,
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
    /// Show a project's business brief: summary, offerings, audiences, voice, topic clusters and content gaps
    Brief {
        slug: String,
        /// Print the brief as JSON
        #[arg(long)]
        json: bool,
        /// Write the brief again from the current profile, in the background
        #[arg(long)]
        regenerate: bool,
    },
    /// Show a project's brand assets: logo, icon, colours, fonts, imagery and social links
    Brand {
        slug: String,
        /// Print the brand as JSON
        #[arg(long)]
        json: bool,
        /// Collect the brand again from the crawl cache and save it
        #[arg(long)]
        rebuild: bool,
    },
    /// List, show or remove a project's assets: articles, posts, images, videos…
    Assets {
        #[command(subcommand)]
        cmd: AssetsCmd,
    },
    /// What waits for you in a project: approvals open in chats, drafts to approve, publishes coming up
    Todo { slug: String },
    /// List the asset types (built-in and your own)
    Types,
    /// Show what changed, newest first
    History { slug: String },
    /// Follow live events (all projects, or one with --project)
    Events {
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum TasksCmd {
    /// List a project's tasks with their schedule and last run
    Ls { slug: String },
    /// Show a task and its runs
    Show { slug: String, task: String },
    /// Run a task now and follow it
    Run { slug: String, task: String },
    /// Pause a task
    Pause { slug: String, task: String },
    /// Resume a paused task
    Resume { slug: String, task: String },
    /// Remove a task and its runs
    Rm { slug: String, task: String },
}

#[derive(Subcommand)]
enum AbilitiesCmd {
    /// What Seemo can do here, then what the marketplace offers
    Ls {
        /// See it as one project sees it (its own keys, copies and switches)
        #[arg(long)]
        project: Option<String>,
    },
    /// Print an ability's instructions (`--file` picks one when it has several)
    Show {
        id: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        file: Option<String>,
    },
    /// Teach Seemo an ability from a SKILL.md, or replace one's instructions (yours, or a copy of KIQR's)
    Teach {
        id: String,
        /// The SKILL.md to install as its instructions
        #[arg(long)]
        file: std::path::PathBuf,
        /// For one project only (default: every project)
        #[arg(long)]
        project: Option<String>,
    },
    /// Install one by id, everywhere or for one project; keys are asked for
    Install {
        id: String,
        #[arg(long)]
        project: Option<String>,
    },
    /// Pull the newest version (git), or refresh a shipped one
    Update {
        id: String,
        #[arg(long)]
        project: Option<String>,
    },
    /// Switch one off without removing it
    Pause {
        id: String,
        #[arg(long)]
        project: Option<String>,
    },
    /// Switch a paused one on again
    Resume {
        id: String,
        #[arg(long)]
        project: Option<String>,
    },
    /// Remove an installed ability's folder
    Rm {
        id: String,
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum SecretsCmd {
    /// Set a secret: `kiqr secrets set FAL_KEY` then type it (an empty value clears it)
    Set {
        name: String,
        /// Set it for one project only (its own .env wins over the machine's)
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum AssetsCmd {
    /// List assets, newest change first
    Ls {
        slug: String,
        /// Only this type (article, post, meme…)
        #[arg(long = "type")]
        kind: Option<String>,
    },
    /// Show one asset: `kiqr assets show <slug> <type>/<id>`
    Show { slug: String, asset: String },
    /// Remove one asset and its files
    Rm { slug: String, asset: String },
    /// Approve an asset for a channel; it becomes ready: `kiqr assets approve <slug> post/x --channel blog`
    Approve {
        slug: String,
        asset: String,
        #[arg(long)]
        channel: String,
    },
    /// Publish now, through the channel it was approved for or the one given
    Publish {
        slug: String,
        asset: String,
        #[arg(long)]
        channel: Option<String>,
    },
    /// Publish at a time (RFC 3339, e.g. 2026-11-02T09:00:00+01:00); `--clear` drops the time
    Schedule {
        slug: String,
        asset: String,
        #[arg(long)]
        at: Option<String>,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        clear: bool,
    },
    /// Stop counting it as published (the copy on the channel stays)
    Unpublish { slug: String, asset: String },
    /// Put it in the archive, or bring it back with --undo
    Archive {
        slug: String,
        asset: String,
        #[arg(long)]
        undo: bool,
    },
    /// Have Seemo write assets from the business profile (posts for now)
    Generate {
        slug: String,
        #[arg(long = "type", default_value = "post")]
        kind: String,
        #[arg(long, default_value_t = 3)]
        count: usize,
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
        Some(Command::Mcp) => {
            let client = Client::new(cli.daemon_url)?;
            mcp::serve(client).await
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

/// Read one line from the terminal without echoing it.
fn read_secret() -> Result<String> {
    use std::io::{BufRead, IsTerminal};
    let stdin = std::io::stdin();
    let tty = stdin.is_terminal();
    let mut saved: Option<libc::termios> = None;
    if tty {
        unsafe {
            let mut t: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(0, &mut t) == 0 {
                saved = Some(t);
                let mut off = t;
                off.c_lflag &= !libc::ECHO;
                libc::tcsetattr(0, libc::TCSANOW, &off);
            }
        }
    }
    let mut line = String::new();
    let r = stdin.lock().read_line(&mut line);
    if let Some(t) = saved {
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &t);
        }
        eprintln!();
    }
    r?;
    Ok(line.trim_end_matches(['\r', '\n']).to_string())
}

pub(crate) struct Client {
    pub(crate) base: String,
    pub(crate) http: reqwest::Client,
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
        // The daemon is local; never route to it through a system proxy. Long
        // timeout: approval polls wait up to two minutes.
        let http = reqwest::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(180)).build()?;
        Ok(Self { base: base.trim_end_matches('/').to_string(), http })
    }

    async fn run(&self, cmd: Command) -> Result<()> {
        match cmd {
            Command::Daemon | Command::Mcp => unreachable!(),
            Command::Chat { slug, message, chat } => match message {
                None => {
                    let v: Vec<serde_json::Value> = self.get(&format!("/api/projects/{slug}/chats")).await?;
                    if v.is_empty() {
                        println!("No chats yet. Start one: kiqr chat {slug} \"What do you know about us?\"");
                    }
                    for c in v {
                        let s = |k: &str| c[k].as_str().unwrap_or("").to_string();
                        println!("{:<24} {:<16} {:<40} {}", s("id"), &s("updated_at")[..16.min(s("updated_at").len())], s("title"), s("last_message"));
                    }
                }
                Some(text) => {
                    let id = match chat {
                        Some(id) => id,
                        None => {
                            let v: serde_json::Value = self.post(&format!("/api/projects/{slug}/chats"), &serde_json::json!({})).await?;
                            v["id"].as_str().unwrap_or("").to_string()
                        }
                    };
                    let _: serde_json::Value = self.post(&format!("/api/projects/{slug}/chats/{id}/messages"), &serde_json::json!({ "text": text })).await?;
                    eprintln!("chat {id}");
                    self.stream_chat(&slug, &id).await?;
                }
            },
            Command::Read { slug, url } => {
                let v: serde_json::Value = self.get(&format!("/api/projects/{slug}/web?url={}", mcp::urlencode(&url))).await?;
                let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
                eprintln!("{} ({}, via {}{})", s("title"), v["status"], s("via"), if s("obstacle").is_empty() { String::new() } else { format!(", {}", s("obstacle")) });
                println!("{}", s("markdown"));
                if v["truncated"].as_bool() == Some(true) {
                    eprintln!("[cut; the page is longer]");
                }
            }
            Command::Abilities { cmd } => match &cmd {
                AbilitiesCmd::Ls { project } => {
                    let q = project.as_ref().map(|p| format!("?project={p}")).unwrap_or_default();
                    let v: serde_json::Value = self.get(&format!("/api/abilities{q}")).await?;
                    let row = |it: &serde_json::Value| {
                        let s = |k: &str| it[k].as_str().unwrap_or("").to_string();
                        let state = if it["installed"].as_bool() != Some(true) {
                            "available".to_string()
                        } else if it["machine_off"].as_bool() == Some(true) {
                            "off".to_string()
                        } else if it["paused"].as_bool() == Some(true) {
                            "off here".to_string()
                        } else if it["ready"].as_bool() == Some(true) {
                            if it["core"].as_bool() == Some(true) { "on (core)".to_string() } else { "on".to_string() }
                        } else {
                            format!("needs {}", it["missing"].as_array().map(|m| m.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default())
                        };
                        let what: Vec<&str> = [(!it["instructions"].as_array().map(|a| a.is_empty()).unwrap_or(true), "instructions"), (!it["auth"].as_array().map(|a| a.is_empty()).unwrap_or(true), "key"), (it["runs_program"].as_bool() == Some(true), "program")].iter().filter(|(on, _)| *on).map(|(_, w)| *w).collect();
                        println!("{:<22} {:<44} {:<10} {:<22} {}", s("id"), s("name"), s("author"), state, what.join(", "));
                    };
                    for it in v["abilities"].as_array().cloned().unwrap_or_default() {
                        row(&it);
                    }
                    let market = v["market"].as_array().cloned().unwrap_or_default();
                    if !market.is_empty() {
                        println!("\nMarketplace");
                        for it in &market {
                            row(it);
                        }
                    }
                    println!();
                    for a in v["contracts"].as_array().cloned().unwrap_or_default() {
                        let active = a["active"]["name"].as_str().map(|n| format!("via {n}")).unwrap_or_else(|| "off (nothing installed)".into());
                        println!("{:<16} {}", a["label"].as_str().unwrap_or(""), active);
                    }
                }
                AbilitiesCmd::Show { id, project, file } => {
                    let path = match project {
                        Some(p) => format!("/api/projects/{p}/abilities/{id}/instructions"),
                        None => format!("/api/abilities/{id}/instructions"),
                    };
                    let q = format!("?all=1{}", file.as_ref().map(|f| format!("&file={f}")).unwrap_or_default());
                    let resp = self.http.get(format!("{}{path}{q}", self.base)).send().await.map_err(connect_hint)?;
                    if !resp.status().is_success() {
                        let v: serde_json::Value = resp.json().await.unwrap_or_default();
                        return Err(anyhow!("{}", v["error"].as_str().unwrap_or("request failed")));
                    }
                    println!("{}", resp.text().await?);
                }
                AbilitiesCmd::Teach { id, file, project } => {
                    let text = std::fs::read_to_string(file).map_err(|e| anyhow!("could not read {}: {e}", file.display()))?;
                    let path = match project {
                        Some(p) => format!("/api/projects/{p}/abilities/{id}/instructions"),
                        None => format!("/api/abilities/{id}/instructions"),
                    };
                    let resp = self.http.put(format!("{}{path}", self.base)).json(&serde_json::json!({ "text": text })).send().await.map_err(connect_hint)?;
                    if !resp.status().is_success() {
                        let v: serde_json::Value = resp.json().await.unwrap_or_default();
                        return Err(anyhow!("{}", v["error"].as_str().unwrap_or("request failed")));
                    }
                    let v: serde_json::Value = resp.json().await.unwrap_or_default();
                    println!("{} now knows how to: {}", if project.is_some() { "This project" } else { "Every project" }, v["name"].as_str().unwrap_or(id));
                }
                AbilitiesCmd::Install { id, project } => {
                    let q = project.as_ref().map(|p| format!("?project={p}")).unwrap_or_default();
                    let list: serde_json::Value = self.get(&format!("/api/abilities{q}")).await?;
                    let item = list["market"].as_array().and_then(|a| a.iter().find(|i| i["id"] == id.as_str())).cloned().ok_or_else(|| anyhow::anyhow!("the marketplace has no ability '{id}' (or it is already installed)"))?;
                    let mut secrets = serde_json::Map::new();
                    for a in item["auth"].as_array().cloned().unwrap_or_default() {
                        if a["set"].is_string() {
                            continue;
                        }
                        let env = a["env"].as_str().unwrap_or("");
                        eprint!("{} ({env}{}): ", a["label"].as_str().unwrap_or(env), if a["optional"].as_bool() == Some(true) { ", optional, Enter to skip" } else { "" });
                        let value = read_secret()?;
                        if !value.trim().is_empty() {
                            secrets.insert(env.to_string(), serde_json::Value::String(value));
                        }
                    }
                    let body = serde_json::json!({ "scope": if project.is_some() { "project" } else { "global" }, "project": project, "secrets": secrets });
                    let v: serde_json::Value = self.post(&format!("/api/abilities/{id}/install"), &body).await?;
                    println!("installed {} ({})", id, v["folder"].as_str().unwrap_or(""));
                }
                AbilitiesCmd::Update { id, project } => {
                    let q = project.as_ref().map(|p| format!("?project={p}")).unwrap_or_default();
                    let v: serde_json::Value = self.post(&format!("/api/abilities/{id}/update{q}"), &serde_json::json!({})).await?;
                    println!("{id}: {}", v["detail"].as_str().unwrap_or("updated"));
                }
                AbilitiesCmd::Pause { id, project } | AbilitiesCmd::Resume { id, project } => {
                    let pause = matches!(&cmd, AbilitiesCmd::Pause { .. });
                    let q = project.as_ref().map(|p| format!("?project={p}")).unwrap_or_default();
                    let _: serde_json::Value = self.post(&format!("/api/abilities/{id}/{}{q}", if pause { "pause" } else { "resume" }), &serde_json::json!({})).await?;
                    println!("{id} {}", if pause { "paused" } else { "on again" });
                }
                AbilitiesCmd::Rm { id, project } => {
                    let q = project.as_ref().map(|p| format!("?project={p}")).unwrap_or_default();
                    self.delete(&format!("/api/abilities/{id}{q}")).await?;
                    println!("Removed {id}.");
                }
            },
            Command::Render { slug, file, asset, name, size, title } => {
                let source = std::fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
                let format = if file.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm")) { "html" } else { "svg" };
                let mut body = serde_json::json!({ "source": source, "format": format, "name": name.unwrap_or_else(|| "image".into()), "by": "user" });
                if let Some(a) = asset {
                    let (t, i) = a.split_once('/').ok_or_else(|| anyhow!("asset is type/id"))?;
                    body["asset"] = serde_json::json!({ "type": t, "id": i });
                }
                if let Some(s) = size {
                    body["size"] = serde_json::json!(s);
                }
                if let Some(t) = title {
                    body["title"] = serde_json::json!(t);
                }
                let v: serde_json::Value = self.post(&format!("/api/projects/{slug}/render"), &body).await?;
                println!("{} {}×{} via {} → {}/{} {}", v["file"].as_str().unwrap_or(""), v["width"], v["height"], v["renderer"].as_str().unwrap_or(""), v["asset"]["type"].as_str().unwrap_or(""), v["asset"]["id"].as_str().unwrap_or(""), v["url"].as_str().unwrap_or(""));
            }
            Command::Channels { slug } => {
                let v: serde_json::Value = self.get(&format!("/api/projects/{slug}/channels")).await?;
                let list = v["channels"].as_array().cloned().unwrap_or_default();
                if list.is_empty() {
                    println!("No channels yet.");
                }
                for c in list {
                    let s = |k: &str| c[k].as_str().unwrap_or("").to_string();
                    println!("{:<24} {:<10} {:<28} {:<22} {} published{}", s("id"), s("kind"), s("destination_text"), s("connection_name"), c["publications"], if c["ready"].as_bool() == Some(true) { "" } else { "  (connection not set up)" });
                }
                let kinds = v["kinds"].as_array().cloned().unwrap_or_default();
                if !kinds.is_empty() {
                    println!("\nKinds a new channel can use: {}", kinds.iter().map(|k| format!("{} via {}{}", k["kind"].as_str().unwrap_or(""), k["connection_name"].as_str().unwrap_or(""), if k["ready"].as_bool() == Some(true) { "" } else { " (needs a key)" })).collect::<Vec<_>>().join("; "));
                }
            }
            Command::Publish { slug, asset, channel, note } => {
                let (t, i) = asset.split_once('/').ok_or_else(|| anyhow!("asset is type/id"))?;
                let v: serde_json::Value = self.post(&format!("/api/projects/{slug}/assets/{t}/{i}/publish"), &serde_json::json!({ "channel": channel, "note": note, "by": "user" })).await?;
                println!("published {} → {}{}", v["asset"]["title"].as_str().unwrap_or(""), v["channel_label"].as_str().unwrap_or(""), v["url"].as_str().map(|u| format!(" {u}")).unwrap_or_else(|| v["detail"].as_str().map(|d| format!(" ({d})")).unwrap_or_default()));
            }
            Command::Email { slug, to, subject, text, asset } => {
                let mut body = serde_json::json!({ "to": to, "subject": subject, "text": text });
                if let Some(a) = asset {
                    let (t, i) = a.split_once('/').ok_or_else(|| anyhow!("asset is type/id"))?;
                    body["asset"] = serde_json::json!({ "type": t, "id": i });
                }
                let v: serde_json::Value = self.post(&format!("/api/projects/{slug}/email"), &body).await?;
                println!("sent \"{}\" to {} via {}", v["subject"].as_str().unwrap_or(""), v["to"].as_str().unwrap_or(""), v["connection"].as_str().unwrap_or(""));
            }
            Command::Secrets { cmd } => match cmd {
                SecretsCmd::Set { name, project } => {
                    eprint!("{name}: ");
                    let value = read_secret()?;
                    let path = match &project {
                        Some(p) => format!("/api/projects/{p}/secrets/{name}"),
                        None => format!("/api/secrets/{name}"),
                    };
                    let v: serde_json::Value = self.put(&path, &serde_json::json!({ "value": value })).await?;
                    println!("{} {}", v["name"].as_str().unwrap_or(&name), if v["set"].as_bool() == Some(true) { "set" } else { "cleared" });
                }
            },
            Command::Tasks { cmd } => match cmd {
                TasksCmd::Ls { slug } => {
                    let v: Vec<serde_json::Value> = self.get(&format!("/api/projects/{slug}/tasks")).await?;
                    if v.is_empty() {
                        println!("No tasks. Ask Seemo for one in a chat, e.g. \"every weekday at 10:00, write a LinkedIn post\".");
                    }
                    for t in v {
                        let s = |k: &str| t[k].as_str().unwrap_or("").to_string();
                        let next = t["next_run"].as_str().map(|x| x[..16.min(x.len())].to_string()).unwrap_or_else(|| "-".into());
                        println!("{:<28} {:<8} {:<30} next {:<17} last {:<8} {}", s("id"), s("status"), s("schedule_text"), next, t["last_status"].as_str().unwrap_or("-"), s("title"));
                    }
                }
                TasksCmd::Show { slug, task } => {
                    let v: serde_json::Value = self.get(&format!("/api/projects/{slug}/tasks/{task}")).await?;
                    println!("{}", serde_json::to_string_pretty(&v)?);
                    let runs: Vec<serde_json::Value> = self.get(&format!("/api/projects/{slug}/tasks/{task}/runs")).await?;
                    for r in runs {
                        let s = |k: &str| r[k].as_str().unwrap_or("").to_string();
                        println!("{:<18} {:<8} {}", s("id"), s("status"), s("summary").lines().next().unwrap_or(""));
                    }
                }
                TasksCmd::Run { slug, task } => {
                    let v: serde_json::Value = self.post(&format!("/api/projects/{slug}/tasks/{task}/run"), &serde_json::json!({})).await?;
                    let run = v["run"].as_str().unwrap_or("").to_string();
                    println!("Running {task} ({run})…");
                    self.stream_events(Some(&slug), false, |ev| matches!(&ev.kind, EventKind::TaskRunFinished { run: r, .. } if *r == run)).await?;
                }
                TasksCmd::Pause { slug, task } => {
                    let _: serde_json::Value = self.post(&format!("/api/projects/{slug}/tasks/{task}/pause"), &serde_json::json!({})).await?;
                    println!("Paused {task}.");
                }
                TasksCmd::Resume { slug, task } => {
                    let _: serde_json::Value = self.post(&format!("/api/projects/{slug}/tasks/{task}/resume"), &serde_json::json!({})).await?;
                    println!("Resumed {task}.");
                }
                TasksCmd::Rm { slug, task } => {
                    self.delete(&format!("/api/projects/{slug}/tasks/{task}")).await?;
                    println!("Removed {task}.");
                }
            },
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
            Command::Crawl { slug, max_pages, concurrency, ai_concurrency, watch } => {
                let mut qs = Vec::new();
                if let Some(n) = max_pages { qs.push(format!("max_pages={n}")); }
                if let Some(n) = concurrency { qs.push(format!("concurrency={n}")); }
                if let Some(n) = ai_concurrency { qs.push(format!("ai_concurrency={n}")); }
                let q = if qs.is_empty() { String::new() } else { format!("?{}", qs.join("&")) };
                let started: Result<serde_json::Value> = self.post(&format!("/api/projects/{slug}/crawl{q}"), &serde_json::json!({})).await;
                match started {
                    // One crawl per project: a new limit goes to the running one.
                    Err(e) if max_pages.is_some() && e.to_string().contains("already running") => {
                        let v: serde_json::Value = self.put(&format!("/api/projects/{slug}/crawl/limit"), &serde_json::json!({ "max_pages": max_pages })).await?;
                        if v["live"].as_bool() == Some(true) {
                            println!("A crawl is already running for {slug}. It now reads up to {} pages.", v["max_pages"]);
                        } else {
                            println!("A crawl is already running for {slug} and has finished fetching. The next crawl reads up to {} pages.", v["max_pages"]);
                        }
                        if !watch {
                            return Ok(());
                        }
                    }
                    // Otherwise follow the one that is running instead.
                    Err(e) if watch && e.to_string().contains("already running") => println!("A crawl is already running for {slug}. Following it."),
                    Err(e) => return Err(e),
                    Ok(_) => {}
                }
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
            Command::Brief { slug, json, regenerate } => {
                if regenerate {
                    let _: serde_json::Value = self.post(&format!("/api/projects/{slug}/brief"), &serde_json::json!({})).await?;
                    println!("Writing the business brief for {slug}. Watch: kiqr events --project {slug}");
                    return Ok(());
                }
                let v: serde_json::Value = self.get(&format!("/api/projects/{slug}/brief")).await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&v)?);
                } else if v.is_null() {
                    println!("No business brief yet. Crawl the site (kiqr crawl {slug}) or write one: kiqr brief {slug} --regenerate");
                } else {
                    print_brief(&v);
                }
            }
            Command::Brand { slug, json, rebuild } => {
                let path = format!("/api/projects/{slug}/brand");
                let v: serde_json::Value = if rebuild { self.post(&path, &serde_json::json!({})).await? } else { self.get(&path).await? };
                if json {
                    println!("{}", serde_json::to_string_pretty(&v)?);
                } else {
                    print_brand(&v);
                }
            }
            Command::Assets { cmd } => match cmd {
                AssetsCmd::Ls { slug, kind } => {
                    let q = kind.map(|k| format!("?type={k}")).unwrap_or_default();
                    let v: Vec<serde_json::Value> = self.get(&format!("/api/projects/{slug}/assets{q}")).await?;
                    if v.is_empty() {
                        println!("No assets yet.");
                    }
                    for a in v {
                        let s = |k: &str| a[k].as_str().unwrap_or("").to_string();
                        println!("{:<10} {:<40} {:<9} {}", s("type"), s("id"), s("status"), s("title"));
                    }
                }
                AssetsCmd::Show { slug, asset } => {
                    let v: serde_json::Value = self.get(&format!("/api/projects/{slug}/assets/{}", asset_path(&asset)?)).await?;
                    println!("{}", serde_json::to_string_pretty(&v)?);
                }
                AssetsCmd::Generate { slug, kind, count } => {
                    let _: serde_json::Value = self.post(&format!("/api/projects/{slug}/assets/generate"), &serde_json::json!({ "type": kind, "count": count })).await?;
                    println!("Seemo is writing {count} {kind}s…");
                    self.stream_events(Some(&slug), false, |ev| matches!(&ev.kind, EventKind::JobFinished { job } if job == "posts")).await?;
                }
                AssetsCmd::Rm { slug, asset } => {
                    self.delete(&format!("/api/projects/{slug}/assets/{}", asset_path(&asset)?)).await?;
                    println!("Removed {asset}.");
                }
                AssetsCmd::Approve { slug, asset, channel } => {
                    let a: serde_json::Value = self.post(&format!("/api/projects/{slug}/assets/{}/approve", asset_path(&asset)?), &serde_json::json!({ "channel": channel })).await?;
                    println!("{asset} is {} for {channel}.", a["status"].as_str().unwrap_or("ready"));
                }
                AssetsCmd::Publish { slug, asset, channel } => {
                    let r: serde_json::Value = self.post(&format!("/api/projects/{slug}/assets/{}/publish", asset_path(&asset)?), &serde_json::json!({ "channel": channel.unwrap_or_default() })).await?;
                    println!("Published to {}{}.", r["channel_label"].as_str().unwrap_or(""), r["url"].as_str().map(|u| format!(" at {u}")).unwrap_or_default());
                }
                AssetsCmd::Schedule { slug, asset, at, channel, clear } => {
                    let path = format!("/api/projects/{slug}/assets/{}/schedule", asset_path(&asset)?);
                    if clear {
                        self.delete(&path).await?;
                        println!("{asset} is no longer scheduled; it stays approved for its channel.");
                    } else {
                        let at = at.ok_or_else(|| anyhow!("give --at <RFC 3339 time> or --clear"))?;
                        let a: serde_json::Value = self.post(&path, &serde_json::json!({ "at": at, "channel": channel.unwrap_or_default() })).await?;
                        println!("{asset} is scheduled for {} to {}.", a["publish_at"].as_str().unwrap_or(&at), a["channel"].as_str().unwrap_or(""));
                    }
                }
                AssetsCmd::Unpublish { slug, asset } => {
                    let _: serde_json::Value = self.post(&format!("/api/projects/{slug}/assets/{}/unpublish", asset_path(&asset)?), &serde_json::json!({})).await?;
                    println!("{asset} no longer counts as published. The copy on the channel stays.");
                }
                AssetsCmd::Archive { slug, asset, undo } => {
                    let _: serde_json::Value = self.patch(&format!("/api/projects/{slug}/assets/{}", asset_path(&asset)?), &serde_json::json!({ "archived": !undo })).await?;
                    println!("{asset} {}.", if undo { "is back from the archive" } else { "is in the archive" });
                }
            },
            Command::Todo { slug } => {
                let v: serde_json::Value = self.get(&format!("/api/projects/{slug}/todo")).await?;
                let arr = |k: &str| v[k].as_array().cloned().unwrap_or_default();
                let approvals = arr("approvals");
                let drafts = arr("drafts");
                let attention = arr("attention");
                let scheduled = arr("scheduled");
                if approvals.is_empty() && drafts.is_empty() && attention.is_empty() && scheduled.is_empty() {
                    println!("Nothing waiting.");
                }
                for a in &approvals {
                    let p = &a["payload"];
                    let what = p["title"].as_str().or(p["name"].as_str()).or(p["label"].as_str()).or(p["subject"].as_str()).or(p["asset"]["title"].as_str()).unwrap_or("");
                    println!("{:<9} {:<40} in chat \"{}\"  ({})", a["kind"].as_str().unwrap_or(""), what, a["chat_title"].as_str().unwrap_or(""), a["id"].as_str().unwrap_or(""));
                }
                for a in &drafts {
                    println!("{:<9} {}/{:<40} {}", "draft", a["type"].as_str().unwrap_or(""), a["id"].as_str().unwrap_or(""), a["title"].as_str().unwrap_or(""));
                }
                for a in &attention {
                    println!("{:<9} {}/{:<40} {}", "failed", a["type"].as_str().unwrap_or(""), a["id"].as_str().unwrap_or(""), a["publish_error"].as_str().unwrap_or(""));
                }
                for a in &scheduled {
                    println!("{:<9} {}/{:<40} {} → {}", "scheduled", a["type"].as_str().unwrap_or(""), a["id"].as_str().unwrap_or(""), a["publish_at"].as_str().unwrap_or(""), a["channel"].as_str().unwrap_or(""));
                }
            }
            Command::Types => {
                let v: Vec<serde_json::Value> = self.get("/api/asset-types").await?;
                for t in v {
                    let s = |k: &str| t[k].as_str().unwrap_or("").to_string();
                    println!("{:<14} {:<16} {}{}", s("id"), s("plural"), s("description"), if t["builtin"].as_bool() == Some(true) { "" } else { "  (yours)" });
                }
            }
            Command::History { slug } => {
                let v: Vec<serde_json::Value> = self.get(&format!("/api/projects/{slug}/history")).await?;
                for c in v {
                    println!("{}  {}  {}", c["hash"].as_str().unwrap_or(""), &c["at"].as_str().unwrap_or("")[..16.min(c["at"].as_str().unwrap_or("").len())], c["message"].as_str().unwrap_or(""));
                }
            }
            Command::Events { project } => {
                self.stream_events(project.as_deref(), false, |_| false).await?;
            }
        }
        Ok(())
    }

    async fn watch(&self, slug: &str) -> Result<()> {
        self.stream_events(Some(slug), true, |ev| matches!(ev.kind, EventKind::CrawlFinished { .. })).await
    }

    /// With `replay`, the daemon first sends the project's current crawl from its start.
    async fn stream_events(&self, project: Option<&str>, replay: bool, stop_when: impl Fn(&Event) -> bool) -> Result<()> {
        self.stream_raw(project, replay, |ev| {
            print_event(ev);
            stop_when(ev)
        })
        .await
    }

    /// Print one chat's turn as it streams: Seemo's words, the tools it uses,
    /// and the cards; stops when the turn ends.
    async fn stream_chat(&self, slug: &str, chat: &str) -> Result<()> {
        use std::io::Write;
        let mut streamed = false;
        self.stream_raw(Some(slug), false, |ev| {
            let mine = |c: &String| c == chat;
            match &ev.kind {
                EventKind::ChatDelta { chat: c, text } if mine(c) => {
                    print!("{text}");
                    let _ = std::io::stdout().flush();
                    streamed = true;
                    false
                }
                EventKind::ChatTool { chat: c, label, status, note, .. } if mine(c) => {
                    if streamed {
                        println!();
                        streamed = false;
                    }
                    match status.as_str() {
                        "started" => eprintln!("  · {label}…"),
                        "failed" => eprintln!("  · {label}: failed{}", if note.is_empty() { String::new() } else { format!(" ({note})") }),
                        _ => {}
                    }
                    false
                }
                EventKind::ChatMessage { chat: c, message } if mine(c) => {
                    if let Some(card) = &message.card {
                        if streamed {
                            println!();
                            streamed = false;
                        }
                        match card.kind.as_str() {
                            "approval" if card.status == "pending" => eprintln!("  [Seemo is asking for a yes ({}): approve it in the web UI]", card.approval_kind.as_deref().unwrap_or("")),
                            _ => eprintln!("  [{} {}]", card.kind, card.status),
                        }
                    }
                    false
                }
                EventKind::ChatTurnFinished { chat: c, error } if mine(c) => {
                    if streamed {
                        println!();
                    }
                    if let Some(e) = error {
                        eprintln!("Seemo couldn't finish: {e}");
                    }
                    true
                }
                _ => false,
            }
        })
        .await
    }

    async fn stream_raw(&self, project: Option<&str>, replay: bool, mut on_event: impl FnMut(&Event) -> bool) -> Result<()> {
        let url = match project {
            Some(p) if replay => format!("{}/api/events?project={p}&replay=1", self.base),
            Some(p) => format!("{}/api/events?project={p}", self.base),
            None => format!("{}/api/events", self.base),
        };
        // Events run for as long as the work does.
        let resp = reqwest::Client::builder().no_proxy().build()?.get(&url).send().await.with_context(|| format!("connecting to {url}"))?.error_for_status()?;
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
                            if on_event(&ev) {
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
    async fn put<T: serde::de::DeserializeOwned>(&self, path: &str, body: &serde_json::Value) -> Result<T> {
        let resp = self.http.put(format!("{}{path}", self.base)).json(body).send().await.map_err(connect_hint)?;
        Self::json(resp).await
    }
    async fn patch<T: serde::de::DeserializeOwned>(&self, path: &str, body: &serde_json::Value) -> Result<T> {
        let resp = self.http.patch(format!("{}{path}", self.base)).json(body).send().await.map_err(connect_hint)?;
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

/// `article/whitening-guide` → the URL path segment, checked.
fn asset_path(s: &str) -> Result<String> {
    match s.split_once('/') {
        Some((t, id)) if !t.is_empty() && !id.is_empty() && !id.contains('/') => Ok(format!("{t}/{id}")),
        _ => Err(anyhow!("name the asset as <type>/<id>, like article/whitening-guide")),
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
        EventKind::CrawlStarted { max_pages, concurrency, ai_concurrency } => format!("reading up to {max_pages} pages, {concurrency} fetches and {ai_concurrency} model reads in parallel"),
        EventKind::PageDiscovered { url, .. } => format!("found    {url}"),
        EventKind::PageQueued { url, waiting } => format!("queued   {url}  {waiting} waiting for the model"),
        EventKind::PageFetching { url, via } => format!("reading  {url}{}", if via == "unblockingapi" { "  via UnblockingAPI" } else { "" }),
        EventKind::PageFetched { url, status, ms, via, title, words, .. } => format!(
            "read     {url}  {status} {ms} ms, {words} words{}{}",
            if via == "unblockingapi" { " via UnblockingAPI" } else { "" },
            if title.is_empty() { String::new() } else { format!("  \"{title}\"") }
        ),
        EventKind::PageBlocked { url, reason } => format!("blocked  {url}  {reason}"),
        EventKind::PageSkipped { url, reason } => format!("skipped  {url}  {reason}"),
        EventKind::PageParsed { url, facts, .. } => format!("parsed   {url}  {facts} facts"),
        EventKind::PageFailed { url, error } => format!("failed   {url}  {error}"),
        EventKind::CrawlFinished { pages, blocked, failed } => format!("done: {pages} pages read, {blocked} blocked, {failed} failed"),
        EventKind::SynthesisStarted { total } => format!("building the profile: {total} parts"),
        EventKind::ProfileUpdated { section, by, title, items } => {
            let name = if title.is_empty() { section } else { title };
            let mut s = format!("profile  {name} updated by {by}");
            for i in items {
                s.push_str(&format!("\n            {i}"));
            }
            s
        }
        EventKind::SynthesisFinished { sections } => format!("profile  done, {sections} parts saved"),
        EventKind::BrandUpdated { logo, colors, images } => format!(
            "brand    {}{} colours, {images} images{}",
            if logo.is_some() { "logo, " } else { "" },
            colors.len(),
            if colors.is_empty() { String::new() } else { format!("  {}", colors.join(" ")) }
        ),
        EventKind::JobFinished { job } => format!("{job} job finished"),
        EventKind::CrawlLimitChanged { max_pages } => format!("page limit now {max_pages}"),
        EventKind::AssetSaved { asset_type, id, title, status, created } => {
            format!("asset    {} {asset_type}/{id}  {title}  ({status})", if *created { "new" } else { "saved" })
        }
        EventKind::AssetDeleted { asset_type, id } => format!("asset    removed {asset_type}/{id}"),
        EventKind::AssetTypeSaved { id, name } => format!("type     {name} ({id}) saved"),
        EventKind::SeemoSays { text } => format!("Seemo: {text}"),
        EventKind::Committed { message } => format!("saved    {message}"),
        EventKind::ChatCreated { chat, title } => format!("chat     {chat} started: {title}"),
        EventKind::ChatRenamed { chat, title } => format!("chat     {chat} renamed: {title}"),
        EventKind::ChatDeleted { chat } => format!("chat     {chat} removed"),
        EventKind::ChatTurnStarted { .. } => "Seemo is thinking…".into(),
        EventKind::ChatDelta { .. } => return,
        EventKind::ChatTool { label, status, note, .. } => match status.as_str() {
            "started" => format!("         {label}…"),
            "failed" => format!("         {label}: failed{}", if note.is_empty() { String::new() } else { format!(" ({note})") }),
            _ => return,
        },
        EventKind::ChatMessage { message, .. } => match message.role {
            kiqr_core::chat::Role::Seemo => format!("Seemo: {}", message.text),
            kiqr_core::chat::Role::User => format!("You: {}", message.text),
            kiqr_core::chat::Role::Card => {
                let c = message.card.as_ref();
                format!("card     {} {}", c.map(|c| c.kind.as_str()).unwrap_or("card"), c.map(|c| c.status.as_str()).unwrap_or(""))
            }
            kiqr_core::chat::Role::Note => format!("note     {}", message.text),
        },
        EventKind::ChatTurnFinished { error, .. } => match error {
            Some(e) => format!("Seemo couldn't finish: {e}"),
            None => return,
        },
        EventKind::ApprovalRequested { approval, kind, .. } => format!("approval {kind} {approval} is waiting for you (approve it in the web UI)"),
        EventKind::ApprovalResolved { approval, decision, .. } => format!("approval {approval}: {decision}"),
        EventKind::TaskSaved { task, title, status, next_run } => format!(
            "task     {task} \"{title}\" {status}{}",
            next_run.map(|t| format!(", next run {}", t.with_timezone(&chrono::Local).format("%a %-d %b %H:%M"))).unwrap_or_default()
        ),
        EventKind::TaskDeleted { task } => format!("task     {task} removed"),
        EventKind::TaskRunStarted { task, run } => format!("run      {task}/{run} started"),
        EventKind::TaskRunStep { label, status, .. } => match status.as_str() {
            "started" => format!("         {label}…"),
            "failed" => format!("         {label}: failed"),
            _ => return,
        },
        EventKind::TaskRunFinished { task, run, status, summary } => format!("run      {task}/{run} {status}{}", if summary.is_empty() { String::new() } else { format!(": {summary}") }),
        EventKind::ConfigChanged { by, restart } => format!("config   changed by {by}{}", if *restart { " (restart the daemon to apply host/port)" } else { "" }),
        EventKind::AbilitySaved { id, scope } => format!("ability  {id} taught ({scope})"),
        EventKind::ExtensionInstalled { id, name } => format!("ability  {name} ({id}) installed"),
        EventKind::ExtensionRemoved { id } => format!("ability  {id} removed"),
        EventKind::ExtensionPaused { id, paused } => format!("ability  {id} {}", if *paused { "paused" } else { "on again" }),
        EventKind::SecretSet { name, set, project } => format!("secret   {name} {}{}", if *set { "set" } else { "cleared" }, project.as_ref().map(|p| format!(" for {p}")).unwrap_or_default()),
        EventKind::MediaCreated { ability, asset_type, id, file, provider } => format!("{ability:<8} {asset_type}/{id}/{file} via {provider}"),
        EventKind::ChannelSaved { channel, label, kind } => format!("channel  {channel} \"{label}\" ({kind})"),
        EventKind::ChannelDeleted { channel } => format!("channel  {channel} removed"),
        EventKind::AssetPublished { asset_type, id, channel, url } => format!("publish  {asset_type}/{id} → {channel}{}", url.as_ref().map(|u| format!(" {u}")).unwrap_or_default()),
        EventKind::AssetApproved { asset_type, id, channel, at, .. } => match at {
            Some(at) => format!("schedule {asset_type}/{id} → {channel} at {}", at.with_timezone(&chrono::Local).format("%a %-d %b %H:%M")),
            None => format!("approve  {asset_type}/{id} for {channel}"),
        },
        EventKind::AssetUnpublished { asset_type, id, .. } => format!("unpublish {asset_type}/{id}"),
    };
    println!("{t}  {line}");
}

fn print_brand(v: &serde_json::Value) {
    let s = |v: &serde_json::Value| v.as_str().unwrap_or("").trim().to_string();
    let strs = |v: &serde_json::Value| v.as_array().cloned().unwrap_or_default().iter().map(s).filter(|x| !x.is_empty()).collect::<Vec<_>>();
    let or_none = |v: &serde_json::Value| if s(v).is_empty() { "none found".to_string() } else { s(v) };
    println!("{}", s(&v["site_name"]));
    if !s(&v["tagline"]).is_empty() {
        println!("{}", s(&v["tagline"]));
    }
    println!();
    println!("Logo     {}", or_none(&v["logo"]));
    println!("Icon     {}", or_none(&v["icon"]));
    println!("Cover    {}", or_none(&v["cover"]));
    let colors = strs(&v["colors"]);
    println!("Colours  {}", if colors.is_empty() { "none found".into() } else { colors.join("  ") });
    let fonts = strs(&v["fonts"]);
    println!("Fonts    {}", if fonts.is_empty() { "none found".into() } else { fonts.join(", ") });
    let images = v["images"].as_array().cloned().unwrap_or_default();
    println!("Images   {}", images.len());
    for i in images.iter().take(12) {
        println!("  - {}", s(&i["url"]));
    }
    let social = v["social"].as_array().cloned().unwrap_or_default();
    if !social.is_empty() {
        println!("Social");
        for x in social {
            println!("  - {:<10} {}", s(&x["network"]), s(&x["url"]));
        }
    }
    println!("\nCollected {}", s(&v["updated_at"]));
}

fn print_brief(v: &serde_json::Value) {
    let s = |v: &serde_json::Value| v.as_str().unwrap_or("").trim().to_string();
    let arr = |v: &serde_json::Value| v.as_array().cloned().unwrap_or_default();
    let strs = |v: &serde_json::Value| arr(v).iter().map(s).filter(|x| !x.is_empty()).collect::<Vec<_>>();
    println!(
        "{}  (brief v{}, {}, {})",
        s(&v["business_name"]),
        v["version"].as_u64().unwrap_or(0),
        if s(&v["source"]) == "edited" { "edited by you" } else { "from the crawl" },
        s(&v["updated_at"])
    );
    let mut meta = Vec::new();
    if !s(&v["language"]).is_empty() {
        meta.push(format!("Language: {}", s(&v["language"])));
    }
    let geo = strs(&v["geographies"]);
    if !geo.is_empty() {
        meta.push(format!("Serves: {}", geo.join(", ")));
    }
    if !meta.is_empty() {
        println!("{}", meta.join("   "));
    }
    if !s(&v["summary"]).is_empty() {
        println!("\n{}", s(&v["summary"]));
    }
    let section = |title: &str, lines: Vec<String>| {
        if !lines.is_empty() {
            println!("\n{title}");
            for l in lines {
                println!("  - {l}");
            }
        }
    };
    section(
        "Offerings",
        arr(&v["offerings"])
            .iter()
            .map(|o| {
                let url = s(&o["url"]);
                format!("{} ({}): {}{}", s(&o["name"]), s(&o["kind"]), s(&o["description"]), if url.is_empty() { String::new() } else { format!("  {url}") })
            })
            .collect(),
    );
    section(
        "Audiences",
        arr(&v["audiences"])
            .iter()
            .map(|a| {
                let needs = strs(&a["needs"]);
                if needs.is_empty() { s(&a["name"]) } else { format!("{}: {}", s(&a["name"]), needs.join("; ")) }
            })
            .collect(),
    );
    let t = &v["tone_of_voice"];
    let mut tone = Vec::new();
    if !s(&t["description"]).is_empty() {
        tone.push(s(&t["description"]));
    }
    if !strs(&t["traits"]).is_empty() {
        tone.push(format!("Traits: {}", strs(&t["traits"]).join(", ")));
    }
    for e in strs(&t["examples"]) {
        tone.push(format!("Example: \"{e}\""));
    }
    if !strs(&t["avoid"]).is_empty() {
        tone.push(format!("Avoid: {}", strs(&t["avoid"]).join("; ")));
    }
    section("Tone of voice", tone);
    section("Differentiators", strs(&v["differentiators"]));
    if !s(&v["pricing_summary"]).is_empty() {
        section("Pricing", vec![s(&v["pricing_summary"])]);
    }
    section(
        "Competitors",
        arr(&v["competitors"]).iter().map(|c| if s(&c["note"]).is_empty() { s(&c["name"]) } else { format!("{}: {}", s(&c["name"]), s(&c["note"])) }).collect(),
    );
    if !strs(&v["brand_terms"]).is_empty() {
        section("Brand terms", vec![strs(&v["brand_terms"]).join(", ")]);
    }
    section(
        "Topic clusters",
        arr(&v["topic_clusters"])
            .iter()
            .map(|c| {
                let n = c["page_count"].as_u64().unwrap_or(0);
                format!("{} ({n} page{}): {}", s(&c["label"]), if n == 1 { "" } else { "s" }, s(&c["description"]))
            })
            .collect(),
    );
    section(
        "Content gaps",
        arr(&v["content_gaps"]).iter().map(|g| format!("{} [{}]: {}", s(&g["topic"]), s(&g["intent"]), s(&g["rationale"]))).collect(),
    );
}
