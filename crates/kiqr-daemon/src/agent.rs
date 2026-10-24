//! Running Seemo: a `claude -p` process per chat turn or task run, with
//! `kiqr mcp` as its only tool server. The pure parts (arguments, parsing)
//! live in `kiqr_core::agent`; this module owns the process, the transcript,
//! the run record and the events.

use crate::App;
use anyhow::{anyhow, Result};
use chrono::Utc;
use kiqr_core::agent::{self as core_agent, AgentEvent, Session, Spec};
use kiqr_core::chat::{self, Chat, Message, Role, Step};
use kiqr_core::events::EventKind;
use kiqr_core::profile::Brief;
use kiqr_core::project::Project;
use kiqr_core::tasks::{self, Run, RunStatus, Task, TaskStatus};
use kiqr_core::tools::{self, Group, ToolSpec};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

/// A chat turn in progress: what has streamed so far, for clients that join late.
pub struct Turn {
    pub chat: String,
    pub message_id: String,
    pub started: chrono::DateTime<Utc>,
    pub text: Mutex<String>,
    pub steps: Mutex<Vec<Step>>,
    pub pid: AtomicU32,
    pub stopped: AtomicBool,
}

#[derive(Serialize)]
pub struct Streaming {
    pub message: String,
    pub text: String,
    pub steps: Vec<Step>,
    pub started: chrono::DateTime<Utc>,
}

impl Turn {
    pub fn new(chat: &str, message_id: &str) -> Self {
        Self { chat: chat.into(), message_id: message_id.into(), started: Utc::now(), text: Mutex::new(String::new()), steps: Mutex::new(vec![]), pid: AtomicU32::new(0), stopped: AtomicBool::new(false) }
    }
    pub fn snapshot(&self) -> Streaming {
        Streaming {
            message: self.message_id.clone(),
            text: self.text.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            steps: self.steps.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            started: self.started,
        }
    }
    /// Ask the process to stop: SIGINT ends the turn cleanly; SIGKILL after 5 s.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        let pid = self.pid.load(Ordering::SeqCst);
        if pid == 0 {
            return;
        }
        unsafe {
            libc::kill(pid as i32, libc::SIGINT);
        }
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            // If it is gone, kill(0) fails and this is a no-op; a reused pid in 5 s is not a concern on a desktop.
            unsafe {
                if libc::kill(pid as i32, 0) == 0 {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            }
        });
    }
}

/// What one process produced.
#[derive(Default, Debug)]
pub struct Outcome {
    pub text: String,
    pub steps: Vec<Step>,
    pub session_id: Option<String>,
    pub cost_usd: Option<f64>,
    pub error: Option<String>,
    /// `type/id` of assets made or changed.
    pub assets: Vec<String>,
    /// The resume failed: the provider no longer has the session.
    pub session_lost: bool,
}

/// Where a process reports progress.
enum Sink<'a> {
    Chat { slug: &'a str, turn: &'a Turn },
    Run { slug: &'a str, task: &'a str, run: &'a str },
}

impl Sink<'_> {
    fn stopped(&self) -> bool {
        match self {
            Sink::Chat { turn, .. } => turn.stopped.load(Ordering::SeqCst),
            Sink::Run { .. } => false,
        }
    }
    fn pid(&self, pid: u32) {
        if let Sink::Chat { turn, .. } = self {
            turn.pid.store(pid, Ordering::SeqCst);
        }
    }
    fn delta(&self, app: &App, text: &str) {
        if let Sink::Chat { slug, turn } = self {
            turn.text.lock().unwrap_or_else(|e| e.into_inner()).push_str(text);
            app.bus.emit(Some(slug), EventKind::ChatDelta { chat: turn.chat.clone(), text: text.to_string() });
        }
    }
    fn step(&self, app: &App, steps: &[Step], i: usize) {
        let s = &steps[i];
        let status = match s.ok {
            None => "started",
            Some(true) => "done",
            Some(false) => "failed",
        };
        match self {
            Sink::Chat { slug, turn } => {
                *turn.steps.lock().unwrap_or_else(|e| e.into_inner()) = steps.to_vec();
                app.bus.emit(Some(slug), EventKind::ChatTool { chat: turn.chat.clone(), tool: s.tool.clone(), label: s.label.clone(), status: status.into(), note: s.note.clone(), image: s.image.clone() });
            }
            Sink::Run { slug, task, run } => {
                app.bus.emit(Some(slug), EventKind::TaskRunStep { task: task.to_string(), run: run.to_string(), label: s.label.clone(), status: status.into() });
            }
        }
    }
}

