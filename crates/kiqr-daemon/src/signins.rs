//! KIQR's own sign-ins: kiqr.cloud (the account that connects a KIQR agent
//! with third-party apps, and relays their sign-ins), and UnblockingAPI by
//! account instead of an API key (fetching then goes through its MCP server).
//! These are core: kiqr.cloud is KIQR's own service and UnblockingAPI the
//! one service core may name. Abilities sign in through their manifest
//! instead (`connect.rs`).
//!
//! Each machine registers itself once per service as a public client named
//! "KIQR on <host>" (RFC 7591). A UI opened on this computer signs in with the
//! code flow and PKCE back to `{origin}/api/signins/callback`; elsewhere, the
//! device flow when the service has one (kiqr.cloud), else the kiqr.cloud
//! relay. Tokens live in the machine's `.env` (`provider::SIGNIN_KEYS`) and are
//! refreshed before use.

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
use kiqr_core::events::EventKind;
use kiqr_core::oauth::{self, Poll, Server, Tokens};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

/// One of KIQR's own sign-ins.
pub struct SignIn {
    pub id: &'static str,
    pub name: &'static str,
    scope: &'static str,
    /// The `.env` prefix: `{prefix}_TOKEN`, `_REFRESH_TOKEN`, `_TOKEN_EXPIRES_AT`, `_CLIENT_ID`, `_ACCOUNT`.
    prefix: &'static str,
}

pub const CLOUD: &str = "kiqr-cloud";
pub const UNBLOCKINGAPI: &str = "unblockingapi";

pub const SIGNINS: &[SignIn] = &[
    SignIn { id: CLOUD, name: "kiqr.cloud", scope: "account", prefix: "KIQR_CLOUD" },
    SignIn { id: UNBLOCKINGAPI, name: "UnblockingAPI", scope: "mcp", prefix: "UNBLOCKINGAPI" },
];

const CALLBACK_PATH: &str = "/api/signins/callback";
const PENDING_FOR: Duration = Duration::from_secs(600);

fn find(id: &str) -> Result<&'static SignIn, ApiError> {
    SIGNINS.iter().find(|s| s.id == id).ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no sign-in '{id}'")))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

/// Where a sign-in's tokens live: the machine's `.env` (every project), or
/// one project's `.env`. kiqr.cloud is always the machine's (it links the
/// agent); UnblockingAPI may be either, and a project's own wins.
#[derive(Clone, Debug, PartialEq)]
pub enum Place {
    Machine,
    Project(String),
}

/// A browser sign-in waiting for its callback.
struct Pending {
    id: &'static str,
    place: Place,
    server: Server,
    client_id: String,
    redirect_uri: String,
    verifier: String,
    created: Instant,
}

/// What a device sign-in shows while the user approves.
#[derive(Clone, Default, serde::Serialize)]
struct DeviceView {
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
}

#[derive(Default)]
pub struct SignInState {
    pending: tokio::sync::Mutex<HashMap<String, Pending>>,
    devices: tokio::sync::Mutex<HashMap<&'static str, DeviceView>>,
    servers: tokio::sync::Mutex<HashMap<&'static str, (Server, Instant)>>,
    /// One refresh at a time per sign-in.
    refreshing: tokio::sync::Mutex<()>,
}

pub type Shared = std::sync::Arc<SignInState>;

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/signins", get(list))
        .route("/api/signins/callback", get(callback))
        .route("/api/signins/{id}", axum::routing::delete(sign_out))
        .route("/api/signins/{id}/start", post(start))
}

// ------------------------------------------------------------------ values

fn env_name(s: &SignIn, what: &str) -> String {
    format!("{}_{what}", s.prefix)
}

