//! One-click sign-ins that make credentials. "Sign in with …" from the Vault:
//! kiqr.cloud holds the provider's app, builds the consent page and relays
//! the provider back to this KIQR's callback. And an ability's own `connect`
//! on a credential need ("Connect with WordPress"): the service's page hands
//! back the values (a redirect, or an oauth2 code exchanged here). Either
//! way the values become a credential in the Vault (new, or the one signed
//! in again), the grants are made, and settings the flow returns (the site)
//! go to the project. Renewing a token is `creds::renew`.

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
use kiqr_core::events::EventKind;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

/// A sign-in waiting for its callback.
pub struct Pending {
    connect: Connect,
    verifier: String,
    created: Instant,
    callback: String,
    target: Target,
}

enum Target {
    Vault(VaultSignIn),
    Need(NeedSignIn),
}

/// An ability's own one-click sign-in for one of its needs, in a project.
#[derive(Clone, Debug)]
pub struct NeedSignIn {
    project: String,
    ability: String,
    need: String,
    /// The settings typed first, and the website's address (`SITE_URL`), by name in capitals.
    values: BTreeMap<String, String>,
    /// The website channel it was started from: what comes back becomes its publisher.
    channel: Option<String>,
}

/// A "Sign in with …" started from the Vault, through kiqr.cloud.
#[derive(Clone, Debug)]
pub struct VaultSignIn {
    pub provider: String,
    pub name: String,
    pub domains: Vec<String>,
    pub scopes: Vec<String>,
    /// The credential signed in again; none makes a new one.
    pub credential: Option<String>,
    pub label: String,
    pub grant: Vec<GrantTo>,
}

/// A grant to make once the credential exists: this ability, in this project, may use it.
#[derive(Clone, Debug, Deserialize)]
pub struct GrantTo {
    pub project: String,
    pub ability: String,
}

/// Sign-ins waiting, by state.
#[derive(Default)]
pub struct Connects {
    pending: tokio::sync::Mutex<HashMap<String, Pending>>,
}

pub type Shared = std::sync::Arc<Connects>;

const PENDING_FOR: Duration = Duration::from_secs(600);

