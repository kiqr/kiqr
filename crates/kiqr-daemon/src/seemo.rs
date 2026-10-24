//! Seemo's API: chats, approvals, tasks, the calendar, and the small routes
//! the tools need (skills, one profile section or field, brief and asset
//! patches). Mounted next to `api.rs`.

use crate::agent::{self, Turn};
use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use kiqr_core::chat::{self, Card, Chat, Message, Role};
use kiqr_core::events::EventKind;
use kiqr_core::profile::{self, Brief, BriefSource, Field, Section};
use kiqr_core::project::Project;
use kiqr_core::tasks::{self, Run, Schedule, ScheduleKind, Task, TaskStatus};
use kiqr_core::tools::{self, Group};
use kiqr_core::{assets, Config};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::watch;

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/tools", get(get_tools))
        .route("/api/skills", get(list_skills))
        .route("/api/skills/{name}", get(get_skill))
        .route("/api/projects/{slug}/profile/{section}", get(get_section))
        .route("/api/projects/{slug}/profile/{section}/fields/{field}", axum::routing::put(put_field))
        .route("/api/projects/{slug}/brief", patch(patch_brief))
        .route("/api/projects/{slug}/assets/{kind}/{id}", patch(patch_asset))
        .route("/api/projects/{slug}/web", get(read_web))
        .route("/api/projects/{slug}/chats", get(list_chats).post(create_chat))
        .route("/api/projects/{slug}/chats/{id}", get(get_chat).put(rename_chat).delete(delete_chat))
        .route("/api/projects/{slug}/chats/{id}/messages", post(send_message))
        .route("/api/projects/{slug}/chats/{id}/stop", post(stop_chat))
        .route("/api/projects/{slug}/approvals", post(create_approval))
        .route("/api/projects/{slug}/approvals/{id}", get(get_approval).post(resolve_approval))
        .route("/api/projects/{slug}/tasks", get(list_tasks).post(create_task))
        .route("/api/projects/{slug}/tasks/{id}", get(get_task).patch(patch_task).delete(delete_task))
        .route("/api/projects/{slug}/tasks/{id}/run", post(run_task))
        .route("/api/projects/{slug}/tasks/{id}/pause", post(pause_task))
        .route("/api/projects/{slug}/tasks/{id}/resume", post(resume_task))
        .route("/api/projects/{slug}/tasks/{id}/runs", get(list_runs))
        .route("/api/projects/{slug}/tasks/{id}/runs/{run}", get(get_run))
        .route("/api/projects/{slug}/calendar", get(calendar))
}

fn conflict(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::CONFLICT, msg.into())
}
fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

// ------------------------------------------------------------- tools, skills

async fn get_tools() -> Json<Value> {
    Json(json!({ "groups": tools::groups_json(), "tools": tools::catalogue().iter().map(|t| json!({ "name": t.name, "group": t.group.id(), "label": t.label, "description": t.description })).collect::<Vec<_>>() }))
}

// --------------------------------------------------------------------- web

#[derive(Deserialize)]
struct ReadWeb {
    url: String,
    #[serde(default)]
    max_chars: Option<usize>,
}

/// One web page as Markdown, read the way a crawl reads it: locally first,
/// UnblockingAPI when the site blocks us and escalation is on. Seemo's
/// `read_url` tool; nothing is stored.
async fn read_web(State(app): State<App>, Path(slug): Path<String>, Query(q): Query<ReadWeb>) -> ApiResult<kiqr_core::fetch::ReadPage> {
    Project::load(&app.store, &slug)?;
    let url = url::Url::parse(q.url.trim()).map_err(|_| bad(format!("'{}' is not a URL", q.url)))?;
    let escalate = app.config.read().await.crawl.escalate_to_unblockingapi;
    let max = q.max_chars.unwrap_or(20_000).min(100_000);
    let page = kiqr_core::fetch::read_page(&url, escalate, max).await.map_err(|e| bad(e.to_string()))?;
    Ok(Json(page))
}

async fn list_skills(State(app): State<App>) -> Json<Vec<String>> {
    Json(kiqr_core::extract::skill_names(&app.store))
}

/// A skill's text, the user's copy first. Plain text.
async fn get_skill(State(app): State<App>, Path(name): Path<String>) -> Result<String, ApiError> {
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(bad("a skill name is letters, digits and dashes"));
    }
    let path = app.store.home().skills_dir().join(&name).join("SKILL.md");
    if let Ok(t) = std::fs::read_to_string(&path) {
        return Ok(t);
    }
    kiqr_core::extract::builtin_skill(&name).map(str::to_string).ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no skill '{name}'")))
}

