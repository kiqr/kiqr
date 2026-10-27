//! Signing in through a manifest's `connect` block: the routes that start a
//! flow and take the service's callback, and the tokens a call needs at the
//! moment it is made (an oauth2 token refreshed, a service account's minted).
//! The flows are generic (`kiqr_core::connect`); what a service wants comes
//! from its manifest. Values land in one connection's `.env` (one account of
//! the ability in a project) through `ext::set_connection_secret`, like typed
//! keys, so nothing else needs to know how they got there.

use crate::api::{ApiError, ApiResult};
use crate::App;
use anyhow::{anyhow, bail, Context, Result};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Json, Router,
};
use kiqr_core::connect::{self, Callback, Connect, Flow};
use kiqr_core::connections::Connection;
use kiqr_core::events::EventKind;
use kiqr_core::extensions::Manifest;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A flow waiting for its callback.
pub struct Pending {
    ability: String,
    /// The connection the values go to.
    connection: String,
    project: String,
    connect: Connect,
    callback: String,
    verifier: String,
    created: Instant,
}

/// Flows waiting, by state; tokens minted for service accounts, by key.
#[derive(Default)]
pub struct Connects {
    pending: tokio::sync::Mutex<HashMap<String, Pending>>,
    tokens: tokio::sync::Mutex<HashMap<String, (String, i64)>>,
}

const PENDING_FOR: Duration = Duration::from_secs(600);