pub fn routes() -> Router<App> {
    Router::new().route("/api/connect/callback", get(callback)).route("/api/projects/{slug}/abilities/{ability}/connect", post(start_need))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

#[derive(Deserialize)]
struct NeedBody {
    /// The UI's origin (`window.location.origin`); the callback is under it.
    origin: String,
    /// Which need (`main` when left out).
    #[serde(default)]
    need: String,
    /// The settings the sign-in asks for first, by name (`site_url` or `SITE_URL`).
    #[serde(default)]
    values: BTreeMap<String, String>,
    /// A website channel: its address is `SITE_URL`, and the credential becomes its publisher.
    #[serde(default)]
    channel: String,
}

/// `POST …/abilities/{ability}/connect {origin, values}`: start an ability's
/// one-click sign-in ("Connect with WordPress"). Answers the page to open;
/// `callback: false` means the service shows the values and the user types
/// them into the Vault's form instead (`fields`).
async fn start_need(State(app): State<App>, Path((slug, ability)): Path<(String, String)>, Json(b): Json<NeedBody>) -> ApiResult<Value> {
    let m = crate::creds::manifest(&app, &slug, &ability).await.ok_or_else(|| bad(format!("{ability} is not installed for this project")))?;
    let need = m.needs().into_iter().find(|n| n.connect.is_some() && (b.need.is_empty() || n.id == b.need)).ok_or_else(|| bad(format!("{} has no one-click sign-in", m.name)))?;
    let c = need.connect.clone().expect("found by it");
    let origin = url::Url::parse(b.origin.trim()).map_err(|_| bad("origin is not a URL"))?;
    if !matches!(origin.scheme(), "http" | "https") {
        return Err(bad("origin must be http or https"));
    }
    // What it asks first: typed now, else the project's settings.
    let mut values: BTreeMap<String, String> = crate::creds::settings_of(&app, &slug, &m.id).into_iter().map(|(k, v)| (k.to_ascii_uppercase(), v)).collect();
    for (k, v) in &b.values {
        let v = v.trim().trim_end_matches('/').to_string();
        if !v.is_empty() {
            values.insert(k.to_ascii_uppercase(), v);
        }
    }
    let channel = match b.channel.trim() {
        "" => None,
        id => {
            let ch = kiqr_core::channels::Channel::load(&app.store, &slug, id).map_err(|e| bad(e.to_string()))?;
            if ch.is_website() {
                values.insert(kiqr_core::conn::SITE_URL.into(), crate::channels::site_url(&ch));
            }
            Some(ch.id)
        }
    };
    if let Some(n) = c.needs.iter().find(|n| !values.contains_key(*n)) {
        if n == kiqr_core::conn::SITE_URL {
            return Err(bad(format!("{} connects a website: start it from the website's channel, under Marketing → Channels", m.name)));
        }
        let label = m.settings.get(&n.to_ascii_lowercase()).and_then(|s| s["label"].as_str()).unwrap_or(n).to_lowercase();
        return Err(bad(format!("give the {label} first")));
    }
    let callback_url = format!("{}/api/connect/callback", origin.origin().ascii_serialization());
    // A service that only calls back over https, and KIQR on plain http: a single-use relay on kiqr.cloud, else the user types the values.
    let relayed = if matches!(c.callback, Callback::Https) && origin.scheme() != "https" {
        crate::signins::open_relay(&app, &callback_url).await.map(|(_, link)| link).map_err(|e| tracing::info!(error = %e, "no kiqr.cloud relay; the user types the values")).ok()
    } else {
        None
    };
    let offered = match c.callback {
        Callback::Https if origin.scheme() != "https" => relayed.as_deref(),
        _ => Some(callback_url.as_str()),
    };
    let state = connect::random_token();
    let verifier = connect::random_token();
    let env = { let v = values.clone(); move |k: &str| v.get(k).cloned() };
    let app_id = uuid::Uuid::new_v4().to_string();
    let url = connect::start_url(&c, &env, &app_id, &state, offered, &verifier).map_err(|e| bad(e.to_string()))?;
    let with_callback = offered.is_some();
    if with_callback {
        let mut p = app.connects.pending.lock().await;
        p.retain(|_, f| f.created.elapsed() < PENDING_FOR);
        p.insert(state, Pending { connect: c.clone(), verifier, created: Instant::now(), callback: callback_url.clone(), target: Target::Need(NeedSignIn { project: slug.clone(), ability: m.id.clone(), need: need.id.clone(), values, channel }) });
    }
    // Without a callback the user types what the page shows: the fields of the credential to add.
    let fields: Vec<&String> = c.returns.keys().filter_map(|k| need.env.get(k)).collect();
    Ok(Json(json!({ "url": url, "callback": with_callback, "relayed": relayed.is_some(), "fields": fields, "template": need.template.first() })))
}

/// The values an ability's own sign-in came back with become a credential,
/// granted to the ability; the settings it returned (the site) go to the project.
async fn finish_need(app: &App, f: &Pending, n: &NeedSignIn, q: &BTreeMap<String, String>) -> Result<String> {
    let m = crate::creds::manifest(app, &n.project, &n.ability).await.ok_or_else(|| anyhow!("{} is no longer installed", n.ability))?;
    let need = m.needs().into_iter().find(|x| x.id == n.need).ok_or_else(|| anyhow!("{} no longer asks for this", m.name))?;
    let values = match f.connect.flow {
        Flow::Redirect => connect::read_redirect(&f.connect, q)?,
        Flow::Oauth2 => {
            let code = connect::read_code(&f.connect, q)?;
            let env = { let v = n.values.clone(); move |k: &str| v.get(k).cloned() };
            let form = connect::code_form(&f.connect, &env, &code, &f.callback, &f.verifier);
            token_request(&f.connect, &env, &form).await?
        }
        _ => bail!("this sign-in has no callback"),
    };
    let setting_names: Vec<String> = m.settings.keys().cloned().collect();
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut settings: BTreeMap<String, String> = BTreeMap::new();
    // The website it signed in on: what came back, else the channel's address it started from.
    let mut site_url = n.values.get(kiqr_core::conn::SITE_URL).cloned();
    for (name, v) in values {
        if name == kiqr_core::conn::SITE_URL {
            site_url = Some(v.trim_end_matches('/').to_string());
        } else if let Some(field) = need.env.get(&name) {
            fields.insert(field.split('|').next().unwrap_or(field).trim().to_string(), v);
        } else if let Some(s) = setting_names.iter().find(|s| s.to_ascii_uppercase() == name) {
            settings.insert(s.clone(), v.trim_end_matches('/').to_string());
        }
    }
    for (k, v) in &n.values {
        if let Some(s) = setting_names.iter().find(|s| s.to_ascii_uppercase() == *k) {
            settings.entry(s.clone()).or_insert_with(|| v.clone());
        }
    }
    // Its domain: the site it signed in on (a setting that is an address), else the need's hosts.
    let site = site_url.iter().chain(settings.values()).find_map(|v| url::Url::parse(v).ok().and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string())));
    let domains: Vec<String> = site.into_iter().chain(need.hosts.iter().filter(|h| !h.contains('$')).cloned()).collect();
    if domains.is_empty() {
        bail!("{} did not say which site this is for", m.name);
    }
    let label = fields.get("username").map(|u| format!("{u} on {}", domains[0])).unwrap_or_else(|| domains[0].clone());
    let view = app.vault.create(kiqr_core::vault::Draft { label: Some(label), template: need.template.first().cloned(), domains: Some(domains), values: fields, ..Default::default() })?;
    crate::vault::saved(app, &view);
    if !settings.is_empty() {
        let mut p = kiqr_core::project::Project::load(&app.store, &n.project)?;
        let s = p.ability_settings.entry(m.id.clone()).or_default();
        for (k, v) in settings {
            s.values.insert(k, Value::String(v));
        }
        p.save(&app.store)?;
        app.store.commit(&format!("project({}): {} settings from its sign-in", n.project, m.name)).await?;
        app.bus.emit(Some(&n.project), EventKind::AbilitySettingsSaved { ability: m.id.clone(), credential: view.id.clone() });
    }
    crate::access::grant(app, &n.project, &m.id, &view.id, "connect").await?;
    // Started from a website channel: it publishes as the new credential.
    if let Some(id) = &n.channel {
        if let Ok(mut ch) = kiqr_core::channels::Channel::load(&app.store, &n.project, id) {
            ch.publisher = view.id.clone();
            ch.save(&app.store, &n.project)?;
            app.store.commit(&format!("channels({}): {} publishes as {}", n.project, ch.label, view.label)).await?;
            crate::channels::emit(app, &n.project, &ch, None);
        }
    }
    Ok(view.id)
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