// ---------------------------------------------------------- profile patches

async fn get_section(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Section> {
    let _ = Project::load(&app.store, &slug)?;
    if !profile::SECTIONS.iter().any(|(s, _, _)| *s == id) {
        return Err(ApiError(StatusCode::NOT_FOUND, format!("no section '{id}'; sections are {}", profile::SECTIONS.iter().map(|(s, _, _)| *s).collect::<Vec<_>>().join(", "))));
    }
    Ok(Json(Section::load(&app.store, &slug, &id)?))
}

#[derive(Deserialize)]
struct FieldValue {
    value: String,
}

/// Set one field to what the user said, and lock it. By Seemo.
async fn put_field(State(app): State<App>, Path((slug, id, field)): Path<(String, String, String)>, Json(b): Json<FieldValue>) -> ApiResult<Section> {
    let project = Project::load(&app.store, &slug)?;
    if !profile::SECTIONS.iter().any(|(s, _, _)| *s == id) {
        return Err(ApiError(StatusCode::NOT_FOUND, format!("no section '{id}'")));
    }
    let value = b.value.trim().to_string();
    if value.is_empty() {
        return Err(bad("a field needs a value"));
    }
    let mut section = Section::load(&app.store, &slug, &id)?;
    let now = Utc::now();
    let name = field.trim().to_lowercase().replace([' ', '-'], "_");
    match section.fields.iter_mut().find(|f| f.name == name) {
        Some(f) => {
            f.value = value;
            f.locked = true;
            f.edited_at = Some(now);
            f.confidence = 1.0;
        }
        None => section.fields.push(Field { name: name.clone(), value, evidence: vec![], confidence: 1.0, locked: true, edited_at: Some(now) }),
    }
    section.updated_at = Some(now);
    section.save(&app.store, &slug)?;
    profile::write_markdown(&app.store, &slug, &project.name, &project.url)?;
    app.store.commit(&format!("profile({slug}): seemo set {id}.{name}")).await?;
    app.bus.emit(Some(&slug), EventKind::ProfileUpdated { section: id, by: "seemo".into(), title: section.title.clone(), items: profile::section_items(&section) });
    Ok(Json(section))
}

#[derive(Deserialize)]
struct BriefPatch {
    changes: serde_json::Map<String, Value>,
}

/// Replace some top-level keys of the brief. By Seemo; the brief becomes `edited`.
async fn patch_brief(State(app): State<App>, Path(slug): Path<String>, Json(p): Json<BriefPatch>) -> ApiResult<Brief> {
    let project = Project::load(&app.store, &slug)?;
    let current = Brief::load(&app.store, &slug)?.ok_or_else(|| ApiError(StatusCode::PRECONDITION_FAILED, "there is no business brief yet; crawl the site first".into()))?;
    let mut v = serde_json::to_value(&current).map_err(anyhow::Error::from)?;
    let protected = ["version", "source", "updated_at"];
    for (k, val) in p.changes {
        if protected.contains(&k.as_str()) {
            continue;
        }
        if v.get(&k).is_none() {
            return Err(bad(format!("the brief has no '{k}'")));
        }
        v[&k] = val;
    }
    let mut brief: Brief = serde_json::from_value(v).map_err(|e| bad(format!("that doesn't fit the brief: {e}")))?;
    brief.normalize();
    brief.version = current.version + 1;
    brief.source = BriefSource::Edited;
    brief.updated_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    brief.save(&app.store, &slug)?;
    profile::write_markdown(&app.store, &slug, &project.name, &project.url)?;
    app.store.commit(&format!("profile({slug}): seemo edited the business brief v{}", brief.version)).await?;
    app.bus.emit(Some(&slug), EventKind::ProfileUpdated { section: "brief".into(), by: "seemo".into(), title: "Business brief".into(), items: kiqr_core::extract::brief_items(&brief) });
    Ok(Json(brief))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AssetPatch {
    title: Option<String>,
    status: Option<assets::Status>,
    tags: Option<Vec<String>>,
    summary: Option<String>,
    text: Option<String>,
}

/// Change parts of an asset; `text` rewrites its body file.
async fn patch_asset(State(app): State<App>, Path((slug, kind, id)): Path<(String, String, String)>, Json(p): Json<AssetPatch>) -> ApiResult<assets::Asset> {
    let mut a = assets::load(&app.store, &slug, &kind, &id)?;
    if let Some(t) = p.title {
        if t.trim().is_empty() {
            return Err(bad("an asset needs a title"));
        }
        a.title = t.trim().to_string();
    }
    if let Some(s) = p.status {
        a.status = s;
    }
    if let Some(t) = p.tags {
        a.tags = t;
    }
    if let Some(s) = p.summary {
        a.summary = s;
    }
    let mut saved = assets::save(&app.store, &slug, &a)?;
    if let Some(text) = p.text {
        let name = assets::body_file(&saved).map(str::to_string).unwrap_or_else(|| format!("{kind}.md"));
        saved = assets::write_file(&app.store, &slug, &kind, &id, "body", &name, (text.trim().to_string() + "\n").as_bytes())?;
    }
    let status = serde_json::to_value(saved.status).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    app.bus.emit(Some(&slug), EventKind::AssetSaved { asset_type: saved.kind.clone(), id: saved.id.clone(), title: saved.title.clone(), status, created: false });
    Ok(Json(saved))
}

// -------------------------------------------------------------------- chats

async fn list_chats(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Vec<Chat>> {
    let _ = Project::load(&app.store, &slug)?;
    Ok(Json(Chat::list(&app.store, &slug)?))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct NewChat {
    title: String,
}

async fn create_chat(State(app): State<App>, Path(slug): Path<String>, body: Option<Json<NewChat>>) -> Result<(StatusCode, Json<Chat>), ApiError> {
    let _ = Project::load(&app.store, &slug)?;
    let cfg = app.config.read().await.clone();
    let title = body.map(|b| b.0.title).unwrap_or_default();
    let chat = Chat::create(&app.store, &slug, &title, &cfg.models.chat)?;
    app.store.commit(&format!("[auto] chat({slug}): new chat")).await?;
    app.bus.emit(Some(&slug), EventKind::ChatCreated { chat: chat.id.clone(), title: chat.title.clone() });
    Ok((StatusCode::CREATED, Json(chat)))
}

#[derive(Serialize)]
struct ChatView {
    #[serde(flatten)]
    chat: Chat,
    messages: Vec<Message>,
    /// The turn in progress, if any, so a late client catches up.
    streaming: Option<agent::Streaming>,
}

async fn get_chat(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<ChatView> {
    let chat = Chat::load(&app.store, &slug, &id)?;
    let messages = chat.messages(&app.store, &slug)?;
    let streaming = app.turns.read().await.get(&id).map(|t| t.snapshot());
    Ok(Json(ChatView { chat, messages, streaming }))
}

#[derive(Deserialize)]
struct Rename {
    title: String,
}

async fn rename_chat(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(b): Json<Rename>) -> ApiResult<Chat> {
    let mut chat = Chat::load(&app.store, &slug, &id)?;
    let title = b.title.trim();
    if title.is_empty() {
        return Err(bad("a chat needs a title"));
    }
    chat.title = title.to_string();
    chat.save(&app.store, &slug)?;
    app.store.commit(&format!("[auto] chat({slug}): rename to {title}")).await?;
    app.bus.emit(Some(&slug), EventKind::ChatRenamed { chat: chat.id.clone(), title: chat.title.clone() });
    Ok(Json(chat))
}

async fn delete_chat(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> Result<StatusCode, ApiError> {
    if let Some(t) = app.turns.read().await.get(&id) {
        t.stop();
    }
    Chat::delete(&app.store, &slug, &id)?;
    app.store.commit(&format!("[auto] chat({slug}): remove {id}")).await?;
    app.bus.emit(Some(&slug), EventKind::ChatDeleted { chat: id });
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SendMessage {
    text: String,
}

/// Add the user's message and start Seemo's reply. 202; the reply streams
/// as `chat_delta`/`chat_tool` events and lands as `chat_message`.
async fn send_message(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(b): Json<SendMessage>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let _ = Project::load(&app.store, &slug)?;
    let mut chat = Chat::load(&app.store, &slug, &id)?;
    let text = b.text.trim().to_string();
    if text.is_empty() {
        return Err(bad("say something first"));
    }
    let cfg = app.config.read().await.clone();
    kiqr_core::agent::claude_model(&cfg.models.chat).map_err(|e| ApiError(StatusCode::PRECONDITION_FAILED, e.to_string()))?;
    if !kiqr_core::provider::available(&cfg.models.chat).await {
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, "Claude Code (`claude`) is not installed or not on PATH".into()));
    }
    let turn = {
        let mut turns = app.turns.write().await;
        if turns.contains_key(&id) {
            return Err(conflict("Seemo is still answering; wait or stop it first"));
        }
        let t = Arc::new(Turn::new(&id, &chat::message_id()));
        turns.insert(id.clone(), t.clone());
        t
    };
    let m = Message { id: chat::message_id(), at: Utc::now(), role: Role::User, text: text.clone(), steps: vec![], card: None, kind: None, error: None };
    let first = chat.messages == 0;
    if let Err(e) = chat.append(&app.store, &slug, &m) {
        app.turns.write().await.remove(&id);
        return Err(e.into());
    }
    if first && chat.title == "New chat" {
        chat.title = chat::title_from(&text);
        chat.save(&app.store, &slug)?;
        app.bus.emit(Some(&slug), EventKind::ChatRenamed { chat: chat.id.clone(), title: chat.title.clone() });
    }
    app.bus.emit(Some(&slug), EventKind::ChatMessage { chat: chat.id.clone(), message: m.clone() });
    tokio::spawn(agent::chat_turn(app.clone(), slug.clone(), chat, turn.clone(), text));
    Ok((StatusCode::ACCEPTED, Json(json!({ "message": m.id, "reply": turn.message_id }))))
}

async fn stop_chat(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let _ = Chat::load(&app.store, &slug, &id)?;
    match app.turns.read().await.get(&id) {
        Some(t) => {
            t.stop();
            Ok(Json(json!({ "stopping": true })))
        }
        None => Ok(Json(json!({ "stopping": false }))),
    }
}

// ---------------------------------------------------------------- approvals

/// Something Seemo wants a yes for, waiting in a chat.
pub struct Approval {
    pub id: String,
    pub slug: String,
    pub chat: Option<String>,
    pub kind: String,
    pub payload: Value,
    pub message_id: Option<String>,
    pub created: DateTime<Utc>,
    pub tx: watch::Sender<Option<Decision>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Decision {
    /// `approved`, `declined` or `expired`.
    pub status: String,
    pub result: Value,
}

#[derive(Deserialize)]
struct NewApproval {
    kind: String,
    #[serde(default)]
    chat: Option<String>,
    #[serde(default)]
    payload: Value,
}

/// What a task approval holds once checked: the fields plus the next runs.
fn task_preview(app_cfg: &Config, payload: &Value) -> Result<Value, ApiError> {
    let title = payload["title"].as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad("a task needs a title"))?;
    let instruction = payload["instruction"].as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad("a task needs an instruction"))?;
    let mut schedule: Schedule = serde_json::from_value(payload["schedule"].clone()).map_err(|e| bad(format!("the schedule is not in the expected shape: {e}")))?;
    let tz = tasks::local_tz(&app_cfg.agent.timezone);
    schedule.validate(tz).map_err(|e| bad(e.to_string()))?;
    if schedule.tz.trim().is_empty() {
        schedule.tz = tz.name().to_string();
    }
    let allowed: Vec<Group> = match payload["allowed"].as_array() {
        Some(a) if !a.is_empty() => a.iter().filter_map(|v| v.as_str()).filter_map(Group::parse).filter(|g| Group::TASK_DEFAULT.contains(g)).collect(),
        _ => Group::TASK_DEFAULT.to_vec(),
    };
    let now = Utc::now();
    let next = schedule.upcoming(now - chrono::Duration::seconds(1), now, 3, tz).map_err(|e| bad(e.to_string()))?;
    let follow_up = payload["follow_up"].as_bool().unwrap_or(schedule.kind == ScheduleKind::Once);
    Ok(json!({
        "title": title,
        "instruction": instruction,
        "schedule": schedule,
        "schedule_text": schedule.describe(tz),
        "allowed": allowed,
        "allowed_labels": allowed.iter().map(|g| g.label()).collect::<Vec<_>>(),
        "follow_up": follow_up,
        "next": next,
        "timezone": tz.name(),
    }))
}

/// Apply dotted-key changes to a copy of the config. Unknown keys and values
/// of the wrong type are errors. Returns the new config, the change list and
/// whether a restart is needed.
pub fn apply_changes(cfg: &Config, changes: &serde_json::Map<String, Value>) -> Result<(Config, Vec<Value>, bool), String> {
    let mut v = serde_json::to_value(cfg).map_err(|e| e.to_string())?;
    let mut list = Vec::new();
    for (key, new) in changes {
        if key.is_empty() || key.split('.').any(|p| p.is_empty()) {
            return Err(format!("'{key}' is not a setting"));
        }
        let pointer = format!("/{}", key.replace('.', "/"));
        let Some(slot) = v.pointer_mut(&pointer) else { return Err(format!("there is no setting '{key}'")) };
        if slot.is_object() {
            return Err(format!("'{key}' is a group of settings; name one inside it"));
        }
        let old = slot.clone();
        // Keep the type the setting has: numbers stay numbers, flags stay flags.
        let new = match (&old, new) {
            (Value::Number(_), Value::String(s)) => s.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map(Value::Number).ok_or_else(|| format!("'{key}' takes a number"))?,
            (Value::Bool(_), Value::String(s)) => match s.to_ascii_lowercase().as_str() {
                "true" | "yes" | "on" => Value::Bool(true),
                "false" | "no" | "off" => Value::Bool(false),
                _ => return Err(format!("'{key}' takes true or false")),
            },
            (Value::String(_), other) if !other.is_string() => Value::String(other.to_string()),
            (_, other) => other.clone(),
        };
        list.push(json!({ "key": key, "from": old, "to": new }));
        *slot = new;
    }
    let mut new_cfg: Config = serde_json::from_value(v).map_err(|e| format!("that is not a valid setting: {e}"))?;
    new_cfg.crawl.concurrency = new_cfg.crawl.concurrency.clamp(1, 10);
    new_cfg.crawl.ai_concurrency = new_cfg.crawl.ai_concurrency.clamp(1, 10);
    new_cfg.crawl.max_pages = new_cfg.crawl.max_pages.clamp(1, 5000);
    new_cfg.agent.max_concurrent = new_cfg.agent.max_concurrent.clamp(1, 10);
    if !new_cfg.agent.timezone.trim().is_empty() && new_cfg.agent.timezone.parse::<chrono_tz::Tz>().is_err() {
        return Err(format!("'{}' is not a timezone name (use an IANA name like Europe/Stockholm)", new_cfg.agent.timezone));
    }
    let restart = new_cfg.daemon.host != cfg.daemon.host || new_cfg.daemon.port != cfg.daemon.port;
    Ok((new_cfg, list, restart))
}

fn config_preview(cfg: &Config, payload: &Value) -> Result<Value, ApiError> {
    let changes = payload["changes"].as_object().filter(|m| !m.is_empty()).ok_or_else(|| bad("name at least one setting to change"))?;
    let (_, list, restart) = apply_changes(cfg, changes).map_err(bad)?;
    Ok(json!({ "changes": list, "reason": payload["reason"].as_str().unwrap_or(""), "restart": restart }))
}

/// From `kiqr mcp`: open an approval. The chat gets a card; the tool polls `GET`.
async fn create_approval(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<NewApproval>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let _ = Project::load(&app.store, &slug)?;
    let cfg = app.config.read().await.clone();
    let preview = match b.kind.as_str() {
        "task" => task_preview(&cfg, &b.payload)?,
        "config" => config_preview(&cfg, &b.payload)?,
        other => return Err(bad(format!("unknown approval kind '{other}'"))),
    };
    let chat_id = b.chat.filter(|c| !c.is_empty());
    let Some(chat_id) = chat_id else {
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, "this needs a person: it can only be asked from a chat, not from a task run".into()));
    };
    let mut chat = Chat::load(&app.store, &slug, &chat_id)?;
    if !app.turns.read().await.contains_key(&chat_id) {
        return Err(conflict("the chat is not in a turn; approvals can only be asked while answering"));
    }
    let id = chat::message_id();
    let card = Message {
        id: chat::message_id(),
        at: Utc::now(),
        role: Role::Card,
        text: String::new(),
        steps: vec![],
        card: Some(Card { kind: "approval".into(), approval: Some(id.clone()), approval_kind: Some(b.kind.clone()), status: "pending".into(), task: None, payload: preview.clone(), result: None }),
        kind: None,
        error: None,
    };
    chat.append(&app.store, &slug, &card)?;
    app.bus.emit(Some(&slug), EventKind::ChatMessage { chat: chat.id.clone(), message: card.clone() });
    let (tx, _) = watch::channel(None);
    let approval = Arc::new(Approval { id: id.clone(), slug: slug.clone(), chat: Some(chat_id.clone()), kind: b.kind.clone(), payload: preview.clone(), message_id: Some(card.id.clone()), created: Utc::now(), tx });
    {
        let mut all = app.approvals.write().await;
        all.retain(|_, a| Utc::now() - a.created < chrono::Duration::hours(2));
        all.insert(id.clone(), approval);
    }
    app.bus.emit(Some(&slug), EventKind::ApprovalRequested { chat: chat_id, approval: id.clone(), kind: b.kind });
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "status": "pending", "preview": preview }))))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Wait {
    /// Seconds to wait for a decision before answering `pending`.
    wait: Option<u64>,
}