/// Run one `claude` process to completion, streaming to `sink`.
async fn drive(app: &App, spec: &Spec, specs: &[ToolSpec], prompt: &str, sink: &Sink<'_>) -> Outcome {
    let mut out = Outcome::default();
    let dir = app.store.home().agent_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        out.error = Some(format!("could not prepare the agent directory: {e}"));
        return out;
    }
    let mut child = match tokio::process::Command::new("claude")
        .args(core_agent::claude_args(spec))
        .current_dir(&dir)
        .env("MCP_TOOL_TIMEOUT", "3600000")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            out.error = Some(format!("could not start `claude` (is Claude Code installed and logged in?): {e}"));
            return out;
        }
    };
    if let Some(pid) = child.id() {
        sink.pid(pid);
    }
    if let Some(mut stdin) = child.stdin.take() {
        let p = prompt.to_string();
        tokio::spawn(async move {
            let _ = stdin.write_all(p.as_bytes()).await;
            let _ = stdin.shutdown().await;
        });
    }
    let stderr = child.stderr.take();
    let stderr_task = tokio::spawn(async move {
        let mut s = String::new();
        if let Some(mut e) = stderr {
            let _ = tokio::io::AsyncReadExt::read_to_string(&mut e, &mut s).await;
        }
        s
    });
    let stdout = child.stdout.take().expect("piped stdout");
    let mut lines = tokio::io::BufReader::new(stdout).lines();

    let mut parts: Vec<String> = vec![];
    let mut steps: Vec<Step> = vec![];
    let mut step_ids: Vec<String> = vec![];
    let mut buf = String::new();
    let mut last_flush = Instant::now();
    let mut saw_result = false;
    // A resume of a session Claude no longer has ends in a zero-turn error result.
    let mut zero_turn_error = false;
    let mut init_failed: Option<String> = None;

    while let Ok(Some(line)) = lines.next_line().await {
        for ev in core_agent::parse_stream_line(&line) {
            match ev {
                AgentEvent::Delta(t) => {
                    buf.push_str(&t);
                    if last_flush.elapsed() >= Duration::from_millis(50) {
                        sink.delta(app, &buf);
                        buf.clear();
                        last_flush = Instant::now();
                    }
                    continue;
                }
                _ => {
                    if !buf.is_empty() {
                        sink.delta(app, &buf);
                        buf.clear();
                        last_flush = Instant::now();
                    }
                }
            }
            match ev {
                AgentEvent::Init { session_id, mcp_ok, mcp_status } => {
                    if out.session_id.is_none() && !session_id.is_empty() {
                        out.session_id = Some(session_id);
                    }
                    if !mcp_ok {
                        init_failed = Some(format!("my tools did not start (kiqr mcp: {mcp_status})"));
                        let _ = child.start_kill();
                    }
                }
                AgentEvent::Text(t) => parts.push(t),
                AgentEvent::ToolStarted { id, name, input } => {
                    let short = name.strip_prefix("mcp__kiqr__").unwrap_or(&name).to_string();
                    steps.push(Step { tool: short.clone(), label: tools::label_for(specs, &short, &input), ok: None, note: String::new(), image: None });
                    step_ids.push(id);
                    sink.step(app, &steps, steps.len() - 1);
                }
                AgentEvent::ToolFinished { id, ok, output } => {
                    if let Some(i) = step_ids.iter().position(|x| *x == id) {
                        steps[i].ok = Some(ok);
                        steps[i].note = core_agent::tool_note(&output, ok);
                        if ok {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&output) {
                                // Assets made or changed, by the asset routes or by an ability that attached a file.
                                let made = match (v["type"].as_str(), v["id"].as_str()) {
                                    (Some(t), Some(id)) => Some(format!("{t}/{id}")),
                                    _ => match (v["asset"]["type"].as_str(), v["asset"]["id"].as_str()) {
                                        (Some(t), Some(id)) => Some(format!("{t}/{id}")),
                                        _ => None,
                                    },
                                };
                                if let Some(key) = made {
                                    if !out.assets.contains(&key) {
                                        out.assets.push(key);
                                    }
                                }
                                if v["mime"].as_str().is_some_and(|m| m.starts_with("image/")) {
                                    steps[i].image = v["url"].as_str().map(str::to_string);
                                }
                            }
                        }
                        sink.step(app, &steps, i);
                    }
                }
                AgentEvent::Result { session_id, text, cost_usd, is_error, subtype, num_turns } => {
                    saw_result = true;
                    zero_turn_error = is_error && num_turns == 0;
                    if !session_id.is_empty() {
                        out.session_id = Some(session_id);
                    }
                    out.cost_usd = cost_usd;
                    if parts.is_empty() && !text.trim().is_empty() {
                        parts.push(text.clone());
                    }
                    if is_error {
                        out.error = Some(match subtype.as_str() {
                            "error_max_turns" => "I ran out of turns before finishing".into(),
                            "error_max_budget_usd" => "I reached the spend limit for one turn (agent.max_budget_usd)".into(),
                            _ if !text.trim().is_empty() => text.trim().to_string(),
                            other => format!("the session ended with {other}"),
                        });
                    }
                }
                AgentEvent::Delta(_) => {}
            }
        }
    }
    if !buf.is_empty() {
        sink.delta(app, &buf);
    }
    let status = child.wait().await.ok();
    let stderr = stderr_task.await.unwrap_or_default();
    let stderr_tail: String = stderr.trim().lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" ");

    out.text = parts.join("\n\n").trim().to_string();
    out.steps = steps;
    let lost = stderr.contains("No conversation found") || (stderr.contains("session") && stderr.contains("not found"));
    if let Some(e) = init_failed {
        // No session was created; keeping the id would make every later turn resume nothing.
        out.session_id = None;
        out.error = Some(e);
    } else if sink.stopped() {
        out.error = Some("stopped".into());
    } else if saw_result && zero_turn_error && lost {
        out.session_lost = true;
    } else if !saw_result && out.error.is_none() {
        out.session_lost = lost;
        out.error = Some(if stderr_tail.is_empty() {
            format!("claude exited {} without an answer", status.map(|s| s.to_string()).unwrap_or_else(|| "early".into()))
        } else {
            stderr_tail
        });
    }
    out
}