async fn callback(State(app): State<App>, Query(q): Query<BTreeMap<String, String>>) -> (StatusCode, Html<String>) {
    let Some(state) = q.get("state").cloned() else {
        return page(StatusCode::BAD_REQUEST, "This link is not from a sign-in KIQR started.");
    };
    let Some(f) = app.connects.pending.lock().await.remove(&state).filter(|f| f.created.elapsed() < PENDING_FOR) else {
        return page(StatusCode::BAD_REQUEST, "This sign-in has expired. Start it again from the Vault.");
    };
    let v = match &f.target {
        Target::Vault(v) => v.clone(),
        Target::Need(n) => {
            let name = crate::creds::manifest(&app, &n.project, &n.ability).await.map(|m| m.name).unwrap_or_else(|| n.ability.clone());
            return match finish_need(&app, &f, n, &q).await {
                Ok(id) => {
                    app.bus.emit(Some(&n.project), EventKind::CredentialConnected { provider: n.ability.clone(), ok: true, error: None, credential: Some(id) });
                    page(StatusCode::OK, &format!("{name} is connected. You can close this tab."))
                }
                Err(e) => {
                    app.bus.emit(Some(&n.project), EventKind::CredentialConnected { provider: n.ability.clone(), ok: false, error: Some(format!("{e:#}")), credential: None });
                    page(StatusCode::BAD_REQUEST, &format!("{name} is not connected: {e}"))
                }
            };
        }
    };
    match finish_vault(&app, &f, &v, &q).await {
        Ok(id) => {
            app.bus.emit(None, EventKind::CredentialConnected { provider: v.provider.clone(), ok: true, error: None, credential: Some(id) });
            page(StatusCode::OK, &format!("You are signed in with {}. You can close this tab.", v.name))
        }
        Err(e) => {
            app.bus.emit(None, EventKind::CredentialConnected { provider: v.provider.clone(), ok: false, error: Some(format!("{e:#}")), credential: v.credential.clone() });
            page(StatusCode::BAD_REQUEST, &format!("The sign-in with {} did not finish: {e}", v.name))
        }
    }
}

