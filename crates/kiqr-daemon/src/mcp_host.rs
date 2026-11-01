//! Hosting MCP servers for connections with `transport: mcp`: anything that
//! needs code, a local framework or a browser. The daemon starts the program
//! on first use, speaks newline JSON-RPC to it (the same framing `kiqr mcp`
//! uses), keeps it alive, starts it again when it died, and kills it with the
//! daemon. One server runs per account and program
//! (`<slug>/<account>@<program>`): every ability of the service that runs on
//! that program (its own, or another's through `program:`) shares it, so one
//! browser profile is never opened twice. The model never talks to these
//! servers directly: every call goes through the daemon, which maps an
//! ability or an exposed tool onto them.

use anyhow::{anyhow, bail, Context, Result};
use kiqr_core::extensions::Manifest;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::{oneshot, Mutex, RwLock};

/// How long one tool call may take; browsers and generators are slow.
const CALL_TIMEOUT: Duration = Duration::from_secs(600);
const START_TIMEOUT: Duration = Duration::from_secs(60);

/// Which copy of a program to run and what it is given: one server per
/// account and program (`<slug>/<account>@<program>`), started with that
/// account's keys and the program's environment.
#[derive(Clone, Debug, Default)]
pub struct Launch {
    pub key: String,
    /// The account's keys by name, and the program's `env_set`, filled.
    pub vars: Vec<(String, String)>,
    /// The account's own folder: the working directory unless the manifest
    /// names one, `KIQR_ACCOUNT_DIR` in its environment and in its `args`.
    pub dir: Option<std::path::PathBuf>,
    /// The project's folder, `KIQR_PROJECT_DIR`.
    pub project_dir: Option<std::path::PathBuf>,
    /// Tools called once right after the program starts (opening the browser), with their arguments filled.
    pub warmup: Vec<(String, Value)>,
}

impl Launch {
    /// The key for an account's server of a program.
    pub fn key_for(slug: &str, connection: Option<&str>, program: &str) -> String {
        match connection {
            Some(c) => format!("{slug}/{c}@{program}"),
            None => format!("@{program}"),
        }
    }
}

/// One slot per key: the server, and a lock so two callers starting the same
/// program wait for one spawn instead of racing, without holding the whole table.
#[derive(Default)]
pub struct McpHost {
    slots: Mutex<HashMap<String, Arc<Mutex<Option<Arc<Server>>>>>>,
}

pub struct Server {
    id: String,
    stdin: Mutex<tokio::process::ChildStdin>,
    child: Mutex<tokio::process::Child>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    next_id: AtomicU64,
    alive: AtomicBool,
    /// `tools/list` as last fetched; refreshed when the server says it changed.
    tools: RwLock<Vec<Value>>,
    stale: AtomicBool,
}

impl McpHost {
    /// The running server for an account and program, started if needed. `m`
    /// is the program's manifest (the one with the command).
    pub async fn server(&self, home: &std::path::Path, m: &Manifest, launch: &Launch) -> Result<Arc<Server>> {
        let key = if launch.key.is_empty() { Launch::key_for("", None, &m.id) } else { launch.key.clone() };
        let slot = self.slots.lock().await.entry(key.clone()).or_default().clone();
        let mut slot = slot.lock().await;
        if let Some(s) = slot.as_ref() {
            if s.alive.load(Ordering::SeqCst) {
                return Ok(s.clone());
            }
            tracing::warn!(id = %key, "MCP server died; starting it again");
            *slot = None;
        }
        let s = Server::spawn(home, m, launch).await?;
        // The warmup opens what an account owns (its browser with its profile): with no account there is nothing to open, and a bare server answers tool lists.
        for (tool, args) in launch.warmup.iter().filter(|_| launch.dir.is_some()) {
            let r = s.request("tools/call", json!({ "name": tool, "arguments": args }), CALL_TIMEOUT).await.with_context(|| format!("{}: warming up with {tool}", m.name))?;
            let out = read_result(&r);
            if out.is_error {
                bail!("{}: {tool} said: {}", m.name, kiqr_core::profile::truncate_chars(out.text.trim(), 300));
            }
        }
        *slot = Some(s.clone());
        Ok(s)
    }

    async fn take(&self, keep: impl Fn(&str) -> bool) -> Vec<Arc<Server>> {
        let slots: Vec<Arc<Mutex<Option<Arc<Server>>>>> = {
            let mut all = self.slots.lock().await;
            let keys: Vec<String> = all.keys().filter(|k| !keep(k)).cloned().collect();
            keys.iter().filter_map(|k| all.remove(k)).collect()
        };
        let mut gone = Vec::new();
        for s in slots {
            if let Some(server) = s.lock().await.take() {
                gone.push(server);
            }
        }
        gone
    }