fn get_env(s: &SignIn, what: &str) -> Option<String> {
    std::env::var(env_name(s, what)).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Write one of the machine's sign-in values (`.env` and the running process).
fn put_env(app: &App, s: &SignIn, what: &str, value: Option<&str>) -> Result<()> {
    let name = env_name(s, what);
    let value = value.unwrap_or("");
    kiqr_core::secrets::write_env(&app.store.home().env_file(), &name, value)?;
    if value.is_empty() {
        std::env::remove_var(&name);
    } else {
        std::env::set_var(&name, value);
    }
    Ok(())
}

/// One sign-in value where it lives.
fn read(home: &kiqr_core::paths::Home, s: &SignIn, what: &str, place: &Place) -> Option<String> {
    match place {
        Place::Machine => get_env(s, what),
        Place::Project(slug) => kiqr_core::secrets::read_env_file(&home.project_env_file(slug)).remove(&env_name(s, what)).filter(|v| !v.is_empty()),
    }
}

fn write(app: &App, s: &SignIn, what: &str, value: Option<&str>, place: &Place) -> Result<()> {
    match place {
        Place::Machine => put_env(app, s, what, value),
        Place::Project(slug) => kiqr_core::secrets::write_env(&app.store.home().project_env_file(slug), &env_name(s, what), value.unwrap_or("")),
    }
}

fn save_tokens(app: &App, s: &SignIn, t: &Tokens, place: &Place) -> Result<()> {
    write(app, s, "TOKEN", Some(&t.access), place)?;
    write(app, s, "REFRESH_TOKEN", t.refresh.as_deref(), place)?;
    write(app, s, "TOKEN_EXPIRES_AT", t.expires_at.map(|e| e.to_string()).as_deref(), place)?;
    Ok(())
}

fn clear(app: &App, s: &SignIn, place: &Place) -> Result<()> {
    for what in ["TOKEN", "REFRESH_TOKEN", "TOKEN_EXPIRES_AT", "ACCOUNT"] {
        write(app, s, what, None, place)?;
    }
    Ok(())
}

/// Signed in for every project (the machine's sign-in).
pub fn signed_in(s: &SignIn) -> bool {
    get_env(s, "TOKEN").is_some()
}

fn signed_in_at(home: &kiqr_core::paths::Home, s: &SignIn, place: &Place) -> bool {
    read(home, s, "TOKEN", place).is_some()
}

/// Where a core ability's sign-in for this project comes from: `project`
/// (its own), `machine` (every project's), or none.
pub fn signin_scope(home: &kiqr_core::paths::Home, slug: Option<&str>, ability: &str) -> Option<&'static str> {
    let s = SIGNINS.iter().find(|s| s.id == ability)?;
    if slug.is_some_and(|slug| signed_in_at(home, s, &Place::Project(slug.to_string()))) {
        Some("project")
    } else if signed_in(s) {
        Some("machine")
    } else {
        None
    }
}

/// Whether a core ability's key is there: set (the project's or the
/// machine's), or stood in for by a sign-in (UnblockingAPI by account).
pub fn has_core_key(home: &kiqr_core::paths::Home, slug: Option<&str>, env: &str) -> bool {
    kiqr_core::secrets::has(home, slug, env) || (env == "UNBLOCKINGAPI_KEY" && signin_scope(home, slug, UNBLOCKINGAPI).is_some())
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "this computer".into())
}

fn loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

// ---------------------------------------------------------------- servers

async fn cloud_url(app: &App) -> String {
    app.config.read().await.cloud.url.trim_end_matches('/').to_string()
}

/// The service's authorization server, looked up once an hour.
async fn server(app: &App, s: &'static SignIn) -> Result<Server> {
    if let Some((srv, at)) = app.signins.servers.lock().await.get(s.id).cloned() {
        if at.elapsed() < Duration::from_secs(3600) {
            return Ok(srv);
        }
    }
    let issuer = match s.id {
        CLOUD => cloud_url(app).await,
        _ => {
            let mcp = url::Url::parse(&kiqr_core::fetch::unblocking_mcp_url())?;
            oauth::resource_issuer(&format!("{}/.well-known/oauth-protected-resource", mcp.origin().ascii_serialization())).await?
        }
    };
    let srv = oauth::discover(&issuer).await.with_context(|| format!("finding {}'s sign-in", s.name))?;
    app.signins.servers.lock().await.insert(s.id, (srv.clone(), Instant::now()));
    Ok(srv)
}

