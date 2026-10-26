//! Hosting MCP servers for connections with `transport: mcp`: anything that
//! needs code, a local framework or a browser. The daemon starts the program
//! on first use, speaks newline JSON-RPC to it (the same framing `kiqr mcp`
//! uses), keeps it alive, restarts it once if it dies, and kills it with the
//! daemon. The model never talks to these servers directly: every call goes
//! through the daemon, which maps an ability or an exposed tool onto them.

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
/// connection (`id#connection`), started with that connection's keys.
#[derive(Clone, Debug, Default)]
pub struct Launch {
    pub key: String,
    pub vars: Vec<(String, String)>,
}

#[derive(Default)]
pub struct McpHost {
    servers: Mutex<HashMap<String, Arc<Server>>>,
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
    /// The running server for a connection, started if needed.
    pub async fn server(&self, home: &std::path::Path, m: &Manifest, launch: &Launch) -> Result<Arc<Server>> {
        let key = if launch.key.is_empty() { m.id.clone() } else { launch.key.clone() };
        let mut all = self.servers.lock().await;
        if let Some(s) = all.get(&key) {
            if s.alive.load(Ordering::SeqCst) {
                return Ok(s.clone());
            }
            tracing::warn!(id = %key, "MCP server died; starting it again");
            all.remove(&key);
        }
        let s = Server::spawn(home, m, &launch.vars).await?;
        all.insert(key, s.clone());
        Ok(s)
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

    /// Stop an ability's servers, every connection's (after a remove or a
    /// manifest change), or one connection's with `id#connection`.
    pub async fn stop(&self, id: &str) {
        let gone: Vec<Arc<Server>> = {
            let mut all = self.servers.lock().await;
            let keys: Vec<String> = all.keys().filter(|k| *k == id || k.strip_prefix(id).is_some_and(|r| r.starts_with('#'))).cloned().collect();
            keys.iter().filter_map(|k| all.remove(k)).collect()
        };
        for s in gone {
            s.kill().await;
        }
    }

    /// Stop every ability's server for one connection (`*#connection`): an
    /// account is shared by the abilities of its service.
    pub async fn stop_connection(&self, connection: &str) {
        let gone: Vec<Arc<Server>> = {
            let mut all = self.servers.lock().await;
            let keys: Vec<String> = all.keys().filter(|k| k.rsplit_once('#').is_some_and(|(_, c)| c == connection)).cloned().collect();
            keys.iter().filter_map(|k| all.remove(k)).collect()
        };
        for s in gone {
            s.kill().await;
        }
    }

    pub async fn running(&self) -> Vec<String> {
        self.servers.lock().await.iter().filter(|(_, s)| s.alive.load(Ordering::SeqCst)).map(|(k, _)| k.clone()).collect()
    }
}

impl Server {
    async fn spawn(home: &std::path::Path, m: &Manifest, vars: &[(String, String)]) -> Result<Arc<Server>> {
        let mut cmd = tokio::process::Command::new(&m.command);
        cmd.args(&m.args);
        cmd.env_clear();
        for k in ["PATH", "HOME", "LANG", "LC_ALL", "TMPDIR", "USER", "DISPLAY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"] {
            if let Ok(v) = std::env::var(k) {
                cmd.env(k, v);
            }
        }
        // The connection's keys, by the names the manifest gives them.
        for (k, v) in vars {
            if !v.trim().is_empty() {
                cmd.env(k, v);
            }
        }
        let cwd = m.cwd.clone().map(std::path::PathBuf::from).unwrap_or_else(|| home.join("tmp").join("mcp").join(&m.id));
        let _ = std::fs::create_dir_all(&cwd);
        cmd.current_dir(&cwd);
        cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
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

    async fn kill(&self) {
        self.alive.store(false, Ordering::SeqCst);
        let _ = self.child.lock().await.start_kill();
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