    pub async fn tools(&self, home: &std::path::Path, m: &Manifest, launch: &Launch) -> Result<Vec<Value>> {
        let s = self.server(home, m, launch).await?;
        s.tools_cached().await
    }

    /// Call one of the server's tools. Returns the raw MCP result.
    pub async fn call(&self, home: &std::path::Path, m: &Manifest, launch: &Launch, tool: &str, args: &Value) -> Result<Value> {
        let s = self.server(home, m, launch).await?;
        let r = s.request("tools/call", json!({ "name": tool, "arguments": args }), CALL_TIMEOUT).await?;
        Ok(r)
    }

    /// Stop a program's servers, every account's (after a remove or a manifest change).
    pub async fn stop(&self, program: &str) {
        for s in self.take(|k| k.rsplit_once('@').is_none_or(|(_, p)| p != program)).await {
            s.kill().await;
        }
    }

    /// Stop every program's server for one account of a project: an account is
    /// shared by the abilities of its service, and its keys just changed or it is gone.
    pub async fn stop_connection(&self, slug: &str, connection: &str) {
        let prefix = format!("{slug}/{connection}@");
        for s in self.take(|k| !k.starts_with(&prefix)).await {
            s.kill().await;
        }
    }

    pub async fn running(&self) -> Vec<String> {
        let all: Vec<(String, Arc<Mutex<Option<Arc<Server>>>>)> = self.slots.lock().await.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let mut out = Vec::new();
        for (k, s) in all {
            if s.lock().await.as_ref().is_some_and(|s| s.alive.load(Ordering::SeqCst)) {
                out.push(k);
            }
        }
        out
    }
}