/// The redirect URIs this machine registers: the loopback callback (any port),
/// and for a service without a device flow, the kiqr.cloud relay.
async fn redirect_uris(app: &App, s: &SignIn) -> Vec<String> {
    let mut v = vec![format!("http://127.0.0.1{CALLBACK_PATH}"), format!("http://localhost{CALLBACK_PATH}")];
    if s.id != CLOUD {
        let cloud = cloud_url(app).await;
        if cloud.starts_with("https://") {
            v.push(format!("{cloud}/relay"));
        }
    }
    v
}

/// This machine's client id at the service, registered on first use.
async fn client_id(app: &App, s: &'static SignIn, srv: &Server) -> Result<String> {
    if let Some(id) = get_env(s, "CLIENT_ID") {
        return Ok(id);
    }
    let grants: &[&str] = if srv.has_device_flow() { &["authorization_code", "refresh_token", "urn:ietf:params:oauth:grant-type:device_code"] } else { &["authorization_code", "refresh_token"] };
    let id = oauth::register(srv, &format!("KIQR on {}", hostname()), &redirect_uris(app, s).await, grants).await?;
    put_env(app, s, "CLIENT_ID", Some(&id))?;
    Ok(id)
}

// ----------------------------------------------------------------- tokens

/// The machine's valid access token for a sign-in (see `token_at`).
pub async fn token(app: &App, id: &str) -> Option<String> {
    token_at(app, id, &Place::Machine).await
}

/// A valid access token where the sign-in lives, refreshed when it runs out
/// within five minutes. None when not signed in there; a refresh the service
/// refuses signs that place out.
pub async fn token_at(app: &App, id: &str, place: &Place) -> Option<String> {
    let s = SIGNINS.iter().find(|s| s.id == id)?;
    let home = app.store.home();
    let access = read(home, s, "TOKEN", place)?;
    let expires = read(home, s, "TOKEN_EXPIRES_AT", place).and_then(|e| e.parse::<i64>().ok());
    if expires.is_none_or(|e| e - 300 > now()) {
        return Some(access);
    }
    let _guard = app.signins.refreshing.lock().await;
    // Another call may have refreshed while this one waited.
    if let Some(e) = read(home, s, "TOKEN_EXPIRES_AT", place).and_then(|e| e.parse::<i64>().ok()) {
        if e - 300 > now() {
            return read(home, s, "TOKEN", place);
        }
    }
    let result: Result<Tokens> = async {
        let refresh = read(home, s, "REFRESH_TOKEN", place).ok_or_else(|| anyhow!("no refresh token"))?;
        let srv = server(app, s).await?;
        let cid = get_env(s, "CLIENT_ID").ok_or_else(|| anyhow!("no client id"))?;
        oauth::refresh(&srv, &cid, &refresh, now()).await
    }
    .await;
    match result {
        Ok(t) => {
            let _ = save_tokens(app, s, &t, place);
            Some(t.access)
        }
        Err(e) => {
            tracing::warn!(signin = s.id, error = %e, "refresh failed; signed out");
            let _ = clear(app, s, place);
            app.bus.emit(None, EventKind::SignInChanged { id: s.id.into(), signed_in: false, error: Some(format!("{e:#}")) });
            None
        }
    }
}