pub fn routes() -> Router<App> {
    Router::new().route("/api/projects/{slug}/connect/{ability}", post(start)).route("/api/connect/callback", get(callback))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

#[derive(Deserialize)]
struct StartBody {
    /// The connection to sign in again; none adds a new one first.
    #[serde(default)]
    connection: Option<String>,
    /// For a new one: its name, and for a channel its kind and destination.
    #[serde(default)]
    label: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    destination: serde_json::Map<String, Value>,
    /// The UI's origin (`window.location.origin`); the callback is under it.
    origin: String,
    /// The `needs` values typed first (the site), saved before the flow starts.
    #[serde(default)]
    values: BTreeMap<String, String>,
}

async fn start(State(app): State<App>, Path((slug, ability)): Path<(String, String)>, Json(b): Json<StartBody>) -> ApiResult<Value> {
    let m = manifest(&app, &slug, &ability).await.ok_or_else(|| bad(format!("{ability} is not installed for this project")))?;
    // The way with a button; a key file (`service_account`) is typed in instead.
    let c = m.connect.iter().find(|c| c.has_button()).cloned().ok_or_else(|| bad(format!("{} signs in with keys only", m.name)))?;
    let origin = url::Url::parse(b.origin.trim()).map_err(|_| bad("origin is not a URL"))?;
    if !matches!(origin.scheme(), "http" | "https") {
        return Err(bad("origin must be http or https"));
    }
    let conn = match b.connection.as_deref().filter(|c| !c.trim().is_empty()) {
        Some(id) => {
            let c = Connection::load(&app.store, &slug, id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
            if c.service() != m.service() {
                return Err(bad(format!("'{id}' is not an account of {}", m.name)));
            }
            c
        }
        None if !b.kind.trim().is_empty() => {
            let preview = crate::publish::channel_preview(&app, &slug, &crate::publish::NewChannel { label: b.label.clone(), kind: b.kind.clone(), connection: Some(m.id.clone()), destination: b.destination.clone(), ..Default::default() }).await?;
            let v = crate::publish::save_channel(&app, &slug, &preview, "user", BTreeMap::new()).await?;
            Connection::load(&app.store, &slug, v["id"].as_str().unwrap_or(""))?
        }
        None => crate::accounts::create(&app, &slug, crate::accounts::NewConnection { ability: m.id.clone(), label: b.label.clone(), ..Default::default() }, "user").await?,
    };
    for n in &c.needs {
        if let Some(v) = b.values.get(n).filter(|v| !v.trim().is_empty()) {
            crate::ext::set_connection_secret(&app, &slug, &conn.id, n, v).await?;
        }
    }
    let callback_url = format!("{}/api/connect/callback", origin.origin().ascii_serialization());
    // kiqr.cloud holds the provider's app: it builds the page, the provider comes back through its relay.
    if c.flow == Flow::Cloud {
        let verifier = connect::random_token();
        let started = crate::signins::cloud_connect(&app, &c.provider, "start", json!({ "return_to": callback_url, "scopes": c.scopes, "params": c.params, "code_challenge": connect::pkce_challenge(&verifier) }))
            .await
            .map_err(|e| bad(format!("{e:#}")))?;
        let (Some(url), Some(state)) = (started["url"].as_str(), started["state"].as_str()) else {
            return Err(bad("kiqr.cloud did not say where to sign in"));
        };
        let mut p = app.connects.pending.lock().await;
        p.retain(|_, f| f.created.elapsed() < PENDING_FOR);
        p.insert(state.to_string(), Pending { ability: m.id.clone(), connection: conn.id.clone(), project: slug.clone(), connect: c.clone(), callback: callback_url, verifier, created: Instant::now() });
        return Ok(Json(json!({ "url": url, "callback": true, "relayed": true, "connection": conn.id, "fields": [] })));
    }
    // A service that only calls back over https, and KIQR on plain http: when
    // this KIQR is signed in to kiqr.cloud, the service calls back to a
    // single-use relay there, which sends the browser on to this KIQR; else
    // the service shows the values and the user pastes them.
    let relayed = if matches!(c.callback, Callback::Https) && origin.scheme() != "https" && c.flow == Flow::Redirect {
        match crate::signins::open_relay(&app, &callback_url).await {
            Ok((_, link)) => Some(link),
            Err(e) => {
                tracing::info!(error = %e, "no kiqr.cloud relay; the user pastes instead");
                None
            }
        }
    } else {
        None
    };
    let offered = match c.callback {
        Callback::Https if origin.scheme() != "https" => relayed.as_deref(),
        _ => Some(callback_url.as_str()),
    };
    let state = connect::random_token();
    let verifier = connect::random_token();
    let env = kiqr_core::connections::env_for(app.store.home(), &slug, Some(&conn), &m);
    let app_id = uuid::Uuid::new_v4().to_string();
    let url = connect::start_url(&c, &env, &app_id, &state, offered, &verifier).map_err(|e| bad(e.to_string()))?;
    let with_callback = offered.is_some();
    {
        let mut p = app.connects.pending.lock().await;
        p.retain(|_, f| f.created.elapsed() < PENDING_FOR);
        if with_callback {
            p.insert(state, Pending { ability: m.id.clone(), connection: conn.id.clone(), project: slug.clone(), connect: c.clone(), callback: callback_url, verifier, created: Instant::now() });
        }
    }
    Ok(Json(json!({ "url": url, "callback": with_callback, "relayed": relayed.is_some(), "connection": conn.id, "fields": c.returns.keys().filter(|k| !c.needs.contains(k)).collect::<Vec<_>>() })))
}

async fn callback(State(app): State<App>, Query(q): Query<BTreeMap<String, String>>) -> (StatusCode, Html<String>) {
    let Some(state) = q.get("state").cloned() else {
        return page(StatusCode::BAD_REQUEST, "This link is not from a sign-in KIQR started.");
    };
    let Some(f) = app.connects.pending.lock().await.remove(&state).filter(|f| f.created.elapsed() < PENDING_FOR) else {
        return page(StatusCode::BAD_REQUEST, "This sign-in has expired. Start it again from Connect.");
    };
    let name = manifest(&app, &f.project, &f.ability).await.map(|m| m.name).unwrap_or_else(|| f.ability.clone());
    match finish(&app, &f, &q).await {
        Ok(()) => {
            app.bus.emit(Some(&f.project), EventKind::ConnectFinished { ability: f.ability.clone(), ok: true, error: None, connection: Some(f.connection.clone()) });
            page(StatusCode::OK, &format!("{name} is connected. You can close this tab."))
        }
        Err(e) => {
            app.bus.emit(Some(&f.project), EventKind::ConnectFinished { ability: f.ability.clone(), ok: false, error: Some(e.to_string()), connection: Some(f.connection.clone()) });
            page(StatusCode::BAD_REQUEST, &format!("{name} is not connected: {e}"))
        }
    }
}

async fn finish(app: &App, f: &Pending, q: &BTreeMap<String, String>) -> Result<()> {
    let conn = Connection::load(&app.store, &f.project, &f.connection)?;
    let m = manifest(app, &f.project, &f.ability).await.ok_or_else(|| anyhow!("{} is no longer installed", f.ability))?;
    let values = match f.connect.flow {
        Flow::Redirect => connect::read_redirect(&f.connect, q)?,
        Flow::Oauth2 => {
            let code = connect::read_code(&f.connect, q)?;
            let env = kiqr_core::connections::env_for(app.store.home(), &f.project, Some(&conn), &m);
            let form = connect::code_form(&f.connect, &env, &code, &f.callback, &f.verifier);
            token_request(&f.connect, &env, &form).await?
        }
        Flow::Cloud => {
            let code = connect::read_code(&f.connect, q)?;
            let body = crate::signins::cloud_connect(app, &f.connect.provider, "token", json!({ "code": code, "code_verifier": f.verifier })).await?;
            connect::read_token(&f.connect, &body, chrono::Utc::now().timestamp())?
        }
        Flow::ServiceAccount => bail!("a service account has no callback"),
    };
    for (n, v) in values {
        crate::ext::set_connection_secret(app, &f.project, &conn.id, &n, &v).await.map_err(|e| anyhow!(e.1))?;
    }
    Ok(())
}

/// POST a form to the manifest's token URL and read the token out of the answer.
async fn token_request(c: &Connect, env: &impl Fn(&str) -> Option<String>, form: &[(String, String)]) -> Result<Vec<(String, String)>> {
    let url = kiqr_core::extensions::fill(&c.token_url, &Value::Null, env);
    let client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(40)).user_agent(kiqr_core::fetch::USER_AGENT).build()?;
    let resp = client.post(&url).header("Accept", "application/json").form(form).send().await.with_context(|| format!("reaching {url}"))?;
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("the token URL answered {status}: {}", connect::error_text(&body));
    }
    connect::read_token(c, &body, chrono::Utc::now().timestamp())
}

