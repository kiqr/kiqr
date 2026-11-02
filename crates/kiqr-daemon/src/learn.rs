//! Learning from sources. An owned channel with learning on, published to by
//! an account whose ability declares `learn:` (a GitHub repository: its
//! README, docs and releases), is a source: its documents are read through
//! the ability's own tools, each extracted into facts like a page of the
//! site, and the profile is written again from everything. The facts cite the
//! document's link, so the profile shows where each came from.
//!
//! A source is read when learning is switched on, with the crawl, on
//! "Learn now", and again every week (`due`, from the scheduler).

use crate::api::ApiError;
use crate::App;
use axum::http::StatusCode;
use kiqr_core::accounts::Account;
use kiqr_core::channels::Channel;
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{self, LearnDoc, Manifest};
use kiqr_core::extract::Extractor;
use kiqr_core::project::Project;
use kiqr_core::provider;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Mutex;

/// How often a source is read again.
const EVERY: chrono::Duration = chrono::Duration::days(7);

/// Sources tried since the daemon started without ever being read (not
/// signed in yet): the scheduler tries each once more after a restart, not
/// every tick.
static TRIED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// One account's ability, when it can teach the project.
async fn learnable(app: &App, slug: &str, c: &Account) -> Option<Manifest> {
    crate::accounts::manifests_of(app, slug, &c.service).await.into_iter().find(|m| m.learn.is_some())
}

/// Whether accounts of a service can teach the project.
pub async fn learnable_service(app: &App, slug: &str, service: &str) -> bool {
    !service.is_empty() && crate::accounts::manifests_of(app, slug, service).await.iter().any(|m| m.learn.is_some())
}

/// The channels learned from, with the account each is read through.
fn sources(app: &App, slug: &str) -> Vec<(Channel, Account)> {
    Channel::all(&app.store, slug).unwrap_or_default().into_iter().filter(|ch| ch.learn && ch.owned() && !ch.suggested).filter_map(|ch| Account::load(&app.store, slug, &ch.publisher).ok().map(|c| (ch, c))).collect()
}

/// A tool's answer as text: what it said, a text file, or the JSON itself.
fn doc_text(out: &crate::ext::ExecOut) -> String {
    if let Some(t) = out.text.as_ref().filter(|t| !t.trim().is_empty()) {
        return t.clone();
    }
    if let Some(t) = out.file.as_ref().and_then(|(b, _)| std::str::from_utf8(b).ok()) {
        return t.to_string();
    }
    if let Some(t) = out.json.get("text").and_then(Value::as_str) {
        return t.to_string();
    }
    serde_json::to_string_pretty(&out.json).unwrap_or_default()
}

async fn call(app: &App, slug: &str, m: &Manifest, c: &Account, tool: &str, args: &Value) -> anyhow::Result<crate::ext::ExecOut> {
    let t = m.tools.iter().find(|t| t.name == tool).ok_or_else(|| anyhow::anyhow!("{} has no tool '{tool}'", m.name))?;
    crate::ext::execute(app, slug, m, &t.provide, None, Some(tool), args, Some(&c.id)).await
}

/// One document into facts: 1 when it gave any.
async fn extract(ex: &Extractor, slug: &str, title: String, url: String, text: String) -> usize {
    let Ok(u) = url::Url::parse(&url) else {
        tracing::warn!(%slug, %url, "a learned document's link is not a URL");
        return 0;
    };
    match ex.extract_page(slug, &u, &title, &text).await {
        Ok(f) if !f.is_empty() => 1,
        Ok(_) => 0,
        Err(e) => {
            tracing::warn!(%slug, %url, error = %e, "could not extract a learned document");
            0
        }
    }
}

