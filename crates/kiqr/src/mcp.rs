//! `kiqr mcp`: KIQR's tools served to a model over MCP (JSON-RPC, one message
//! per line on stdin/stdout). The daemon starts one per Seemo session with the
//! project, chat or run, and the allowed tool groups in the environment. Each
//! tool is one request to the daemon's API, so this process never touches files.
//!
//! Only frames go to stdout; anything else goes to stderr.

use crate::Client;
use anyhow::Result;
use kiqr_core::tools::{self, Body, Call, Group};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

struct Scope {
    project: String,
    chat: Option<String>,
    task: Option<String>,
    allowed: Vec<Group>,
}

pub async fn serve(client: Client) -> Result<()> {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
    let scope = Scope {
        project: env("KIQR_PROJECT").unwrap_or_default(),
        chat: env("KIQR_CHAT"),
        task: env("KIQR_TASK"),
        allowed: env("KIQR_ALLOWED").map(|a| Group::parse_list(&a)).unwrap_or_else(|| Group::ALL.to_vec()),
    };
    if scope.project.is_empty() {
        eprintln!("kiqr mcp: KIQR_PROJECT is not set");
    }
    let stdin = tokio::io::stdin();
    let mut lines = tokio::io::BufReader::new(stdin).lines();
    let mut out = tokio::io::stdout();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(&line) else {
            write(&mut out, &json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "parse error" } })).await?;
            continue;
        };
        let id = req.get("id").cloned();
        let method = req["method"].as_str().unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": params["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": tools::SERVER, "version": env!("CARGO_PKG_VERSION") },
            })),
            "notifications/initialized" | "notifications/cancelled" => continue,
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools::allowed(&scope.allowed).iter().map(tools::mcp_entry).collect::<Vec<_>>() })),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or("").to_string();
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                Ok(call(&client, &scope, &name, args).await)
            }
            _ if id.is_none() => continue,
            other => Err(json!({ "code": -32601, "message": format!("method not found: {other}") })),
        };
        let Some(id) = id else { continue };
        let frame = match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
        };
        write(&mut out, &frame).await?;
    }
    Ok(())
}

async fn write(out: &mut tokio::io::Stdout, v: &Value) -> Result<()> {
    out.write_all(v.to_string().as_bytes()).await?;
    out.write_all(b"\n").await?;
    out.flush().await?;
    Ok(())
}

fn tool_error(msg: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": msg.into() }], "isError": true })
}

fn tool_ok(text: String, structured: Option<Value>) -> Value {
    let mut v = json!({ "content": [{ "type": "text", "text": text }] });
    if let Some(s) = structured {
        if s.is_object() {
            v["structuredContent"] = s;
        }
    }
    v
}

/// Largest tool result the model gets; the profile Markdown can be long.
const MAX_RESULT: usize = 120_000;