/// Skills that drive KIQR's own pipeline, not something Seemo picks up in a chat.
pub(crate) const PIPELINE_SKILLS: &[&str] = &["extract-facts", "build-profile", "business-brief", "crawl-site", "seemo-agent"];

/// What the system prompt needs about this moment.
async fn context(app: &App, project: &Project, task_run: Option<(&str, &[Group])>) -> core_agent::Context {
    let cfg = app.config.read().await.clone();
    let tz = tasks::local_tz(&cfg.agent.timezone);
    let now = Utc::now().with_timezone(&tz);
    let suffix = format!(":{}", project.slug);
    let running_jobs: Vec<String> = app.jobs.read().await.iter().filter_map(|k| k.strip_suffix(&suffix).map(str::to_string)).collect();
    let task_lines = Task::list(&app.store, &project.slug)
        .unwrap_or_default()
        .into_iter()
        .filter(|t| t.status != TaskStatus::Done)
        .take(20)
        .map(|t| {
            let next = t.next_run.map(|n| format!(", next {}", n.with_timezone(&tz).format("%a %-d %b %H:%M"))).unwrap_or_default();
            format!("{} (`{}`): {}, {}{next}", t.title, t.id, t.schedule.describe(tz), t.status.id())
        })
        .collect();
    let skills: Vec<(String, String)> = {
        let reg = app.registry.read().await;
        reg.visible(&project.slug)
            .into_iter()
            .filter(|e| !PIPELINE_SKILLS.contains(&e.name.as_str()))
            .filter_map(|e| {
                let meta = e.skill.as_ref()?;
                let mut desc = meta.description.trim().to_string();
                if let Some(m) = &e.manifest {
                    if !crate::ext::ready(m) {
                        desc.push_str(&format!(" (its connection, {}, is not set up yet)", m.name));
                    }
                }
                Some((e.name.clone(), kiqr_core::profile::truncate_chars(&desc, 200)))
            })
            .collect()
    };
    core_agent::Context {
        skills,
        project_name: project.name.clone(),
        project_url: project.url.clone(),
        slug: project.slug.clone(),
        timezone: tz.name().to_string(),
        now_local: now.format("%a %-d %b %Y, %H:%M").to_string(),
        running_jobs,
        tasks: task_lines,
        task_run: task_run.map(|(t, _)| t.to_string()),
        allowed: task_run.map(|(_, a)| a.to_vec()).unwrap_or_else(|| Group::BUILTIN.to_vec()),
    }
}

