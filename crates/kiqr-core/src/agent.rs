//! Seemo as an agent: the pure parts. Building the `claude` command line and
//! system prompt, and reading the stream-json it prints. The daemon owns the
//! process, the transcript and the events; this module owns nothing and is
//! tested against recorded output.
//!
//! Every chat turn and every task run is one `claude -p` process whose only
//! tools are KIQR's, served by `kiqr mcp` (see `tools`). A chat keeps the
//! provider's session id and resumes it turn after turn.

use crate::profile::Brief;
use crate::tools::{self, Group};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::Path;

pub const BUILTIN_GUIDE: &str = include_str!("../../../skills/seemo-agent/SKILL.md");

/// Linux caps one argument at 128 KiB; the system prompt travels in argv.
pub const MAX_SYSTEM: usize = 100_000;

#[derive(Clone, Debug)]
pub enum Session {
    /// Start a session with this id.
    New(String),
    /// Continue this session.
    Resume(String),
}

#[derive(Clone, Debug)]
pub struct Spec {
    /// The bare model name for `claude --model` (`sonnet`, `opus`, a full id).
    pub model: String,
    pub session: Session,
    pub system: String,
    pub mcp: Value,
    pub max_budget_usd: f64,
}

/// `claude-cli/sonnet` → `sonnet`. Chat needs Claude Code for now; other
/// providers are a plain error the UI can show.
pub fn claude_model(spec: &str) -> Result<String> {
    match spec.split_once('/') {
        Some(("claude-cli", m)) if !m.trim().is_empty() => Ok(m.trim().to_string()),
        _ => bail!("Seemo's chat runs on Claude Code for now: set the chat model to claude-cli/<model> (it is `{spec}`)"),
    }
}

/// The arguments after `claude`. The prompt goes on stdin.
pub fn claude_args(spec: &Spec) -> Vec<String> {
    let mut a: Vec<String> = vec!["-p".into(), "--output-format".into(), "stream-json".into(), "--verbose".into(), "--include-partial-messages".into()];
    match &spec.session {
        Session::New(id) => a.extend(["--session-id".into(), id.clone()]),
        Session::Resume(id) => a.extend(["--resume".into(), id.clone()]),
    }
    a.extend(["--model".into(), spec.model.clone()]);
    a.extend(["--system-prompt".into(), spec.system.clone(), "--system-prompt-snapshot".into(), "off".into()]);
    a.extend(["--mcp-config".into(), spec.mcp.to_string(), "--strict-mcp-config".into(), "--setting-sources".into(), String::new()]);
    a.extend(["--tools".into(), String::new(), "--allowedTools".into(), format!("mcp__{}__*", tools::SERVER)]);
    a.extend(["--permission-mode".into(), "dontAsk".into(), "--permission-prompts".into(), "none".into()]);
    if spec.max_budget_usd > 0.0 {
        a.extend(["--max-budget-usd".into(), format!("{}", spec.max_budget_usd)]);
    }
    a
}

/// The `--mcp-config` value: our own binary as the server, told which project,
/// chat or run it serves and which tool groups it may offer.
pub fn mcp_config(exe: &Path, daemon_url: &str, slug: &str, chat: Option<&str>, task_run: Option<(&str, &str)>, allowed: &[Group]) -> Value {
    let mut env = json!({ "KIQR_URL": daemon_url, "KIQR_PROJECT": slug, "KIQR_ALLOWED": Group::join(allowed) });
    if let Some(c) = chat {
        env["KIQR_CHAT"] = json!(c);
    }
    if let Some((task, run)) = task_run {
        env["KIQR_TASK"] = json!(task);
        env["KIQR_RUN"] = json!(run);
    }
    json!({ "mcpServers": { tools::SERVER: { "command": exe.display().to_string(), "args": ["mcp"], "env": env } } })
}