async fn call(client: &Client, scope: &Scope, name: &str, mut args: Value) -> Value {
    let Some(tool) = tools::find(name) else { return tool_error(format!("no tool named {name}")) };
    if !scope.allowed.contains(&tool.group) {
        return tool_error(format!(
            "This session may not use {}: it is allowed to {}. Do what you can without it and say so in your report.",
            tool.name,
            scope.allowed.iter().map(|g| g.label().to_lowercase()).collect::<Vec<_>>().join("; ")
        ));
    }
    if !args.is_object() {
        args = json!({});
    }
    match &tool.call {
        Call::Approval(kind) => approval(client, scope, kind, args).await,
        Call::Http { method, path, body } => {
            // Fill the path from the project and the arguments it names.
            let mut url = path.replace("{slug}", &scope.project);
            let mut used = vec![];
            for (k, v) in args.as_object().cloned().unwrap_or_default() {
                let ph = format!("{{{k}}}");
                if url.contains(&ph) {
                    let s = match &v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    url = url.replace(&ph, &urlencode(&s));
                    used.push(k);
                }
            }
            if url.contains('{') {
                let missing: Vec<&str> = url.split('{').skip(1).filter_map(|p| p.split('}').next()).collect();
                return tool_error(format!("missing argument(s): {}", missing.join(", ")));
            }
            let obj = args.as_object_mut().unwrap();
            for k in used {
                obj.remove(&k);
            }
            // Who made it: the chat or task behind this session.
            if tool.name == "create_asset" {
                let by = scope.task.as_ref().map(|t| format!("task:{t}")).or_else(|| scope.chat.as_ref().map(|c| format!("chat:{c}"))).unwrap_or_else(|| "seemo".into());
                obj.insert("by".into(), json!(by));
            }
            let (send_body, query) = match body {
                Body::None => (None, String::new()),
                Body::Args => (Some(Value::Object(obj.clone())), String::new()),
                Body::Query => {
                    let q: Vec<String> = obj.iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| format!("{}={}", urlencode(k), urlencode(&match v { Value::String(s) => s.clone(), o => o.to_string() }))).collect();
                    (None, if q.is_empty() { String::new() } else { format!("?{}", q.join("&")) })
                }
            };
            match request(client, method, &format!("{url}{query}"), send_body).await {
                Ok((status, text)) if status.is_success() => {
                    let structured = serde_json::from_str::<Value>(&text).ok();
                    let shown = if text.chars().count() > MAX_RESULT { format!("{}\n\n[cut after {MAX_RESULT} characters]", text.chars().take(MAX_RESULT).collect::<String>()) } else { text };
                    tool_ok(if shown.is_empty() { "ok".into() } else { shown }, structured)
                }
                Ok((status, text)) => {
                    let msg = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["error"].as_str().map(str::to_string)).unwrap_or(text);
                    tool_error(format!("{} ({})", msg.trim(), status.as_u16()))
                }
                Err(e) => tool_error(format!("could not reach the KIQR daemon: {e}")),
            }
        }
    }
}

async fn request(client: &Client, method: &str, path: &str, body: Option<Value>) -> Result<(reqwest::StatusCode, String)> {
    let url = format!("{}{path}", client.base);
    let m = reqwest::Method::from_bytes(method.as_bytes())?;
    let mut req = client.http.request(m, &url);
    if let Some(b) = body {
        req = req.json(&b);
    }
    let resp = req.send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    Ok((status, text))
}

/// Open an approval in the chat and wait for the person. Polls in short
/// waits so a dropped connection costs little; gives up after 15 minutes.
async fn approval(client: &Client, scope: &Scope, kind: &str, args: Value) -> Value {
    let Some(chat) = &scope.chat else {
        return tool_error("This needs a person to say yes, and this is a task run with nobody watching. Leave it out and mention it in your report.");
    };
    let open = request(client, "POST", &format!("/api/projects/{}/approvals", scope.project), Some(json!({ "kind": kind, "chat": chat, "payload": args }))).await;
    let (status, text) = match open {
        Ok(x) => x,
        Err(e) => return tool_error(format!("could not reach the KIQR daemon: {e}")),
    };
    if !status.is_success() {
        let msg = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["error"].as_str().map(str::to_string)).unwrap_or(text);
        return tool_error(format!("{} ({})", msg.trim(), status.as_u16()));
    }
    let opened: Value = serde_json::from_str(&text).unwrap_or_default();
    let id = opened["id"].as_str().unwrap_or("").to_string();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15 * 60);
    while std::time::Instant::now() < deadline {
        match request(client, "GET", &format!("/api/projects/{}/approvals/{id}?wait=60", scope.project), None).await {
            Ok((s, t)) if s.is_success() => {
                let v: Value = serde_json::from_str(&t).unwrap_or_default();
                match v["status"].as_str().unwrap_or("pending") {
                    "pending" => continue,
                    "approved" => return tool_ok(v["result"].to_string(), Some(v["result"].clone())),
                    "declined" => return tool_ok(json!({ "declined": true, "note": "The user declined. Say so in one line and do not retry unless asked." }).to_string(), None),
                    other => return tool_error(format!("the request {other} before the user answered")),
                }
            }
            Ok((s, _)) if s.as_u16() == 404 => return tool_error("the request expired before the user answered"),
            Ok(_) => tokio::time::sleep(std::time::Duration::from_secs(2)).await,
            Err(_) => tokio::time::sleep(std::time::Duration::from_secs(2)).await,
        }
    }
    tool_error("No answer within 15 minutes. Treat it as declined for now; the user can ask again.")
}

pub(crate) fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