fn persona(app: &App) -> String {
    std::fs::read_to_string(app.store.home().seemo_dir().join("PERSONA.md")).unwrap_or_else(|_| crate::PERSONA.to_string())
}

async fn spec_for(app: &App, project: &Project, session: Session, chat: Option<&str>, run: Option<(&str, &str)>, allowed: &[Group], task_run: Option<(&str, &[Group])>) -> Result<Spec> {
    let cfg = app.config.read().await.clone();
    let model = core_agent::claude_model(&cfg.models.chat)?;
    let guide = kiqr_core::extract::skill_text(&app.store, "seemo-agent", core_agent::BUILTIN_GUIDE);
    let brief = Brief::load(&app.store, &project.slug)?;
    let ctx = context(app, project, task_run).await;
    let system = core_agent::system_prompt(&persona(app), &guide, &ctx, brief.as_ref());
    let mcp = core_agent::mcp_config(&app.exe, &app.daemon_url, &project.slug, chat, run, allowed);
    Ok(Spec { model, session, system, mcp, max_budget_usd: cfg.agent.max_budget_usd })
}

// ------------------------------------------------------------------- chats

/// Run Seemo's reply to the last message of `chat`. The caller has appended
/// the user's message and registered `turn` in `app.turns`.
pub async fn chat_turn(app: App, slug: String, mut chat: Chat, turn: std::sync::Arc<Turn>, prompt: String) {
    let _permit = app.agents.clone().acquire_owned().await;
    app.bus.emit(Some(&slug), EventKind::ChatTurnStarted { chat: chat.id.clone(), message: turn.message_id.clone() });
    let result = async {
        let project = Project::load(&app.store, &slug)?;
        let session = match &chat.session_id {
            Some(s) => Session::Resume(s.clone()),
            None => Session::New(uuid::Uuid::new_v4().to_string()),
        };
        let resumed = matches!(session, Session::Resume(_));
        let groups = crate::ext::chat_groups(&app, &slug).await;
        let specs = crate::ext::tool_specs(&app, &slug, &groups).await;
        let spec = spec_for(&app, &project, session, Some(&chat.id), None, &groups, None).await?;
        let sink = Sink::Chat { slug: &slug, turn: &turn };
        let mut out = drive(&app, &spec, &specs, &prompt, &sink).await;
        if out.session_lost && resumed && !turn.stopped.load(Ordering::SeqCst) {
            // The provider forgot the session; start again from our transcript.
            let history = chat::transcript(&chat.messages(&app.store, &slug)?, 40_000);
            let prompt = if history.is_empty() { prompt.clone() } else { format!("Earlier in this conversation:\n\n{history}\n\n---\n\n{prompt}") };
            let spec = Spec { session: Session::New(uuid::Uuid::new_v4().to_string()), ..spec };
            turn.text.lock().unwrap_or_else(|e| e.into_inner()).clear();
            out = drive(&app, &spec, &specs, &prompt, &sink).await;
        }
        anyhow::Ok(out)
    }
    .await;
    let out = match result {
        Ok(o) => o,
        Err(e) => Outcome { error: Some(e.to_string()), ..Default::default() },
    };
    let text = if out.text.is_empty() {
        match &out.error {
            Some(e) if e == "stopped" => "I stopped there.".to_string(),
            Some(e) => format!("I couldn't finish that: {e}."),
            None => String::new(),
        }
    } else {
        out.text.clone()
    };
    let m = Message { id: turn.message_id.clone(), at: Utc::now(), role: Role::Seemo, text, steps: out.steps.clone(), card: None, kind: None, error: out.error.clone().filter(|e| e != "stopped") };
    if let Some(s) = &out.session_id {
        chat.session_id = Some(s.clone());
    } else if out.session_lost {
        chat.session_id = None;
    }
    if let Err(e) = chat.append(&app.store, &slug, &m) {
        tracing::error!(%slug, chat = %chat.id, error = %e, "could not save the reply");
    }
    if let Err(e) = app.store.commit(&format!("[auto] chat({slug}): {}", chat.title)).await {
        tracing::warn!(%slug, error = %e, "could not commit the chat");
    }
    app.bus.emit(Some(&slug), EventKind::ChatMessage { chat: chat.id.clone(), message: m });
    app.turns.write().await.remove(&chat.id);
    crate::seemo::expire_approvals_for(&app, &slug, &chat.id).await;
    app.bus.emit(Some(&slug), EventKind::ChatTurnFinished { chat: chat.id.clone(), error: out.error.filter(|e| e != "stopped") });
    // A task that finished while Seemo was talking: say so now.
    let next = app.queued.write().await.get_mut(&chat.id).and_then(|q| q.pop_front());
    if let Some(note) = next {
        if let Err(e) = start_followup(&app, &slug, &chat.id, note).await {
            tracing::warn!(%slug, error = %e, "could not start the follow-up");
        }
    }
}