/// How this project reaches UnblockingAPI, its own first: its key, its
/// sign-in, then the machine's key, the machine's sign-in. None when the
/// ability is off or nothing is set.
pub async fn unblocking_auth(app: &App, slug: &str) -> Option<kiqr_core::fetch::UnblockingAuth> {
    use kiqr_core::fetch::UnblockingAuth;
    if !app.config.read().await.core_on("unblockingapi") {
        return None;
    }
    let home = app.store.home();
    if let Some(k) = UnblockingAuth::key(kiqr_core::secrets::read_env_file(&home.project_env_file(slug)).remove("UNBLOCKINGAPI_KEY")) {
        return Some(k);
    }
    if let Some(t) = token_at(app, UNBLOCKINGAPI, &Place::Project(slug.to_string())).await {
        return Some(UnblockingAuth::Token(t));
    }
    if let Some(k) = UnblockingAuth::key(std::env::var("UNBLOCKINGAPI_KEY").ok()) {
        return Some(k);
    }
    token(app, UNBLOCKINGAPI).await.map(UnblockingAuth::Token)
}

/// A relay on kiqr.cloud back to `return_to` (one of this KIQR's callbacks):
/// `(relay id, https URL to hand the service)`. Needs a kiqr.cloud sign-in.
pub async fn open_relay(app: &App, return_to: &str) -> Result<(String, String)> {
    let token = token(app, CLOUD).await.ok_or_else(|| anyhow!("not signed in to kiqr.cloud"))?;
    let url = format!("{}/api/v1/relays", cloud_url(app).await);
    let resp = reqwest::Client::builder()
        .user_agent(kiqr_core::fetch::USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()?
        .post(&url)
        .bearer_auth(token)
        .json(&json!({ "return_to": return_to }))
        .send()
        .await
        .context("reaching kiqr.cloud")?;
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("kiqr.cloud answered {status}: {}", oauth::error_text(&v));
    }
    let id = v["id"].as_str().ok_or_else(|| anyhow!("kiqr.cloud gave no relay id"))?.to_string();
    let link = v["url"].as_str().ok_or_else(|| anyhow!("kiqr.cloud gave no relay link"))?.to_string();
    Ok((id, link))
}

/// The account a sign-in is for, to show: kiqr.cloud's email.
async fn fetch_account(app: &App, s: &SignIn, access: &str) -> Option<String> {
    if s.id != CLOUD {
        return None;
    }
    let url = format!("{}/api/v1/me", cloud_url(app).await);
    let v: Value = reqwest::Client::new().get(&url).bearer_auth(access).timeout(Duration::from_secs(15)).send().await.ok()?.json().await.ok()?;
    v["email"].as_str().map(str::to_string)
}

async fn finish(app: &App, s: &'static SignIn, t: Tokens, place: &Place) -> Result<()> {
    save_tokens(app, s, &t, place)?;
    if let Some(account) = fetch_account(app, s, &t.access).await {
        write(app, s, "ACCOUNT", Some(&account), place)?;
    }
    app.bus.emit(None, EventKind::SignInChanged { id: s.id.into(), signed_in: true, error: None });
    Ok(())
}

// ----------------------------------------------------------------- routes

#[derive(Deserialize, Default)]
#[serde(default)]
struct ProjectQuery {
    project: Option<String>,
}

/// `?project=` adds whether that project has its own sign-in (`here`).
async fn list(State(app): State<App>, Query(q): Query<ProjectQuery>) -> ApiResult<Value> {
    let devices = app.signins.devices.lock().await.clone();
    let home = app.store.home();
    let here = q.project.filter(|p| !p.is_empty()).map(Place::Project);
    let rows: Vec<Value> = SIGNINS
        .iter()
        .map(|s| {
            let own = here.as_ref().is_some_and(|p| signed_in_at(home, s, p));
            json!({
                "id": s.id,
                "name": s.name,
                "per_project": s.id != CLOUD,
                "signed_in": signed_in(s) || own,
                "scope": if own { Some("project") } else if signed_in(s) { Some("machine") } else { None },
                "account": here.as_ref().filter(|_| own).and_then(|p| read(home, s, "ACCOUNT", p)).or_else(|| get_env(s, "ACCOUNT")),
                "waiting": devices.get(s.id),
            })
        })
        .collect();
    Ok(Json(json!({ "signins": rows, "cloud_url": cloud_url(&app).await })))
}

