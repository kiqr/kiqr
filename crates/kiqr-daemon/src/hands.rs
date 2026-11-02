//! Remote hands: a person sees what an account's hosted program shows and
//! drives it for a moment, to finish a sign-in the runner could not (a
//! captcha, a device check). The manifest's `live_view` names the program's
//! tools: one answers with an image of the page, the others click, press
//! keys, type and open a URL. The daemon polls the image a few times a second
//! while someone watches and sends each frame over the project's event
//! stream, never keeping one; what the person types goes straight to the
//! program and is never logged. The account's turn is held for the whole
//! time, so nothing else drives it. A session ends on "done", after ten idle
//! minutes, or when the daemon stops. Only a person in KIQR's web UI can
//! start one (the same guard as a factory reset), with a token per session.

use crate::api::{ApiError, ApiResult};
use crate::App;
use anyhow::{anyhow, Result};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use kiqr_core::accounts::Account;
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{LiveView, Manifest, Step};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

const IDLE: Duration = Duration::from_secs(600);
const FRAME_EVERY: Duration = Duration::from_millis(400);

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/accounts/{id}/hands", post(start_route))
        .route("/api/projects/{slug}/hands/{hands}/input", post(input_route))
        .route("/api/projects/{slug}/hands/{hands}/end", post(end_route))
}

struct Hands {
    slug: String,
    connection: String,
    token: String,
    manifest: Manifest,
    program: Manifest,
    launch: crate::mcp_host::Launch,
    env: HashMap<String, String>,
    last_input: Mutex<Instant>,
    /// Held for the whole session: nothing else drives the account meanwhile.
    _turn: tokio::sync::OwnedMutexGuard<()>,
    open: std::sync::atomic::AtomicBool,
}

static OPEN: LazyLock<Mutex<HashMap<String, Arc<Hands>>>> = LazyLock::new(Default::default);

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

/// Only a person in the web UI starts or drives a session: never Seemo, never another site.
pub(crate) fn from_browser(h: &HeaderMap) -> Result<(), ApiError> {
    let refuse = || ApiError(StatusCode::FORBIDDEN, "Only a person in KIQR's web UI can take the controls.".into());
    if h.contains_key(crate::reset::AGENT_HEADER) {
        return Err(refuse());
    }
    let get = |k: &str| h.get(k).and_then(|v| v.to_str().ok()).unwrap_or("").trim();
    let site = get("sec-fetch-site");
    if !site.is_empty() && site != "same-origin" {
        return Err(refuse());
    }
    let origin = get("origin");
    let host_of = |o: &str| o.strip_prefix("http://").or_else(|| o.strip_prefix("https://")).map(str::to_string);
    let Some(origin_host) = host_of(origin) else { return Err(refuse()) };
    let forwarded = get("x-forwarded-host").split(',').next().unwrap_or("").trim();
    if !origin_host.is_empty() && [get("host"), forwarded].iter().any(|h| !h.is_empty() && h.eq_ignore_ascii_case(&origin_host)) {
        return Ok(());
    }
    Err(refuse())
}

fn token() -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut h);
    std::process::id().hash(&mut h);
    let a = h.finish();
    (a ^ 0x9e37_79b9_7f4a_7c15u64.rotate_left(17)).hash(&mut h);
    format!("{a:016x}{:016x}", h.finish())
}

impl Hands {
    fn env(&self) -> impl Fn(&str) -> Option<String> + Clone + '_ {
        move |k: &str| self.env.get(k).cloned()
    }

    /// One step of the live view with the given values.
    async fn step(&self, app: &App, st: &Step, args: Value) -> Result<crate::mcp_host::McpOut> {
        if st.call.trim().is_empty() {
            return Err(anyhow!("the live view has no tool for that"));
        }
        let call_args = match &st.args {
            Some(t) => kiqr_core::extensions::fill_value(t, &args, &self.env()),
            None => json!({}),
        };
        if let Some(u) = call_args.get("url").and_then(Value::as_str) {
            crate::ext::check_host(&self.manifest, u, &self.env())?;
        }
        let r = app.mcp.call(app.store.home().root(), &self.program, &self.launch, &st.call, &call_args).await?;
        Ok(crate::mcp_host::read_result(&r))
    }

    fn touch(&self) {
        *self.last_input.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
    }

    fn idle(&self) -> bool {
        self.last_input.lock().unwrap_or_else(|e| e.into_inner()).elapsed() > IDLE
    }
}