/// Start a "Sign in with …" from the Vault: kiqr.cloud builds the consent
/// page, the provider comes back through its relay to this KIQR's callback.
pub async fn start_vault(app: &App, origin: &str, v: VaultSignIn) -> Result<Value> {
    let origin = url::Url::parse(origin.trim()).map_err(|_| anyhow!("origin is not a URL"))?;
    if !matches!(origin.scheme(), "http" | "https") {
        bail!("origin must be http or https");
    }
    for g in &v.grant {
        kiqr_core::project::Project::load(&app.store, &g.project).map_err(|_| anyhow!("no project {}", g.project))?;
    }
    let callback_url = format!("{}/api/connect/callback", origin.origin().ascii_serialization());
    let verifier = connect::random_token();
    let started = crate::signins::cloud_connect(app, &v.provider, "start", json!({ "return_to": callback_url, "scopes": v.scopes, "code_challenge": connect::pkce_challenge(&verifier) })).await?;
    let (Some(url), Some(state)) = (started["url"].as_str(), started["state"].as_str()) else { bail!("kiqr.cloud did not say where to sign in") };
    let mut p = app.connects.pending.lock().await;
    p.retain(|_, f| f.created.elapsed() < PENDING_FOR);
    let c = Connect { flow: Flow::Cloud, provider: v.provider.clone(), scopes: v.scopes.clone(), ..Default::default() };
    p.insert(state.to_string(), Pending { connect: c, verifier, created: Instant::now(), callback: callback_url, target: Target::Vault(v) });
    Ok(json!({ "url": url, "callback": true, "relayed": true }))
}

/// The tokens the provider gave become a credential (new, or the one signed
/// in again), then the grants asked for are made. Returns the credential's id.
async fn finish_vault(app: &App, f: &Pending, v: &VaultSignIn, q: &BTreeMap<String, String>) -> Result<String> {
    let code = connect::read_code(&f.connect, q)?;
    let body = crate::signins::cloud_connect(app, &v.provider, "token", json!({ "code": code, "code_verifier": f.verifier })).await?;
    let token = body["access_token"].as_str().filter(|t| !t.is_empty()).ok_or_else(|| anyhow!("{} gave no token", v.name))?;
    let mut values: Vec<(String, String)> = vec![("token".into(), token.to_string())];
    if let Some(r) = body["refresh_token"].as_str().filter(|r| !r.is_empty()) {
        values.push(("refresh".into(), r.to_string()));
    }
    if let Some(e) = body["expires_in"].as_i64() {
        values.push(("expires_at".into(), (chrono::Utc::now().timestamp() + e).to_string()));
    }
    // Who signed in, when kiqr.cloud could ask the provider: an address, a user name.
    if let Some(a) = body["account"].as_str().filter(|a| !a.trim().is_empty()) {
        values.push(("account".into(), a.trim().to_string()));
    }
    // What the provider says it granted, else what was asked for.
    let granted = body["scope"].as_str().map(|s| s.split([' ', ',']).filter(|s| !s.is_empty()).map(str::to_string).collect::<Vec<_>>()).filter(|s| !s.is_empty()).unwrap_or_else(|| v.scopes.clone());
    values.push(("scopes".into(), granted.join(" ")));
    let id = match v.credential.as_deref() {
        Some(id) => {
            let pairs: Vec<(&str, &str)> = values.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            app.vault.set_values(id, &pairs)?;
            let view = app.vault.view(id)?.ok_or_else(|| anyhow!("the credential is gone"))?;
            crate::vault::saved(app, &view);
            id.to_string()
        }
        None => {
            let view = app.vault.create(kiqr_core::vault::Draft {
                label: Some(if v.label.trim().is_empty() { v.name.clone() } else { v.label.clone() }),
                template: Some("oauth".into()),
                domains: Some(v.domains.clone()),
                values: values.into_iter().collect(),
                provider: Some(v.provider.clone()),
                ..Default::default()
            })?;
            crate::vault::saved(app, &view);
            view.id
        }
    };
    for g in &v.grant {
        crate::access::grant(app, &g.project, &g.ability, &id, "connect").await?;
    }
    Ok(id)
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