#[derive(Deserialize)]
struct StartBody {
    /// The UI's origin (`window.location.origin`), or the daemon's for the CLI.
    origin: String,
    /// Sign in for this project only; none signs in for every project.
    #[serde(default)]
    project: Option<String>,
}

async fn start(State(app): State<App>, Path(id): Path<String>, Json(b): Json<StartBody>) -> ApiResult<Value> {
    let s = find(&id)?;
    let place = match b.project.filter(|p| !p.is_empty()) {
        Some(_) if s.id == CLOUD => return Err(bad("kiqr.cloud links the whole KIQR agent, not one project")),
        Some(slug) => {
            kiqr_core::project::Project::load(&app.store, &slug)?;
            Place::Project(slug)
        }
        None => Place::Machine,
    };
    let origin = url::Url::parse(b.origin.trim()).map_err(|_| bad("origin is not a URL"))?;
    if !matches!(origin.scheme(), "http" | "https") {
        return Err(bad("origin must be http or https"));
    }
    let srv = server(&app, s).await.map_err(|e| ApiError(StatusCode::BAD_GATEWAY, format!("{e:#}")))?;
    let cid = client_id(&app, s, &srv).await.map_err(|e| ApiError(StatusCode::BAD_GATEWAY, format!("{e:#}")))?;
    let base = origin.origin().ascii_serialization();
    let on_this_computer = origin.host_str().is_some_and(loopback);

    // Elsewhere, a service with a device flow shows a code instead.
    if !on_this_computer && srv.has_device_flow() {
        let d = oauth::device_start(&srv, &cid, s.scope).await.map_err(|e| ApiError(StatusCode::BAD_GATEWAY, format!("{e:#}")))?;
        let view = DeviceView { user_code: d.user_code.clone(), verification_uri: d.verification_uri.clone(), verification_uri_complete: d.verification_uri_complete.clone() };
        app.signins.devices.lock().await.insert(s.id, view.clone());
        tokio::spawn(poll_device(app.clone(), s, srv, cid, d, place));
        return Ok(Json(json!({ "mode": "device", "user_code": view.user_code, "verification_uri": view.verification_uri, "verification_uri_complete": view.verification_uri_complete })));
    }

    let verifier = kiqr_core::connect::random_token();
    let (state, redirect_uri) = if on_this_computer {
        (kiqr_core::connect::random_token(), format!("{base}{CALLBACK_PATH}"))
    } else {
        // Not on this computer and no device flow: through the kiqr.cloud relay,
        // which the service calls back over https. The relay id is the state.
        let (relay_id, _) = open_relay(&app, &format!("{base}{CALLBACK_PATH}")).await.map_err(|e| {
            bad(format!("{} only signs in from KIQR on this computer, or through kiqr.cloud: sign in to kiqr.cloud first, or open KIQR at http://localhost ({e})", s.name))
        })?;
        (relay_id, format!("{}/relay", cloud_url(&app).await))
    };
    let url = oauth::authorize_url(&srv, &cid, &redirect_uri, s.scope, &state, &verifier).map_err(|e| bad(e.to_string()))?;
    {
        let mut p = app.signins.pending.lock().await;
        p.retain(|_, f| f.created.elapsed() < PENDING_FOR);
        p.insert(state, Pending { id: s.id, place, server: srv, client_id: cid, redirect_uri, verifier, created: Instant::now() });
    }
    Ok(Json(json!({ "mode": "redirect", "url": url })))
}