impl Server {
    async fn spawn(home: &std::path::Path, m: &Manifest, launch: &Launch) -> Result<Arc<Server>> {
        let account_dir = launch.dir.as_deref();
        let mut cmd = kiqr_core::process::command(&m.command);
        // `${KIQR_ACCOUNT_DIR}` and `${KIQR_PROJECT_DIR}` in an argument are the account's and the
        // project's folders (`--user-data-dir ${KIQR_ACCOUNT_DIR}/profile`); `${NAME}` any variable set.
        let dir_text = account_dir.map(|d| d.display().to_string()).unwrap_or_default();
        let project_text = launch.project_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default();
        let lookup = |k: &str| -> Option<String> {
            match k {
                "KIQR_ACCOUNT_DIR" => Some(dir_text.clone()),
                "KIQR_PROJECT_DIR" => Some(project_text.clone()),
                _ => launch.vars.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()),
            }
        };
        cmd.args(m.args.iter().map(|a| kiqr_core::extensions::fill(a, &Value::Null, lookup)));
        cmd.env_clear();
        for k in kiqr_core::process::BASE_ENV.iter().chain(&["DISPLAY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"]) {
            if let Ok(v) = std::env::var(k) {
                cmd.env(k, v);
            }
        }
        // The account's keys by the names the manifest gives them, and the program's own settings.
        for (k, v) in &launch.vars {
            if !v.trim().is_empty() {
                cmd.env(k, v);
            }
        }
        if let Some(d) = account_dir {
            let _ = std::fs::create_dir_all(d);
            cmd.env("KIQR_ACCOUNT_DIR", d);
        }
        if let Some(d) = &launch.project_dir {
            cmd.env("KIQR_PROJECT_DIR", d);
        }
        let cwd = m.cwd.clone().map(std::path::PathBuf::from).or_else(|| account_dir.map(std::path::Path::to_path_buf)).unwrap_or_else(|| home.join("tmp").join("mcp").join(&m.id));
        let _ = std::fs::create_dir_all(&cwd);
        cmd.current_dir(&cwd);
        cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
        // Its own process group, so that stopping it reaches what it started (a browser) and not only the program.
        #[cfg(unix)]
        cmd.process_group(0);
        let mut child = cmd.spawn().with_context(|| format!("starting {} ({})", m.name, m.command))?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
        let stderr = child.stderr.take();
        let server = Server {
            id: m.id.clone(),
            stdin: Mutex::new(stdin),
            child: Mutex::new(child),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            alive: AtomicBool::new(true),
            tools: RwLock::new(vec![]),
            stale: AtomicBool::new(true),
        };
        let server = Arc::new(server);
        // Reader: answers go to whoever asked; notifications flip flags.
        {
            let s = server.clone();
            tokio::spawn(async move {
                let mut lines = tokio::io::BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                    if let Some(id) = v.get("id").and_then(|i| i.as_u64()) {
                        if let Some(tx) = s.pending.lock().await.remove(&id) {
                            let _ = tx.send(v);
                        }
                    } else if v["method"] == "notifications/tools/list_changed" {
                        s.stale.store(true, Ordering::SeqCst);
                    }
                }
                s.alive.store(false, Ordering::SeqCst);
                // Anyone still waiting gets an answer.
                for (_, tx) in s.pending.lock().await.drain() {
                    let _ = tx.send(json!({ "error": { "message": "the server exited" } }));
                }
            });
        }
        if let Some(stderr) = stderr {
            let id = m.id.clone();
            tokio::spawn(async move {
                let mut lines = tokio::io::BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!(mcp = %id, "{line}");
                }
            });
        }
        let init = server
            .request(
                "initialize",
                json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "kiqr", "version": env!("CARGO_PKG_VERSION") } }),
                START_TIMEOUT,
            )
            .await
            .with_context(|| format!("{} did not answer initialize", m.name))?;
        tracing::info!(id = %m.id, server = %init["serverInfo"]["name"].as_str().unwrap_or("?"), "MCP server started");
        server.notify("notifications/initialized", json!({})).await?;
        Ok(server)
    }

    async fn write(&self, v: &Value) -> Result<()> {
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(v.to_string().as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    async fn notify(&self, method: &str, params: Value) -> Result<()> {
        self.write(&json!({ "jsonrpc": "2.0", "method": method, "params": params })).await
    }

    /// Send a request and wait for its answer. Errors become `Err`.
    pub async fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        if !self.alive.load(Ordering::SeqCst) {
            bail!("the server {} is not running", self.id);
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        if let Err(e) = self.write(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).await {
            self.pending.lock().await.remove(&id);
            self.alive.store(false, Ordering::SeqCst);
            return Err(e);
        }
        let v = match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(v)) => v,
            Ok(Err(_)) => bail!("the server {} closed before answering", self.id),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                bail!("the server {} took longer than {} s", self.id, timeout.as_secs());
            }
        };
        if let Some(err) = v.get("error") {
            bail!("{}", err["message"].as_str().unwrap_or("the server answered with an error"));
        }
        Ok(v["result"].clone())
    }

    pub async fn tools_cached(&self) -> Result<Vec<Value>> {
        if self.stale.swap(false, Ordering::SeqCst) {
            let r = self.request("tools/list", json!({}), START_TIMEOUT).await?;
            *self.tools.write().await = r["tools"].as_array().cloned().unwrap_or_default();
        }
        Ok(self.tools.read().await.clone())
    }

    /// Stop the server: its input is closed first, so that the program ends on its own and closes
    /// what it opened (a browser, whose profile lock goes with it); only one that lingers is killed.
    /// A browser killed under the program keeps the profile locked for a while, and the next start
    /// for the same account is refused.
    async fn kill(&self) {
        use tokio::io::AsyncWriteExt;
        self.alive.store(false, Ordering::SeqCst);
        {
            let mut stdin = self.stdin.lock().await;
            let _ = stdin.shutdown().await;
        }
        let mut child = self.child.lock().await;
        #[cfg(unix)]
        if let Some(pid) = child.id() {
            // The whole group, politely: the program and its browser end on their own and the browser lets go of its profile.
            unsafe { libc::killpg(pid as i32, libc::SIGTERM) };
            if tokio::time::timeout(Duration::from_secs(6), child.wait()).await.is_ok() {
                return;
            }
            tracing::warn!(id = %self.id, "the program did not end when asked; killing it");
            unsafe { libc::killpg(pid as i32, libc::SIGKILL) };
            let _ = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
            return;
        }
        if tokio::time::timeout(Duration::from_secs(6), child.wait()).await.is_err() {
            tracing::warn!(id = %self.id, "the program did not end after its input closed; killing it");
            let _ = child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
        }
    }
}

/// What a tool result holds: text for the model, and a file if the server
/// answered with an image or audio.
pub struct McpOut {
    pub text: String,
    pub structured: Option<Value>,
    pub file: Option<(Vec<u8>, String)>,
    pub is_error: bool,
}

pub fn read_result(r: &Value) -> McpOut {
    use base64::Engine;
    let mut text = String::new();
    let mut file = None;
    for c in r["content"].as_array().cloned().unwrap_or_default() {
        match c["type"].as_str().unwrap_or("") {
            "text" => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(c["text"].as_str().unwrap_or(""));
            }
            "image" | "audio" if file.is_none() => {
                if let Some(b) = c["data"].as_str().and_then(|d| base64::engine::general_purpose::STANDARD.decode(d).ok()) {
                    file = Some((b, c["mimeType"].as_str().unwrap_or("application/octet-stream").to_string()));
                }
            }
            _ => {}
        }
    }
    McpOut { text, structured: r.get("structuredContent").cloned().filter(|v| v.is_object()), file, is_error: r["isError"].as_bool().unwrap_or(false) }
}