async fn get_approval(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Query(q): Query<Wait>) -> ApiResult<Value> {
    let approval = app.approvals.read().await.get(&id).filter(|a| a.slug == slug).cloned().ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "no such approval (it may have expired)".into()))?;
    let mut rx = approval.tx.subscribe();
    let decided = rx.borrow().clone();
    let decided = match decided {
        Some(d) => Some(d),
        None => {
            let wait = q.wait.unwrap_or(0).min(120);
            match tokio::time::timeout(std::time::Duration::from_secs(wait), rx.changed()).await {
                Ok(Ok(())) => rx.borrow().clone(),
                _ => None,
            }
        }
    };
    Ok(Json(match decided {
        Some(d) => json!({ "id": id, "status": d.status, "result": d.result, "kind": approval.kind }),
        None => json!({ "id": id, "status": "pending", "kind": approval.kind }),
    }))
}

#[derive(Deserialize)]
struct Resolve {
    /// `approve` or `decline`.
    decision: String,
    /// For tasks: narrow the allowed groups.
    #[serde(default)]
    allowed: Option<Vec<Group>>,
}

/// From the UI: approve or decline. Applies the change, updates the card and
/// wakes the waiting tool.
async fn resolve_approval(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(b): Json<Resolve>) -> ApiResult<Value> {
    let approval = app.approvals.read().await.get(&id).filter(|a| a.slug == slug).cloned().ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "no such approval (it may have expired)".into()))?;
    if approval.tx.borrow().is_some() {
        return Err(conflict("already decided"));
    }
    let approve = match b.decision.as_str() {
        "approve" => true,
        "decline" => false,
        _ => return Err(bad("decision is approve or decline")),
    };
    let decision = if !approve {
        Decision { status: "declined".into(), result: json!({ "declined": true, "note": "The user declined. Do not retry without being asked." }) }
    } else {
        match approval.kind.as_str() {
            "task" => match approve_task(&app, &slug, &approval, b.allowed).await {
                Ok(v) => Decision { status: "approved".into(), result: v },
                Err(e) => return Err(e),
            },
            _ => match approve_config(&app, &approval).await {
                Ok(v) => Decision { status: "approved".into(), result: v },
                Err(e) => return Err(e),
            },
        }
    };
    finish_approval(&app, &approval, decision.clone()).await;
    Ok(Json(json!({ "id": id, "status": decision.status, "result": decision.result })))
}