async fn poll_device(app: App, s: &'static SignIn, srv: Server, cid: String, d: oauth::DeviceCode, place: Place) {
    let mut interval = d.interval.unwrap_or(5).max(1);
    let deadline = Instant::now() + Duration::from_secs(d.expires_in.clamp(60, 1800));
    let outcome = loop {
        if Instant::now() > deadline {
            break Err(anyhow!("the code expired before it was approved"));
        }
        tokio::time::sleep(Duration::from_secs(interval)).await;
        match oauth::device_poll(&srv, &cid, &d.device_code, now()).await {
            Ok(Poll::Done(t)) => break Ok(t),
            Ok(Poll::Pending) => {}
            Ok(Poll::SlowDown) => interval += 5,
            Ok(Poll::Failed(why)) => break Err(anyhow!(why)),
            Err(e) => tracing::warn!(signin = s.id, error = %e, "device poll failed; trying again"),
        }
    };
    app.signins.devices.lock().await.remove(s.id);
    let result = match outcome {
        Ok(t) => finish(&app, s, t, &place).await,
        Err(e) => Err(e),
    };
    if let Err(e) = result {
        app.bus.emit(None, EventKind::SignInChanged { id: s.id.into(), signed_in: false, error: Some(format!("{e:#}")) });
    }
}

async fn callback(State(app): State<App>, Query(q): Query<BTreeMap<String, String>>) -> (StatusCode, Html<String>) {
    let Some(state) = q.get("state") else {
        return page(StatusCode::BAD_REQUEST, "This link is not from a sign-in KIQR started.");
    };
    let Some(p) = app.signins.pending.lock().await.remove(state).filter(|p| p.created.elapsed() < PENDING_FOR) else {
        return page(StatusCode::BAD_REQUEST, "This sign-in has expired. Start it again from KIQR.");
    };
    let s = SIGNINS.iter().find(|s| s.id == p.id).expect("known sign-in");
    let result: Result<()> = async {
        if let Some(e) = q.get("error") {
            bail!("{}", q.get("error_description").cloned().unwrap_or_else(|| e.clone()));
        }
        let code = q.get("code").ok_or_else(|| anyhow!("no code came back"))?;
        let t = oauth::exchange_code(&p.server, &p.client_id, code, &p.redirect_uri, &p.verifier, now()).await?;
        finish(&app, s, t, &p.place).await
    }
    .await;
    match result {
        Ok(()) => page(StatusCode::OK, &format!("KIQR is signed in to {}. You can close this tab.", s.name)),
        Err(e) => {
            app.bus.emit(None, EventKind::SignInChanged { id: s.id.into(), signed_in: false, error: Some(format!("{e:#}")) });
            page(StatusCode::BAD_REQUEST, &format!("KIQR is not signed in to {}: {e:#}", s.name))
        }
    }
}

/// `?project=` signs that project's own sign-in out; none, the machine's.
async fn sign_out(State(app): State<App>, Path(id): Path<String>, Query(q): Query<ProjectQuery>) -> ApiResult<Value> {
    let s = find(&id)?;
    let place = q.project.filter(|p| !p.is_empty()).map(Place::Project).unwrap_or(Place::Machine);
    if let (Ok(srv), Some(cid)) = (server(&app, s).await, get_env(s, "CLIENT_ID")) {
        for what in ["REFRESH_TOKEN", "TOKEN"] {
            if let Some(t) = read(app.store.home(), s, what, &place) {
                oauth::revoke(&srv, &cid, &t).await;
            }
        }
    }
    clear(&app, s, &place).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    app.bus.emit(None, EventKind::SignInChanged { id: s.id.into(), signed_in: false, error: None });
    Ok(Json(json!({ "id": s.id, "signed_in": false })))
}

fn page(status: StatusCode, msg: &str) -> (StatusCode, Html<String>) {
    let msg = msg.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    (
        status,
        Html(format!(
            r#"<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>KIQR</title>
<style>body{{margin:0;min-height:100vh;display:grid;place-items:center;background:#070a12;color:#e6ecf7;font:15px/1.5 Inter,system-ui,sans-serif}}p{{max-width:28rem;padding:0 16px;text-align:center}}</style>
<p>{msg}</p>"#
        )),
    )
}