/// Append a note to the chat and have Seemo comment on it, or queue it if a
/// turn is running. Boxed: a follow-up starts a turn, and a turn may start a
/// follow-up.
pub fn start_followup<'a>(app: &'a App, slug: &'a str, chat_id: &'a str, note: String) -> futures::future::BoxFuture<'a, Result<()>> {
    Box::pin(async move {
        if app.turns.read().await.contains_key(chat_id) {
            app.queued.write().await.entry(chat_id.to_string()).or_default().push_back(note);
            return Ok(());
        }
        let mut chat = Chat::load(&app.store, slug, chat_id)?;
        let m = Message { id: chat::message_id(), at: Utc::now(), role: Role::Note, text: note.clone(), steps: vec![], card: None, kind: Some("followup".into()), error: None };
        chat.append(&app.store, slug, &m)?;
        app.bus.emit(Some(slug), EventKind::ChatMessage { chat: chat.id.clone(), message: m });
        let turn = std::sync::Arc::new(Turn::new(chat_id, &chat::message_id()));
        app.turns.write().await.insert(chat_id.to_string(), turn.clone());
        let prompt = format!("[From KIQR, not the user] {note}\n\nTell the user in a few plain sentences what happened and what they can do next (for example review the assets in the library). If something failed, say what. Do not repeat the whole report.");
        tokio::spawn(chat_turn(app.clone(), slug.to_string(), chat, turn, prompt));
        Ok(())
    })
}

// ------------------------------------------------------------------- tasks

/// Run a task now, in the background. Returns the run id, or why not.
pub async fn start_run(app: &App, slug: &str, task_id: &str, trigger: &str) -> Result<String> {
    let mut task = Task::load(&app.store, slug, task_id)?;
    let key = format!("task:{task_id}");
    {
        let mut jobs = app.jobs.write().await;
        if jobs.contains(&key) {
            return Err(anyhow!("task '{}' is already running", task.title));
        }
        jobs.insert(key.clone());
    }
    let now = Utc::now();
    let run = Run { id: tasks::run_id(now), task: task.id.clone(), started: now, finished: None, status: RunStatus::Running, trigger: trigger.into(), summary: String::new(), steps: vec![], assets: vec![], session_id: None, cost_usd: None };
    run.save(&app.store, slug)?;
    task.last_run = Some(now);
    task.last_status = Some(RunStatus::Running);
    task.save(&app.store, slug)?;
    app.bus.emit(Some(slug), EventKind::TaskRunStarted { task: task.id.clone(), run: run.id.clone() });
    emit_task(app, slug, &task);
    let run_id = run.id.clone();
    tokio::spawn(task_run(app.clone(), slug.to_string(), task, run, key));
    Ok(run_id)
}