/// What one stream-json line tells us.
#[derive(Clone, Debug, PartialEq)]
pub enum AgentEvent {
    /// The session is up. `mcp_ok` is whether our tool server connected.
    Init { session_id: String, mcp_ok: bool, mcp_status: String },
    /// A piece of text as it is written. Display only.
    Delta(String),
    /// A finished text block. The reply is these joined.
    Text(String),
    ToolStarted { id: String, name: String, input: Value },
    ToolFinished { id: String, ok: bool, output: String },
    /// The last line.
    Result { session_id: String, text: String, cost_usd: Option<f64>, is_error: bool, subtype: String },
}

pub fn parse_stream_line(line: &str) -> Vec<AgentEvent> {
    let Ok(v) = serde_json::from_str::<Value>(line.trim()) else { return vec![] };
    let s = |x: &Value| x.as_str().unwrap_or("").to_string();
    match v["type"].as_str().unwrap_or("") {
        "system" if v["subtype"] == "init" => {
            let servers = v["mcp_servers"].as_array().cloned().unwrap_or_default();
            let ours = servers.iter().find(|m| m["name"] == tools::SERVER);
            let status = ours.map(|m| s(&m["status"])).unwrap_or_else(|| "missing".into());
            vec![AgentEvent::Init { session_id: s(&v["session_id"]), mcp_ok: status == "connected", mcp_status: status }]
        }
        "stream_event" => {
            let ev = &v["event"];
            if ev["type"] == "content_block_delta" && ev["delta"]["type"] == "text_delta" {
                let t = s(&ev["delta"]["text"]);
                if !t.is_empty() {
                    return vec![AgentEvent::Delta(t)];
                }
            }
            vec![]
        }
        "assistant" => {
            let mut out = vec![];
            for block in v["message"]["content"].as_array().cloned().unwrap_or_default() {
                match block["type"].as_str() {
                    Some("text") => {
                        let t = s(&block["text"]);
                        if !t.trim().is_empty() {
                            out.push(AgentEvent::Text(t));
                        }
                    }
                    Some("tool_use") => out.push(AgentEvent::ToolStarted { id: s(&block["id"]), name: s(&block["name"]), input: block["input"].clone() }),
                    _ => {}
                }
            }
            out
        }
        "user" => {
            let mut out = vec![];
            for block in v["message"]["content"].as_array().cloned().unwrap_or_default() {
                if block["type"] == "tool_result" {
                    let output = match &block["content"] {
                        Value::String(t) => t.clone(),
                        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
                        _ => String::new(),
                    };
                    out.push(AgentEvent::ToolFinished { id: s(&block["tool_use_id"]), ok: !block["is_error"].as_bool().unwrap_or(false), output });
                }
            }
            out
        }
        "result" => vec![AgentEvent::Result {
            session_id: s(&v["session_id"]),
            text: s(&v["result"]),
            cost_usd: v["total_cost_usd"].as_f64(),
            is_error: v["is_error"].as_bool().unwrap_or(false) || v["subtype"].as_str().is_some_and(|x| x.starts_with("error")),
            subtype: s(&v["subtype"]),
        }],
        _ => vec![],
    }
}

/// A short, readable note from a tool result, for the step list.
pub fn tool_note(output: &str, ok: bool) -> String {
    let t = output.trim();
    if t.is_empty() {
        return String::new();
    }
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        if let Some(e) = v["error"].as_str() {
            return crate::profile::truncate_chars(e, 160);
        }
        if !ok {
            return crate::profile::truncate_chars(t, 160);
        }
        if let Some(title) = v["title"].as_str() {
            return crate::profile::truncate_chars(title, 120);
        }
        if let Some(n) = v.as_array().map(|a| a.len()) {
            return format!("{n} item{}", if n == 1 { "" } else { "s" });
        }
        return String::new();
    }
    if ok {
        String::new()
    } else {
        crate::profile::truncate_chars(t, 160)
    }
}