/// Read one source's documents into facts. Returns how many documents gave
/// facts; a document that is not there (no `docs/` folder) is skipped.
pub async fn read_source(app: &App, slug: &str, ch: &Channel, c: &Account, ex: &Extractor) -> anyhow::Result<usize> {
    let m = learnable(app, slug, c).await.ok_or_else(|| anyhow::anyhow!("{} has nothing to learn from", c.label))?;
    let st = crate::accounts::state(app, slug, c).await;
    if !st.ready() {
        anyhow::bail!("{}", crate::accounts::state_text(c, &st));
    }
    let spec = m.learn.clone().unwrap_or_default();
    // Titles and links may name the account's keys (`${GITHUB_REPO}`), never a secret: they are kept in the facts.
    let env = crate::ext::target_env(app, slug, &m, c);
    let mut read = 0usize;
    for d in &spec.documents {
        let LearnDoc { tool, args, title, url, each } = d;
        let args = if args.is_null() { json!({}) } else { extensions::fill_value(args, &json!({}), &env) };
        let out = match call(app, slug, &m, c, tool, &args).await {
            Ok(o) => o,
            Err(e) => {
                tracing::info!(%slug, account = %c.label, %tool, error = %e, "a document to learn from is not there");
                continue;
            }
        };
        match each {
            None => read += extract(ex, slug, extensions::fill(title, &json!({}), &env), extensions::fill(url, &json!({}), &env), doc_text(&out)).await,
            Some(e) => {
                let list = if e.list.is_empty() { Some(&out.json) } else { extensions::pointer(&out.json, &e.list) };
                let items = list.and_then(Value::as_array).cloned().unwrap_or_default();
                let values: Vec<String> = items
                    .iter()
                    .filter(|it| e.where_.iter().all(|(k, v)| it.get(k).and_then(Value::as_str) == Some(v.as_str())))
                    .filter_map(|it| extensions::pointer(it, &e.value).and_then(Value::as_str).map(str::to_string))
                    .filter(|v| e.only.is_empty() || e.only.iter().any(|o| v.to_lowercase().ends_with(&o.to_lowercase())))
                    .take(e.limit())
                    .collect();
                for v in values {
                    let at = json!({ "value": v });
                    match call(app, slug, &m, c, &e.tool, &json!({ e.arg.clone(): v })).await {
                        Ok(o) => read += extract(ex, slug, extensions::fill(title, &at, &env), extensions::fill(url, &at, &env), doc_text(&o)).await,
                        Err(err) => tracing::info!(%slug, account = %c.label, value = %v, error = %err, "could not read a document to learn from"),
                    }
                }
            }
        }
    }
    let mut ch = Channel::load(&app.store, slug, &ch.id)?;
    ch.last_learned = Some(chrono::Utc::now());
    ch.learned = read as u32;
    ch.save(&app.store, slug)?;
    app.store.commit(&format!("[auto] source({slug}): learned from {} ({read} documents)", ch.label)).await?;
    Ok(read)
}

/// Every source of the project (or the one named), for the crawl, which
/// writes the profile itself afterwards. Says nothing; returns what was read.
pub async fn read_all(app: &App, slug: &str, only: Option<&str>, ex: &Extractor) -> Vec<(String, anyhow::Result<usize>)> {
    let mut out = Vec::new();
    for (ch, c) in sources(app, slug).into_iter().filter(|(ch, _)| only.is_none_or(|o| o == ch.id)) {
        if learnable(app, slug, &c).await.is_none() {
            continue;
        }
        let r = read_source(app, slug, &ch, &c, ex).await;
        out.push((ch.label.clone(), r));
    }
    out
}

/// Learn from the project's sources (or one) in the background, then write
/// the profile again. One at a time per project, never during a crawl.
pub async fn start(app: &App, slug: &str, only: Option<String>) -> Result<(), ApiError> {
    let project = Project::load(&app.store, slug)?;
    let cfg = app.config.read().await.clone();
    if !provider::available(&cfg.models.extract).await {
        return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("model `{}` is not available", cfg.models.extract)));
    }
    let key = crate::api::claim(app, "learn", slug).await?;
    let ex = app.extractor(&cfg);
    let app = app.clone();
    tokio::spawn(async move {
        let slug = project.slug.clone();
        let done = read_all(&app, &slug, only.as_deref(), &ex).await;
        let read: usize = done.iter().filter_map(|(_, r)| r.as_ref().ok()).sum();
        let failed: Vec<String> = done.iter().filter_map(|(l, r)| r.as_ref().err().map(|e| format!("{l}: {e}"))).collect();
        if !failed.is_empty() {
            app.bus.emit(Some(&slug), EventKind::SeemoSays { text: format!("I couldn't learn from {}.", failed.join("; ")) });
        }
        if read > 0 {
            if provider::available(&cfg.models.synthesize).await {
                // synthesize() tells the user itself when it fails.
                if ex.synthesize(&project, true).await.is_ok() {
                    let names: Vec<&str> = done.iter().filter(|(_, r)| r.as_ref().is_ok_and(|n| *n > 0)).map(|(l, _)| l.as_str()).collect();
                    app.bus.emit(Some(&slug), EventKind::SeemoSays { text: format!("I read {read} documents from {} and updated the profile and the brief with what they say. Lock anything I got wrong.", names.join(", ")) });
                }
            } else {
                crate::build::problem(&app, &slug, format!("The model `{}` that writes the profile isn't available.", cfg.models.synthesize)).await;
            }
        }
        app.jobs.write().await.remove(&key);
        app.bus.emit(Some(&slug), EventKind::JobFinished { job: "learn".into() });
    });
    Ok(())
}

/// From the scheduler: sources not read for a week, and those switched on
/// before they were signed in, now ready.
pub async fn due(app: &App, slug: &str) {
    let now = chrono::Utc::now();
    let mut any = false;
    for (ch, c) in sources(app, slug) {
        let Some(m) = learnable(app, slug, &c).await else { continue };
        if m.learn.is_none() || !crate::accounts::state(app, slug, &c).await.ready() {
            continue;
        }
        let due = match ch.last_learned {
            Some(at) => now - at > EVERY,
            None => {
                let mut t = TRIED.lock().unwrap();
                t.get_or_insert_with(HashSet::new).insert(format!("{slug}/{}", ch.id))
            }
        };
        any |= due;
    }
    if any && !app.jobs.read().await.iter().any(|k| k.ends_with(&format!(":{slug}"))) {
        if let Err(e) = start(app, slug, None).await {
            tracing::info!(%slug, error = %e.1, "not learning from sources now");
        }
    }
}
