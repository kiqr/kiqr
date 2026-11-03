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
use kiqr_core::conn::Conn;
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

/// The ability that reads a channel's documents, when its kind's ability can teach the project.
async fn learnable(app: &App, slug: &str, ch: &Channel) -> Option<Manifest> {
    crate::channels::kind_for(app, slug, ch).await.map(|(m, _)| m).filter(|m| m.learn.is_some())
}

/// The channels learned from, with the ability and the credential each is read through.
async fn sources(app: &App, slug: &str) -> Vec<(Channel, Manifest, Option<Conn>)> {
    let mut out = Vec::new();
    for ch in Channel::all(&app.store, slug).unwrap_or_default().into_iter().filter(|ch| ch.learn && ch.owned() && !ch.suggested) {
        let Some(m) = learnable(app, slug, &ch).await else { continue };
        let named = Some(ch.publisher.as_str()).filter(|p| !p.is_empty());
        match crate::creds::pick(app, slug, &m, named).await {
            Ok(c) => out.push((ch, m, c)),
            Err(e) => tracing::info!(%slug, channel = %ch.id, error = %e, "a source cannot be read yet"),
        }
    }
    out
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

async fn call(app: &App, slug: &str, m: &Manifest, c: Option<&Conn>, tool: &str, args: &Value) -> anyhow::Result<crate::ext::ExecOut> {
    let t = m.tools.iter().find(|t| t.name == tool).ok_or_else(|| anyhow::anyhow!("{} has no tool '{tool}'", m.name))?;
    crate::ext::execute(app, slug, m, &t.provide, None, Some(tool), args, c.map(|c| c.id())).await
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
pub async fn read_source(app: &App, slug: &str, ch: &Channel, m: &Manifest, c: Option<&Conn>, ex: &Extractor) -> anyhow::Result<usize> {
    if let Some(c) = c {
        let st = crate::creds::state(app, slug, c.primary()).await;
        if !st.ready() {
            anyhow::bail!("{}", crate::creds::state_text(c.label(), &st));
        }
    }
    let m = m.clone();
    let spec = m.learn.clone().unwrap_or_default();
    // Titles and links may name settings (`${GITHUB_REPO}`), never a secret: they are kept in the facts.
    let env = match c {
        Some(c) => std::sync::Arc::new(crate::ext::target_env(app, slug, &m, c)) as std::sync::Arc<dyn Fn(&str) -> Option<String> + Send + Sync>,
        None => {
            let set = crate::creds::settings_of(app, slug, &m.id).into_iter().map(|(k, v)| (k.to_ascii_uppercase(), v)).collect::<std::collections::BTreeMap<_, _>>();
            std::sync::Arc::new(move |k: &str| set.get(k).cloned())
        }
    };
    let env = move |k: &str| env(k);
    // The channel's own spot (a website's repository) goes with every call and fills `{{repo}}` in titles and links.
    let spot: serde_json::Map<String, Value> = crate::channels::kind_for(app, slug, ch).await.map(|(_, k)| crate::channels::fields_for(ch, &k)).unwrap_or_default().into_iter().collect();
    let ctx = Value::Object(spot.clone());
    let with_spot = |mut a: Value| {
        if let Some(o) = a.as_object_mut() {
            for (k, v) in &spot {
                o.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        a
    };
    let mut read = 0usize;
    for d in &spec.documents {
        let LearnDoc { tool, args, title, url, each } = d;
        let args = with_spot(if args.is_null() { json!({}) } else { extensions::fill_value(args, &ctx, &env) });
        let out = match call(app, slug, &m, c, tool, &args).await {
            Ok(o) => o,
            Err(e) => {
                tracing::info!(%slug, channel = %ch.label, %tool, error = %e, "a document to learn from is not there");
                continue;
            }
        };
        match each {
            None => read += extract(ex, slug, extensions::fill(title, &ctx, &env), extensions::fill(url, &ctx, &env), doc_text(&out)).await,
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
                    let mut at = ctx.clone();
                    at["value"] = json!(v);
                    match call(app, slug, &m, c, &e.tool, &with_spot(json!({ e.arg.clone(): v }))).await {
                        Ok(o) => read += extract(ex, slug, extensions::fill(title, &at, &env), extensions::fill(url, &at, &env), doc_text(&o)).await,
                        Err(err) => tracing::info!(%slug, channel = %ch.label, value = %v, error = %err, "could not read a document to learn from"),
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
    for (ch, m, c) in sources(app, slug).await.into_iter().filter(|(ch, _, _)| only.is_none_or(|o| o == ch.id)) {
        let r = read_source(app, slug, &ch, &m, c.as_ref(), ex).await;
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
    for (ch, m, c) in sources(app, slug).await {
        if m.learn.is_none() {
            continue;
        }
        if let Some(c) = &c {
            if !crate::creds::state(app, slug, c.primary()).await.ready() {
                continue;
            }
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
