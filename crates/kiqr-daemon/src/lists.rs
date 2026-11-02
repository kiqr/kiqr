//! Lists over HTTP (`kiqr_core::lists`): the people a project emails, and
//! sending to one. A send is one message per person, in the background:
//! the account's turn is held, every address is logged in the list's send
//! log (never committed), a run that stops picks up where it left, and the
//! account's activity gets one entry for the whole send.

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use kiqr_core::accounts::Account;
use kiqr_core::channels::Channel;
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{Manifest, Provide};
use kiqr_core::lists::{self, Contact};
use kiqr_core::project::Project;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/channels/{id}/contacts", get(contacts).post(add_contacts).delete(remove_contact))
        .route("/api/projects/{slug}/channels/{id}/import", post(import))
        .route("/api/projects/{slug}/channels/{id}/unsubscribe", post(unsubscribe))
        .route("/api/projects/{slug}/channels/{id}/sends", get(sends))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

fn load(app: &App, slug: &str, id: &str) -> Result<Channel, ApiError> {
    lists::load(&app.store, slug, id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))
}

fn emit(app: &App, slug: &str, l: &Channel) {
    let v = lists::view(l, app.store.home(), slug);
    crate::channels::emit(app, slug, l, Some(v.contacts));
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct NewList {
    pub label: String,
    pub description: String,
    /// The mail account it is sent from; empty: the first one here that sends email.
    pub account: String,
}

/// A list: a channel of kind `list`, sent by a mail account.
pub async fn create_list(app: &App, slug: &str, b: NewList, by: &str) -> Result<Channel, ApiError> {
    crate::channels::create(app, slug, crate::channels::NewChannel { kind: kiqr_core::channels::LIST.into(), label: b.label, description: b.description, publisher: b.account, ..Default::default() }, by).await
}

/// The people on it. Only the UI reads this; Seemo sees counts.
async fn contacts(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let l = load(&app, &slug, &id)?;
    Ok(Json(json!({ "list": l.id, "contacts": lists::contacts(app.store.home(), &slug, &l.id) })))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AddBody {
    contacts: Vec<Contact>,
    /// How these people agreed to hear from the business, in the user's words.
    consent: String,
}

async fn add_contacts(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(b): Json<AddBody>) -> ApiResult<Value> {
    let l = load(&app, &slug, &id)?;
    let people: Vec<Contact> = b.contacts.into_iter().map(|mut c| { if c.consent.is_empty() { c.consent = b.consent.clone(); } c }).collect();
    let added = lists::add(app.store.home(), &slug, &l.id, people).map_err(|e| bad(e.to_string()))?;
    emit(&app, &slug, &l);
    Ok(Json(json!({ "added": added, "list": lists::view(&l, app.store.home(), &slug) })))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ImportBody {
    /// The CSV as text: an `email` column (or a column of addresses) and a `name` column when there is one.
    csv: String,
    consent: String,
}

async fn import(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(b): Json<ImportBody>) -> ApiResult<Value> {
    let l = load(&app, &slug, &id)?;
    if b.csv.len() > 20 * 1024 * 1024 {
        return Err(bad("that file is over 20 MB"));
    }
    let people = lists::from_csv(&b.csv, &b.consent);
    let found = people.len();
    let added = lists::add(app.store.home(), &slug, &l.id, people).map_err(|e| bad(e.to_string()))?;
    emit(&app, &slug, &l);
    Ok(Json(json!({ "found": found, "added": added, "list": lists::view(&l, app.store.home(), &slug) })))
}

#[derive(Deserialize)]
struct EmailBody {
    email: String,
}

async fn remove_contact(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(b): Json<EmailBody>) -> ApiResult<Value> {
    let l = load(&app, &slug, &id)?;
    let gone = lists::remove(app.store.home(), &slug, &l.id, &b.email).map_err(|e| bad(e.to_string()))?;
    emit(&app, &slug, &l);
    Ok(Json(json!({ "removed": gone, "list": lists::view(&l, app.store.home(), &slug) })))
}

/// Someone asked to leave (they replied, they said so): marked, never written to again.
async fn unsubscribe(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(b): Json<EmailBody>) -> ApiResult<Value> {
    let l = load(&app, &slug, &id)?;
    let done = lists::unsubscribe(app.store.home(), &slug, &l.id, &b.email).map_err(|e| bad(e.to_string()))?;
    emit(&app, &slug, &l);
    Ok(Json(json!({ "unsubscribed": done, "list": lists::view(&l, app.store.home(), &slug) })))
}

// ------------------------------------------------------------------ sends

/// One line per person in a send's log: `lists/<id>/sends/<key>.jsonl`.
#[derive(Clone, Debug, serde::Serialize, Deserialize)]
struct Sent {
    email: String,
    at: chrono::DateTime<chrono::Utc>,
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn sends_dir(app: &App, slug: &str, list: &str) -> std::path::PathBuf {
    lists::dir(app.store.home(), slug, list).join("sends")
}

/// A send's key: the asset it carries, else the subject (one send of a subject per list).
fn send_key(subject: &str, asset: Option<&crate::publish::AssetRef>) -> String {
    match asset {
        Some(a) if !a.id.is_empty() => format!("{}-{}", a.kind, a.id),
        _ => kiqr_core::assets::slugify(subject),
    }
}

fn read_log(path: &std::path::Path) -> Vec<Sent> {
    std::fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// What was sent to a list, newest first: per send, how many went and how many failed.
async fn sends(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let l = load(&app, &slug, &id)?;
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(sends_dir(&app, &slug, &l.id)) {
        for e in rd.flatten() {
            let log = read_log(&e.path());
            let key = e.path().file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let at = log.iter().map(|s| s.at).max();
            out.push(json!({ "key": key, "sent": log.iter().filter(|s| s.ok).count(), "failed": log.iter().filter(|s| !s.ok).count(), "at": at, "running": app.jobs.read().await.contains(&format!("list-send:{slug}/{}/{key}", l.id)) }));
        }
    }
    out.sort_by(|a, b| b["at"].as_str().cmp(&a["at"].as_str()));
    Ok(Json(json!({ "list": l.id, "sends": out })))
}

/// Start sending to everyone on the list through the account, one message
/// each, in the background. People already in this send's log are skipped,
/// so a stopped run picks up where it left. Returns how many are to go.
pub async fn send(app: &App, slug: &str, c: &Account, m: &Manifest, provide: &Provide, l: &Channel, subject: &str, text_md: &str, asset: Option<crate::publish::AssetRef>, by: &str) -> Result<usize, ApiError> {
    let home = app.store.home();
    let key = send_key(subject, asset.as_ref());
    let job = format!("list-send:{slug}/{}/{key}", l.id);
    if !app.jobs.write().await.insert(job.clone()) {
        return Err(bad(format!("\"{subject}\" is being sent to {} right now", l.label)));
    }
    let log_path = sends_dir(app, slug, &l.id).join(format!("{key}.jsonl"));
    let done: Vec<String> = read_log(&log_path).into_iter().filter(|s| s.ok).map(|s| s.email).collect();
    let people: Vec<Contact> = lists::recipients(home, slug, &l.id).into_iter().filter(|p| !done.contains(&p.email)).collect();
    let total = people.len();
    if total == 0 {
        app.jobs.write().await.remove(&job);
        return Err(bad(if done.is_empty() { format!("nobody is on {} yet", l.label) } else { format!("everyone on {} has had \"{subject}\" already", l.label) }));
    }
    let business = Project::load(&app.store, slug).map(|p| p.name).unwrap_or_default();
    // Every message says why it came and how to leave; the user marks who left on the list's channel.
    let footer = format!("\n\n---\n\nYou get this because you are on the {} list of {}. Reply with \"unsubscribe\" to leave it.", l.label, business);
    let text = format!("{}{footer}", text_md.trim_end());
    let html = kiqr_core::publishing::markdown_to_html(&text);
    let (app, slug, c, m, provide, list, subject, by) = (app.clone(), slug.to_string(), c.clone(), m.clone(), provide.clone(), l.clone(), subject.to_string(), by.to_string());
    tokio::spawn(async move {
        let home = app.store.home().clone();
        let _ = std::fs::create_dir_all(log_path.parent().unwrap_or(std::path::Path::new(".")));
        let (mut sent, mut failed) = (0usize, 0usize);
        let asset_v = asset.as_ref().map(|a| json!({ "type": a.kind, "id": a.id })).unwrap_or(Value::Null);
        for (i, p) in people.iter().enumerate() {
            let args = json!({ "to": p.email, "subject": subject, "text": text, "html": html, "asset": asset_v });
            let r = crate::ext::run_through(&app, &slug, &m, &provide, None, None, &args, Some(&c)).await;
            let line = match &r {
                Ok(_) => { sent += 1; Sent { email: p.email.clone(), at: chrono::Utc::now(), ok: true, error: None } }
                Err(e) => { failed += 1; Sent { email: p.email.clone(), at: chrono::Utc::now(), ok: false, error: Some(kiqr_core::profile::truncate_chars(&format!("{e:#}"), 200)) } }
            };
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
                use std::io::Write;
                let _ = writeln!(f, "{}", serde_json::to_string(&line).unwrap_or_default());
            }
            if i % 10 == 9 || i + 1 == total {
                app.bus.emit(Some(&slug), EventKind::ListSending { list: list.id.clone(), subject: subject.clone(), sent, failed, total, done: i + 1 == total });
            }
            // Mail servers rate-limit; a short pause keeps a list send from looking like a burst.
            if i + 1 < total {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            }
        }
        let a = kiqr_core::accounts::Activity { at: chrono::Utc::now(), ability: m.id.clone(), role: "publish".into(), tool: "email".into(), target: format!("list:{}:{}", list.id, send_key(&subject, asset.as_ref())), channel: list.id.clone(), url: None };
        let _ = c.record(&home, &slug, &a);
        app.bus.emit(Some(&slug), EventKind::AccountActed { account: c.id.clone(), ability: m.id.clone(), role: "publish".into(), target: a.target.clone(), channel: list.id.clone(), url: None });
        let text = if failed == 0 { format!("\"{subject}\" went to all {sent} people on {}.", list.label) } else { format!("\"{subject}\" went to {sent} people on {}; {failed} could not be sent (the list page shows who).", list.label) };
        app.bus.emit(Some(&slug), EventKind::SeemoSays { text });
        tracing::info!(slug, list = %list.id, sent, failed, by = %by, "sent to a list");
        app.jobs.write().await.remove(&job);
    });
    Ok(total)
}