/// Frames to whoever watches, until the session ends or goes idle.
async fn stream(app: App, id: String, h: Arc<Hands>, view: LiveView) {
    use std::sync::atomic::Ordering;
    use base64::Engine;
    while h.open.load(Ordering::SeqCst) {
        if h.idle() {
            end(&app, &id, "no one at the controls for ten minutes").await;
            break;
        }
        match h.step(&app, &view.frame, json!({})).await {
            Ok(out) => {
                if let Some((bytes, mime)) = out.file {
                    let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    app.bus.emit(Some(&h.slug), EventKind::HandsFrame { account: h.connection.clone(), hands: id.clone(), mime, data, url: String::new() });
                }
            }
            Err(e) => {
                tracing::warn!(account = %h.connection, "live view: {e:#}");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
        tokio::time::sleep(FRAME_EVERY).await;
    }
}

async fn end(app: &App, id: &str, why: &str) {
    let h = OPEN.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    if let Some(h) = h {
        h.open.store(false, std::sync::atomic::Ordering::SeqCst);
        app.bus.emit(Some(&h.slug), EventKind::HandsEnded { account: h.connection.clone(), hands: id.to_string(), message: why.to_string() });
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct StartBody {
    /// A URL to open first (the site's sign-in page); the account's hosts only.
    url: String,
}

/// `POST …/accounts/{id}/hands`: take the controls. Answers the session id and its token.
async fn start_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>, headers: HeaderMap, Json(b): Json<StartBody>) -> ApiResult<Value> {
    from_browser(&headers)?;
    let c = Account::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    let m = {
        let reg = app.registry.read().await;
        let off = crate::ext::project_off(&app, &slug);
        reg.of_service(&slug, &c.service).into_iter().find(|m| m.live_view.is_some() && !off.contains(&m.id)).cloned()
    }
    .ok_or_else(|| bad(format!("{} has no live view", c.label)))?;
    if let st @ (kiqr_core::accounts::State::MissingKeys { .. } | kiqr_core::accounts::State::NeedsProgram { .. }) = crate::accounts::state(&app, &slug, &c).await {
        return Err(bad(crate::accounts::state_text(&c, &st)));
    }
    // One session per account.
    let already: Option<String> = OPEN.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(_, h)| h.slug == slug && h.connection == c.id).map(|(k, _)| k.clone());
    if let Some(open) = already {
        end(&app, &open, "someone else took the controls").await;
    }
    let view = m.live_view.clone().expect("live_view");
    let program = crate::ext::program_of(&app, Some(&slug), &m).await.map_err(|e| bad(e.to_string()))?;
    let env_fn = kiqr_core::accounts::env_for(app.store.home(), &slug, Some(&c), &m, Default::default());
    let launch = crate::ext::mcp_env(&app, &slug, &m, Some(&c), &env_fn);
    let env: HashMap<String, String> = kiqr_core::accounts::key_names(&m).into_iter().filter_map(|k| env_fn(&k).map(|v| (k, v))).collect();
    let turn = crate::ext::account_turn(&slug, &c.id).await;
    let hands_id = token();
    let h = Arc::new(Hands { slug: slug.clone(), connection: c.id.clone(), token: token(), manifest: m.clone(), program, launch, env, last_input: Mutex::new(Instant::now()), _turn: turn, open: std::sync::atomic::AtomicBool::new(true) });
    OPEN.lock().unwrap_or_else(|e| e.into_inner()).insert(hands_id.clone(), h.clone());
    // Open the page asked for, else the one the sign-in check starts from.
    let first = Some(b.url.trim().to_string()).filter(|u| !u.is_empty()).or_else(|| crate::session::start_url(&app, &slug, &c, &m));
    if let Some(u) = first {
        if let Err(e) = h.step(&app, &view.navigate, json!({ "url": u })).await {
            tracing::warn!(account = %c.id, "live view: could not open the first page: {e:#}");
        }
    }
    app.bus.emit(Some(&slug), EventKind::HandsStarted { account: c.id.clone(), hands: hands_id.clone() });
    tokio::spawn(stream(app.clone(), hands_id.clone(), h.clone(), view.clone()));
    Ok(Json(json!({ "hands": hands_id, "token": h.token, "can": { "click": !view.click.call.is_empty(), "key": !view.key.call.is_empty(), "type": !view.type_.call.is_empty(), "navigate": !view.navigate.call.is_empty() } })))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct InputBody {
    token: String,
    /// `click`, `key`, `type`, `navigate`.
    kind: String,
    /// For a click: where, as a fraction of the frame (0 to 1), and the frame's size in pixels.
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    key: String,
    text: String,
    url: String,
}

/// `POST …/hands/{hands}/input`: a click, a key, text or a URL from the person at the controls.
async fn input_route(State(app): State<App>, Path((slug, hands)): Path<(String, String)>, headers: HeaderMap, Json(b): Json<InputBody>) -> ApiResult<Value> {
    from_browser(&headers)?;
    let h = OPEN.lock().unwrap_or_else(|e| e.into_inner()).get(&hands).cloned().ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "that session is over".into()))?;
    if h.slug != slug || h.token != b.token {
        return Err(ApiError(StatusCode::FORBIDDEN, "not your session".into()));
    }
    h.touch();
    let view = h.manifest.live_view.clone().expect("live_view");
    // Text goes key by key when the step takes `{{char}}` (a real keystroke each, which a page cannot tell from a person's).
    if b.kind == "type" && view.type_.args.as_ref().is_some_and(|a| a.to_string().contains("{{char}}")) {
        for ch in b.text.chars() {
            let key = match ch {
                '\n' => "Enter".to_string(),
                '\t' => "Tab".to_string(),
                c => c.to_string(),
            };
            let out = h.step(&app, &view.type_, json!({ "char": key })).await.map_err(|e| bad(format!("{e:#}")))?;
            if out.is_error {
                return Ok(Json(json!({ "ok": false, "message": out.text })));
            }
        }
        return Ok(Json(json!({ "ok": true })));
    }
    let mut geometry = Value::Null;
    let (step, args) = match b.kind.as_str() {
        "click" => {
            let (mut px, mut py) = ((b.x * b.width.max(1.0)).round() as i64, (b.y * b.height.max(1.0)).round() as i64);
            // A frame of the whole window: the click is moved onto the page (below the tab strip and address bar), in CSS pixels.
            if !view.metrics.call.trim().is_empty() {
                let m = h.step(&app, &view.metrics, json!({})).await.map_err(|e| bad(format!("{e:#}")))?;
                // The answer is JSON, or a `result` holding the JSON as text (an MCP tool's answer wrapped once).
                let mut g: Value = m.structured.clone().unwrap_or_else(|| serde_json::from_str(m.text.trim()).unwrap_or(Value::Null));
                for _ in 0..2 {
                    match g.get("result").cloned() {
                        Some(Value::String(t)) => g = serde_json::from_str(t.trim()).unwrap_or(Value::Null),
                        Some(inner @ Value::Object(_)) => g = inner,
                        _ => break,
                    }
                }
                let num = |k: &str| g.get(k).and_then(Value::as_f64).filter(|v| *v > 0.0);
                geometry = g.clone();
                if let (Some(ow), Some(oh), Some(iw), Some(ih)) = (num("ow"), num("oh"), num("iw"), num("ih")) {
                    // Where the page sits in the window: as the page says (`ox`, `oy`, from its position on the screen), else
                    // below the window's bars and between its side borders (half the width difference each side).
                    let off = |k: &str| g.get(k).and_then(Value::as_f64).filter(|v| *v >= 0.0);
                    let (ox, oy) = (off("ox").unwrap_or((ow - iw) / 2.0), off("oy").unwrap_or(oh - ih));
                    let (x, y) = (b.x * ow - ox, b.y * oh - oy);
                    if y < 0.0 || x < 0.0 {
                        return Ok(Json(json!({ "ok": false, "message": "that is the browser's own bar, not the page" })));
                    }
                    (px, py) = (x.round() as i64, y.round() as i64);
                }
            }
            (&view.click, json!({ "x": b.x, "y": b.y, "px": px, "py": py }))
        }
        "key" => (&view.key, json!({ "key": b.key })),
        "type" => (&view.type_, json!({ "text": b.text })),
        "navigate" => (&view.navigate, json!({ "url": b.url })),
        other => return Err(bad(format!("'{other}' is not something the live view does"))),
    };
    // What the person types is never logged; only that something happened.
    let mut at = json!({ "px": args.get("px"), "py": args.get("py"), "frame": [b.width, b.height], "geometry": geometry });
    let out = h.step(&app, step, args).await.map_err(|e| bad(format!("{e:#}")))?;
    if !out.text.trim().is_empty() {
        at["answer"] = Value::String(kiqr_core::profile::truncate_chars(out.text.trim(), 200));
    }
    // Where a click went (page pixels, the frame's size, the window's geometry): what a person needs to see why it landed where it did.
    Ok(Json(json!({ "ok": !out.is_error, "message": if out.is_error { Value::String(out.text) } else { Value::Null }, "at": if b.kind == "click" { at } else { Value::Null } })))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct EndBody {
    token: String,
    /// The person says they finished the sign-in: the runner checks right after.
    signed_in: bool,
}

async fn end_route(State(app): State<App>, Path((slug, hands)): Path<(String, String)>, headers: HeaderMap, Json(b): Json<EndBody>) -> ApiResult<Value> {
    from_browser(&headers)?;
    let h = OPEN.lock().unwrap_or_else(|e| e.into_inner()).get(&hands).cloned().ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "that session is over".into()))?;
    if h.slug != slug || h.token != b.token {
        return Err(ApiError(StatusCode::FORBIDDEN, "not your session".into()));
    }
    let connection = h.connection.clone();
    drop(h);
    end(&app, &hands, if b.signed_in { "signed in by hand" } else { "done" }).await;
    if b.signed_in {
        // The turn is free again: the runner checks and keeps the result.
        let (app2, slug2, id2) = (app.clone(), slug.clone(), connection.clone());
        tokio::spawn(async move {
            if let Err(e) = crate::session::signin(&app2, &slug2, &id2).await {
                tracing::warn!(account = %id2, "check after hands: {e:#}");
            }
        });
    }
    Ok(Json(json!({ "ended": true })))
}