pub fn emit_task(app: &App, slug: &str, t: &Task) {
    app.bus.emit(Some(slug), EventKind::TaskSaved { task: t.id.clone(), title: t.title.clone(), status: t.status.id().into(), next_run: t.next_run });
}

async fn task_run(app: App, slug: String, mut task: Task, mut run: Run, key: String) {
    let _permit = app.agents.clone().acquire_owned().await;
    let allowed: Vec<Group> = if task.allowed.is_empty() { Group::TASK_DEFAULT.to_vec() } else { task.allowed.clone() };
    let result = async {
        let project = Project::load(&app.store, &slug)?;
        let spec = spec_for(&app, &project, Session::New(uuid::Uuid::new_v4().to_string()), None, Some((&task.id, &run.id)), &allowed, Some((&task.title, &allowed))).await?;
        let specs = crate::ext::tool_specs(&app, &slug, &allowed).await;
        let sink = Sink::Run { slug: &slug, task: &task.id, run: &run.id };
        let prompt = format!("Task: {}\n\n{}", task.title, task.instruction);
        anyhow::Ok(drive(&app, &spec, &specs, &prompt, &sink).await)
    }
    .await;
    let out = match result {
        Ok(o) => o,
        Err(e) => Outcome { error: Some(e.to_string()), ..Default::default() },
    };
    let now = Utc::now();
    run.finished = Some(now);
    run.status = if out.error.is_some() { RunStatus::Failed } else { RunStatus::Ok };
    run.summary = match &out.error {
        Some(e) if out.text.is_empty() => e.clone(),
        Some(e) => format!("{}\n\n({e})", out.text),
        None => out.text.clone(),
    };
    run.steps = out.steps;
    run.assets = out.assets;
    run.session_id = out.session_id;
    run.cost_usd = out.cost_usd;
    if let Err(e) = run.save(&app.store, &slug) {
        tracing::error!(%slug, error = %e, "could not save the run");
    }
    task.runs += 1;
    task.last_status = Some(run.status);
    let cfg = app.config.read().await.clone();
    let tz = tasks::local_tz(&cfg.agent.timezone);
    if task.schedule.kind == tasks::ScheduleKind::Once {
        task.status = TaskStatus::Done;
        task.next_run = None;
    } else if let Err(e) = task.plan_next(now, tz) {
        tracing::warn!(%slug, error = %e, "could not plan the next run");
    }
    if let Err(e) = task.save(&app.store, &slug) {
        tracing::error!(%slug, error = %e, "could not save the task");
    }
    if let Err(e) = app.store.commit(&format!("[auto] task({slug}): {} ran, {}", task.title, run.status.id())).await {
        tracing::warn!(%slug, error = %e, "could not commit the run");
    }
    app.jobs.write().await.remove(&key);
    let first_line = run.summary.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
    app.bus.emit(Some(&slug), EventKind::TaskRunFinished { task: task.id.clone(), run: run.id.clone(), status: run.status.id().into(), summary: kiqr_core::profile::truncate_chars(&first_line, 200) });
    emit_task(&app, &slug, &task);
    match task.follow_up_chat.clone() {
        Some(chat_id) => {
            let assets = if run.assets.is_empty() { String::new() } else { format!("\nAssets: {}.", run.assets.join(", ")) };
            let note = format!("Task \"{}\" finished: {}.{assets}\n\nReport:\n{}", task.title, run.status.id(), run.summary.trim());
            if let Err(e) = start_followup(&app, &slug, &chat_id, note).await {
                tracing::warn!(%slug, error = %e, "could not start the follow-up");
            }
        }
        None => {
            let text = match run.status {
                RunStatus::Ok => format!("Task \"{}\" ran. {}", task.title, kiqr_core::profile::truncate_chars(&first_line, 200)),
                _ => format!("Task \"{}\" failed: {}", task.title, kiqr_core::profile::truncate_chars(&first_line, 200)),
            };
            app.bus.emit(Some(&slug), EventKind::SeemoSays { text });
        }
    }
}