/// What the system prompt needs to know about the moment.
#[derive(Clone, Debug, Default)]
pub struct Context {
    pub project_name: String,
    pub project_url: String,
    pub slug: String,
    /// IANA name.
    pub timezone: String,
    /// `Sat 24 Oct 2026, 14:05`.
    pub now_local: String,
    pub running_jobs: Vec<String>,
    /// One line per task: title, schedule, status.
    pub tasks: Vec<String>,
    /// Set for a task run: the task's title. Chats leave it empty.
    pub task_run: Option<String>,
    pub allowed: Vec<Group>,
}

/// The brief as compact JSON: without the page lists (that is the content map).
pub fn compact_brief(brief: &Brief) -> Value {
    let mut b = serde_json::to_value(brief).unwrap_or(Value::Null);
    if let Some(clusters) = b.get_mut("topic_clusters").and_then(|v| v.as_array_mut()) {
        for c in clusters {
            if let Some(o) = c.as_object_mut() {
                o.remove("urls");
            }
        }
    }
    b
}

/// Persona, then the tool guide, then where we are, then the brief. Kept
/// under [`MAX_SYSTEM`] by shortening the brief's lists, longest first.
pub fn system_prompt(persona: &str, guide: &str, ctx: &Context, brief: Option<&Brief>) -> String {
    let mut head = String::new();
    head.push_str(persona.trim());
    head.push_str("\n\n");
    head.push_str(guide.trim());
    head.push_str("\n\n# Where you are\n\n");
    head.push_str(&format!("- Project: {} ({}), slug `{}`.\n", ctx.project_name, ctx.project_url, ctx.slug));
    head.push_str(&format!("- Local time: {} ({}). Use this timezone for schedules unless the user names another.\n", ctx.now_local, ctx.timezone));
    if ctx.running_jobs.is_empty() {
        head.push_str("- Nothing is running on the project right now.\n");
    } else {
        head.push_str(&format!("- Running now: {}.\n", ctx.running_jobs.join(", ")));
    }
    if ctx.tasks.is_empty() {
        head.push_str("- No tasks are scheduled.\n");
    } else {
        head.push_str("- Tasks:\n");
        for t in &ctx.tasks {
            head.push_str(&format!("  - {t}\n"));
        }
    }
    match &ctx.task_run {
        Some(title) => head.push_str(&format!(
            "- This is a task run (\"{title}\"), not a conversation. Nobody is reading along. Do the work with the tools you have ({}), then end with a short report of what you did and what you made (type/id of each asset). Ask no questions.\n",
            ctx.allowed.iter().map(|g| g.id()).collect::<Vec<_>>().join(", ")
        )),
        None => head.push_str("- This is a chat with the user.\n"),
    }
    let Some(brief) = brief else {
        head.push_str("\nThere is no business brief yet: the website has not been crawled and read. Say so when it matters, and offer to start a crawl.\n");
        return head;
    };
    head.push_str("\n# The business brief\n\nWhat the website says about the business, as JSON. Evidence and the full sections are a tool call away (`get_section`, `get_profile`).\n\n");
    let mut b = compact_brief(brief);
    let budget = MAX_SYSTEM.saturating_sub(head.len() + 64);
    let mut text = b.to_string();
    let mut trimmed = false;
    while text.len() > budget {
        trimmed = true;
        let Some(obj) = b.as_object_mut() else { break };
        // Shorten the longest list by a fifth, until it fits.
        let longest = obj.iter().filter_map(|(k, v)| v.as_array().filter(|a| a.len() > 1).map(|a| (k.clone(), a.len()))).max_by_key(|(_, n)| *n);
        let Some((key, n)) = longest else { break };
        if let Some(a) = obj.get_mut(&key).and_then(|v| v.as_array_mut()) {
            a.truncate((n * 4 / 5).max(1));
        }
        text = b.to_string();
    }
    if trimmed {
        head.push_str("(Some lists are shortened here; `get_brief` has the whole brief.)\n\n");
    }
    head.push_str(&text);
    head
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_name_a_new_or_resumed_session() {
        let spec = Spec { model: "sonnet".into(), session: Session::New("u1".into()), system: "S".into(), mcp: json!({}), max_budget_usd: 2.0 };
        let a = claude_args(&spec);
        assert!(a.windows(2).any(|w| w == ["--session-id", "u1"]));
        assert!(a.windows(2).any(|w| w == ["--tools", ""]));
        assert!(a.windows(2).any(|w| w == ["--allowedTools", "mcp__kiqr__*"]));
        assert!(!a.contains(&"--bare".to_string()));
        let spec = Spec { session: Session::Resume("u1".into()), ..spec };
        assert!(claude_args(&spec).windows(2).any(|w| w == ["--resume", "u1"]));
        assert!(claude_model("claude-cli/sonnet").is_ok());
        assert!(claude_model("anthropic/claude-haiku-5-5").is_err());
    }

    #[test]
    fn reads_the_stream() {
        let init = r#"{"type":"system","subtype":"init","session_id":"s1","tools":[],"mcp_servers":[{"name":"kiqr","status":"connected"}]}"#;
        assert_eq!(parse_stream_line(init), vec![AgentEvent::Init { session_id: "s1".into(), mcp_ok: true, mcp_status: "connected".into() }]);
        let delta = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi "}}}"#;
        assert_eq!(parse_stream_line(delta), vec![AgentEvent::Delta("Hi ".into())]);
        let asst = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Let me check."},{"type":"tool_use","id":"t1","name":"mcp__kiqr__get_brief","input":{}}]}}"#;
        let ev = parse_stream_line(asst);
        assert_eq!(ev[0], AgentEvent::Text("Let me check.".into()));
        assert!(matches!(&ev[1], AgentEvent::ToolStarted { id, name, .. } if id == "t1" && name == "mcp__kiqr__get_brief"));
        let res = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":[{"type":"text","text":"{\"error\":\"no project\"}"}],"is_error":true}]}}"#;
        assert_eq!(parse_stream_line(res), vec![AgentEvent::ToolFinished { id: "t1".into(), ok: false, output: "{\"error\":\"no project\"}".into() }]);
        assert_eq!(tool_note("{\"error\":\"no project\"}", false), "no project");
        let done = r#"{"type":"result","subtype":"success","is_error":false,"result":"Done.","session_id":"s1","total_cost_usd":0.0123}"#;
        assert!(matches!(parse_stream_line(done).remove(0), AgentEvent::Result { text, cost_usd: Some(c), is_error: false, .. } if text == "Done." && c > 0.01));
        assert!(parse_stream_line("not json").is_empty());
        assert!(parse_stream_line(r#"{"type":"system","subtype":"api_retry"}"#).is_empty());
    }

    #[test]
    fn system_prompt_fits_in_an_argument() {
        let mut brief = Brief::default();
        brief.summary = "x".repeat(1000);
        brief.content_gaps = (0..3000).map(|i| crate::profile::ContentGap { topic: format!("topic {i}"), rationale: "r".repeat(60), intent: "informational".into() }).collect();
        brief.differentiators = (0..500).map(|i| format!("d{i}")).collect();
        let ctx = Context { project_name: "Acme".into(), project_url: "https://acme.test".into(), slug: "acme".into(), timezone: "Europe/Stockholm".into(), now_local: "Sat 24 Oct 2026, 14:05".into(), ..Default::default() };
        let s = system_prompt("PERSONA", "GUIDE", &ctx, Some(&brief));
        assert!(s.len() <= MAX_SYSTEM, "{}", s.len());
        assert!(s.contains("Some lists are shortened"));
        assert!(s.contains("Project: Acme"));
        let small = system_prompt("PERSONA", "GUIDE", &ctx, None);
        assert!(small.contains("no business brief yet"));
    }
}