async fn approve_task(app: &App, slug: &str, approval: &Approval, allowed: Option<Vec<Group>>) -> Result<Value, ApiError> {
    let p = &approval.payload;
    let schedule: Schedule = serde_json::from_value(p["schedule"].clone()).map_err(|e| bad(e.to_string()))?;
    let cfg = app.config.read().await.clone();
    let tz = tasks::local_tz(&cfg.agent.timezone);
    let title = p["title"].as_str().unwrap_or("Task").to_string();
    let base = assets::slugify(&title);
    let mut id = base.clone();
    let mut n = 2;
    while app.store.home().task_dir(slug, &id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    let allowed: Vec<Group> = allowed
        .filter(|a| !a.is_empty())
        .map(|a| a.into_iter().filter(|g| Group::TASK_DEFAULT.contains(g)).collect())
        .unwrap_or_else(|| serde_json::from_value(p["allowed"].clone()).unwrap_or_else(|_| Group::TASK_DEFAULT.to_vec()));
    let now = Utc::now();
    let mut task = Task {
        id,
        title,
        instruction: p["instruction"].as_str().unwrap_or("").to_string(),
        schedule,
        allowed,
        status: TaskStatus::Active,
        created_by: approval.chat.as_ref().map(|c| format!("chat:{c}")).unwrap_or_else(|| "user".into()),
        follow_up_chat: if p["follow_up"].as_bool().unwrap_or(false) { approval.chat.clone() } else { None },
        created_at: now,
        next_run: None,
        last_run: None,
        last_status: None,
        runs: 0,
    };
    task.plan_next(now - chrono::Duration::seconds(1), tz).map_err(|e| bad(e.to_string()))?;
    if task.schedule.kind == ScheduleKind::Once && task.next_run.is_none() {
        task.next_run = Some(now);
    }
    task.save(&app.store, slug)?;
    app.store.commit(&format!("[auto] task({slug}): add {}", task.title)).await?;
    agent::emit_task(app, slug, &task);
    let due_now = task.next_run.is_some_and(|t| t <= now);
    let mut running = false;
    if due_now {
        match agent::start_run(app, slug, &task.id, "approval").await {
            Ok(_) => running = true,
            Err(e) => tracing::warn!(%slug, error = %e, "could not start the task"),
        }
    }
    Ok(json!({
        "task": task.id,
        "title": task.title,
        "status": task.status.id(),
        "schedule": task.schedule.describe(tz),
        "next_run": task.next_run,
        "running": running,
        "note": if running { "The task was confirmed and is running now. Its card in the chat shows progress; you will be told when it finishes." } else { "The task was confirmed and is scheduled." },
    }))
}

async fn approve_config(app: &App, approval: &Approval) -> Result<Value, ApiError> {
    let changes: serde_json::Map<String, Value> = approval.payload["changes"]
        .as_array()
        .map(|l| l.iter().filter_map(|c| Some((c["key"].as_str()?.to_string(), c["to"].clone()))).collect())
        .unwrap_or_default();
    let current = app.config.read().await.clone();
    let (new_cfg, list, restart) = apply_changes(&current, &changes).map_err(bad)?;
    crate::api::apply_config(app, new_cfg, "seemo", restart).await?;
    Ok(json!({
        "applied": list,
        "restart": restart,
        "note": if restart { "Saved. The daemon's host or port changed, which applies after `kiqr daemon` is restarted; everything else is live now." } else { "Applied and live." },
    }))
}

async fn finish_approval(app: &App, approval: &Approval, decision: Decision) {
    let _ = approval.tx.send(Some(decision.clone()));
    if let (Some(chat_id), Some(mid)) = (&approval.chat, &approval.message_id) {
        if let Ok(mut chat) = Chat::load(&app.store, &approval.slug, chat_id) {
            if let Ok(msgs) = chat.messages(&app.store, &approval.slug) {
                if let Some(mut m) = msgs.into_iter().find(|m| &m.id == mid) {
                    if let Some(c) = m.card.as_mut() {
                        c.status = decision.status.clone();
                        c.result = Some(decision.result.clone());
                        c.task = decision.result["task"].as_str().map(str::to_string);
                    }
                    if chat.replace(&app.store, &approval.slug, &m).is_ok() {
                        app.bus.emit(Some(&approval.slug), EventKind::ChatMessage { chat: chat_id.clone(), message: m });
                    }
                }
            }
        }
        app.bus.emit(Some(&approval.slug), EventKind::ApprovalResolved { chat: chat_id.clone(), approval: approval.id.clone(), decision: decision.status });
    }
}

/// When a turn ends, anything it was still asking for is off the table.
pub async fn expire_approvals_for(app: &App, slug: &str, chat_id: &str) {
    let pending: Vec<Arc<Approval>> = app.approvals.read().await.values().filter(|a| a.slug == slug && a.chat.as_deref() == Some(chat_id) && a.tx.borrow().is_none()).cloned().collect();
    for a in pending {
        finish_approval(app, &a, Decision { status: "expired".into(), result: json!({ "expired": true }) }).await;
    }
}

// -------------------------------------------------------------------- tasks

async fn list_tasks(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Vec<Value>> {
    let _ = Project::load(&app.store, &slug)?;
    let cfg = app.config.read().await.clone();
    let tz = tasks::local_tz(&cfg.agent.timezone);
    let jobs = app.jobs.read().await.clone();
    Ok(Json(Task::list(&app.store, &slug)?.into_iter().map(|t| task_view(&t, tz, jobs.contains(&format!("task:{}", t.id)))).collect()))
}

fn task_view(t: &Task, tz: chrono_tz::Tz, running: bool) -> Value {
    let mut v = serde_json::to_value(t).unwrap_or(Value::Null);
    v["schedule_text"] = json!(t.schedule.describe(tz));
    v["allowed_labels"] = json!(t.allowed.iter().map(|g| g.label()).collect::<Vec<_>>());
    v["running"] = json!(running);
    v
}

#[derive(Deserialize)]
struct NewTask {
    title: String,
    instruction: String,
    schedule: Schedule,
    #[serde(default)]
    allowed: Vec<Group>,
}

/// A task made in the UI or CLI, without an approval (the person is the one asking).
async fn create_task(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<NewTask>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let _ = Project::load(&app.store, &slug)?;
    let cfg = app.config.read().await.clone();
    let preview = task_preview(&cfg, &json!({ "title": b.title, "instruction": b.instruction, "schedule": b.schedule, "allowed": b.allowed, "follow_up": false }))?;
    let (tx, _) = watch::channel(None);
    let a = Approval { id: String::new(), slug: slug.clone(), chat: None, kind: "task".into(), payload: preview, message_id: None, created: Utc::now(), tx };
    let v = approve_task(&app, &slug, &a, None).await?;
    Ok((StatusCode::CREATED, Json(v)))
}

async fn get_task(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let t = Task::load(&app.store, &slug, &id)?;
    let cfg = app.config.read().await.clone();
    let running = app.jobs.read().await.contains(&format!("task:{id}"));
    Ok(Json(task_view(&t, tasks::local_tz(&cfg.agent.timezone), running)))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TaskPatch {
    title: Option<String>,
    instruction: Option<String>,
    schedule: Option<Schedule>,
    allowed: Option<Vec<Group>>,
}

async fn patch_task(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(p): Json<TaskPatch>) -> ApiResult<Value> {
    let mut t = Task::load(&app.store, &slug, &id)?;
    let cfg = app.config.read().await.clone();
    let tz = tasks::local_tz(&cfg.agent.timezone);
    if let Some(x) = p.title.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        t.title = x;
    }
    if let Some(x) = p.instruction.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        t.instruction = x;
    }
    if let Some(s) = p.schedule {
        s.validate(tz).map_err(|e| bad(e.to_string()))?;
        t.schedule = s;
        if t.status == TaskStatus::Done {
            t.status = TaskStatus::Active;
        }
        t.plan_next(Utc::now(), tz).map_err(|e| bad(e.to_string()))?;
    }
    if let Some(a) = p.allowed {
        t.allowed = a.into_iter().filter(|g| Group::TASK_DEFAULT.contains(g)).collect();
    }
    t.save(&app.store, &slug)?;
    app.store.commit(&format!("task({slug}): edit {}", t.title)).await?;
    agent::emit_task(&app, &slug, &t);
    Ok(Json(task_view(&t, tz, false)))
}

async fn delete_task(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> Result<StatusCode, ApiError> {
    let t = Task::load(&app.store, &slug, &id)?;
    Task::delete(&app.store, &slug, &id)?;
    app.store.commit(&format!("task({slug}): remove {}", t.title)).await?;
    app.bus.emit(Some(&slug), EventKind::TaskDeleted { task: id });
    Ok(StatusCode::NO_CONTENT)
}

async fn run_task(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let cfg = app.config.read().await.clone();
    kiqr_core::agent::claude_model(&cfg.models.chat).map_err(|e| ApiError(StatusCode::PRECONDITION_FAILED, e.to_string()))?;
    let run = agent::start_run(&app, &slug, &id, "now").await.map_err(|e| if e.to_string().contains("already running") { conflict(e.to_string()) } else { e.into() })?;
    Ok(Json(json!({ "started": true, "run": run })))
}

async fn set_status(app: &App, slug: &str, id: &str, status: TaskStatus) -> ApiResult<Value> {
    let mut t = Task::load(&app.store, slug, id)?;
    let cfg = app.config.read().await.clone();
    let tz = tasks::local_tz(&cfg.agent.timezone);
    t.status = status;
    t.plan_next(Utc::now(), tz).map_err(|e| bad(e.to_string()))?;
    t.save(&app.store, slug)?;
    app.store.commit(&format!("task({slug}): {} {}", status.id(), t.title)).await?;
    agent::emit_task(app, slug, &t);
    Ok(Json(task_view(&t, tz, false)))
}

async fn pause_task(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    set_status(&app, &slug, &id, TaskStatus::Paused).await
}

async fn resume_task(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    set_status(&app, &slug, &id, TaskStatus::Active).await
}

async fn list_runs(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Vec<Run>> {
    let t = Task::load(&app.store, &slug, &id)?;
    Ok(Json(t.runs(&app.store, &slug)?))
}

async fn get_run(State(app): State<App>, Path((slug, id, run)): Path<(String, String, String)>) -> ApiResult<Run> {
    Ok(Json(Run::load(&app.store, &slug, &id, &run)?))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Range {
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
}

/// Upcoming runs of the project's active tasks, soonest first.
async fn calendar(State(app): State<App>, Path(slug): Path<String>, Query(r): Query<Range>) -> ApiResult<Vec<Value>> {
    let _ = Project::load(&app.store, &slug)?;
    let cfg = app.config.read().await.clone();
    let tz = tasks::local_tz(&cfg.agent.timezone);
    let from = r.from.unwrap_or_else(Utc::now);
    let to = r.to.unwrap_or(from + chrono::Duration::days(30));
    let mut items = Vec::new();
    for t in Task::list(&app.store, &slug)?.into_iter().filter(|t| t.status == TaskStatus::Active) {
        let when = t.schedule.upcoming(from - chrono::Duration::seconds(1), t.created_at, 200, tz).unwrap_or_default();
        for at in when.into_iter().filter(|a| *a <= to) {
            items.push(json!({ "task": t.id, "title": t.title, "at": at, "schedule_text": t.schedule.describe(tz) }));
        }
    }
    items.sort_by(|a, b| a["at"].as_str().cmp(&b["at"].as_str()));
    Ok(Json(items))
}