async fn manifest(app: &App, slug: &str, id: &str) -> Option<Manifest> {
    let reg = app.registry.read().await;
    reg.entry(Some(slug), id).and_then(|e| e.manifest.clone())
}

fn page(status: StatusCode, msg: &str) -> (StatusCode, Html<String>) {
    let msg = msg.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    (
        status,
        Html(format!(
            r#"<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>KIQR</title>
<style>body{{margin:0;min-height:100vh;display:grid;place-items:center;background:#0b0d10;color:#e6e8eb;font:15px/1.5 Inter,system-ui,sans-serif}}p{{max-width:28rem;padding:0 16px;text-align:center}}</style>
<p>{msg}</p>"#
        )),
    )
}

// ------------------------------------------------------- tokens at call time

/// Variables a call needs beyond the connection's `.env`: a service
/// account's token, minted (and kept until it runs out). An oauth2 token about
/// to expire is refreshed into the connection first (through kiqr.cloud for
/// `cloud`). Of several ways to sign in, the one set up on the account is
/// used. Nothing for a manifest without `connect`, or a call through no connection.
pub async fn call_env(app: &App, slug: &str, m: &Manifest, conn: Option<&Connection>) -> Result<BTreeMap<String, String>> {
    let Some(conn) = conn else { return Ok(BTreeMap::new()) };
    let home = app.store.home();
    let env = kiqr_core::connections::env_for(home, slug, Some(conn), m);
    let Some(c) = connect::pick(&m.connect, &|k: &str| env(k).is_some()) else { return Ok(BTreeMap::new()) };
    let now = chrono::Utc::now().timestamp();
    match c.flow {
        Flow::Redirect => Ok(BTreeMap::new()),
        Flow::Cloud => {
            if !connect::needs_refresh(c, &env, now) {
                return Ok(BTreeMap::new());
            }
            let Some(refresh) = (!c.refresh_env.is_empty()).then(|| env(&c.refresh_env)).flatten() else { return Ok(BTreeMap::new()) };
            let body = crate::signins::cloud_connect(app, &c.provider, "refresh", json!({ "refresh_token": refresh })).await.with_context(|| format!("{} needs connecting again", m.name))?;
            let values = connect::read_token(c, &body, now)?;
            for (n, v) in &values {
                crate::ext::set_connection_secret(app, slug, &conn.id, n, v).await.map_err(|e| anyhow!(e.1))?;
            }
            Ok(values.into_iter().collect())
        }
        Flow::Oauth2 => {
            if !connect::needs_refresh(c, &env, now) {
                return Ok(BTreeMap::new());
            }
            let Some(refresh) = (!c.refresh_env.is_empty()).then(|| env(&c.refresh_env)).flatten() else {
                // No way to renew: the stored token is tried as it is; the service says when it is dead.
                return Ok(BTreeMap::new());
            };
            let values = token_request(c, &env, &connect::refresh_form(c, &env, &refresh)).await.with_context(|| format!("{} needs connecting again", m.name))?;
            for (n, v) in &values {
                crate::ext::set_connection_secret(app, slug, &conn.id, n, v).await.map_err(|e| anyhow!(e.1))?;
            }
            Ok(values.into_iter().collect())
        }
        Flow::ServiceAccount => {
            let path = env(&c.key_file).ok_or_else(|| anyhow!("{} is not set", c.key_file))?;
            let cache_key = format!("{slug}|{}|{}|{}|{}", conn.id, path.trim(), c.token_url, c.scopes.join(" "));
            if let Some((t, exp)) = app.connects.tokens.lock().await.get(&cache_key).cloned() {
                if exp - 60 > now {
                    return Ok(BTreeMap::from([(c.token_env.clone(), t)]));
                }
            }
            let text = std::fs::read_to_string(path.trim()).with_context(|| format!("reading the key file {}", path.trim()))?;
            let key = connect::read_service_key(&text)?;
            let der = connect::pem_to_der(&key.private_key_pem)?;
            let pair = ring::signature::RsaKeyPair::from_pkcs8(&der).map_err(|e| anyhow!("the private key is not PKCS#8: {e}"))?;
            let unsigned = connect::jwt_unsigned(c, &key, now);
            let mut sig = vec![0u8; pair.public().modulus_len()];
            pair.sign(&ring::signature::RSA_PKCS1_SHA256, &ring::rand::SystemRandom::new(), unsigned.as_bytes(), &mut sig).map_err(|e| anyhow!("signing the token failed: {e}"))?;
            let signed = format!("{unsigned}.{}", base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &sig));
            // The minted token is not stored: read it from the answer by hand.
            let probe = Connect { token_env: c.token_env.clone(), refresh_env: String::new(), returns: BTreeMap::new(), ..c.clone() };
            let values = token_request(&probe, &env, &connect::jwt_form(&signed)).await.with_context(|| format!("{}: the service account was refused", m.name))?;
            let token = values.iter().find(|(n, _)| n == &c.token_env).map(|(_, v)| v.clone()).context("no token in the answer")?;
            let exp = values.iter().find(|(n, _)| n == &c.expires_env()).and_then(|(_, v)| v.parse().ok()).unwrap_or(now + 3600);
            app.connects.tokens.lock().await.insert(cache_key, (token.clone(), exp));
            Ok(BTreeMap::from([(c.token_env.clone(), token)]))
        }
    }
}

pub type Shared = Arc<Connects>;
