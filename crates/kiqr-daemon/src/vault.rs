//! The Vault over HTTP: credentials for the whole machine (`kiqr_core::vault`),
//! their domains' favicons, and "Sign in with …" for the providers kiqr.cloud
//! offers, which makes a signed-in app into a credential. Values go in and
//! never come out: every answer is a `CredentialView`.

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use kiqr_core::events::EventKind;
use kiqr_core::vault::{self, Draft};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/vault", get(list).post(create))
        .route("/api/vault/{id}", get(one).put(update).delete(remove))
        .route("/api/vault/connect/{provider}", post(connect_start))
        .route("/api/connect/providers", get(providers))
        .route("/api/favicon/{domain}", get(favicon))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}
fn locked(e: impl std::fmt::Display) -> ApiError {
    ApiError(StatusCode::SERVICE_UNAVAILABLE, e.to_string())
}

async fn list(State(app): State<App>) -> ApiResult<Value> {
    let status = app.vault.status();
    let credentials = if status.unlocked { app.vault.list().map_err(locked)? } else { vec![] };
    Ok(Json(json!({ "status": status, "credentials": credentials, "templates": vault::templates() })))
}

async fn one(State(app): State<App>, Path(id): Path<String>) -> ApiResult<Value> {
    let c = app.vault.view(&id).map_err(locked)?.ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no credential {id}")))?;
    Ok(Json(json!(c)))
}

async fn create(State(app): State<App>, Json(d): Json<Draft>) -> ApiResult<Value> {
    if d.domains.as_ref().is_none_or(|d| d.iter().all(|x| x.trim().is_empty())) {
        return Err(bad("a credential needs at least one domain: where it is used"));
    }
    let c = app.vault.create(d).map_err(|e| bad(format!("{e:#}")))?;
    saved(&app, &c);
    Ok(Json(json!(c)))
}

async fn update(State(app): State<App>, Path(id): Path<String>, Json(d): Json<Draft>) -> ApiResult<Value> {
    if d.domains.as_ref().is_some_and(|d| d.iter().all(|x| x.trim().is_empty())) {
        return Err(bad("a credential keeps at least one domain"));
    }
    let c = app.vault.update(&id, d).map_err(|e| bad(format!("{e:#}")))?;
    saved(&app, &c);
    Ok(Json(json!(c)))
}

async fn remove(State(app): State<App>, Path(id): Path<String>) -> ApiResult<Value> {
    if !app.vault.delete(&id).map_err(locked)? {
        return Err(ApiError(StatusCode::NOT_FOUND, format!("no credential {id}")));
    }
    let _ = std::fs::remove_dir_all(app.store.home().credential_session_dir(&id));
    crate::access::forget(&app, &id).await;
    app.bus.emit(None, EventKind::CredentialDeleted { id });
    Ok(Json(json!({ "ok": true })))
}

pub fn saved(app: &App, c: &vault::CredentialView) {
    app.bus.emit(None, EventKind::CredentialSaved { id: c.id.clone(), label: c.label.clone(), fields: c.fields.iter().filter(|f| f.set).map(|f| f.name.clone()).collect() });
}

// ------------------------------------------------------- kiqr.cloud sign-ins

/// The providers kiqr.cloud signs in to, with their domains and the access
/// each offers, and whether this KIQR is signed in to kiqr.cloud (the sign-in
/// goes through it). The last good list is kept for when kiqr.cloud is away.
async fn providers(State(app): State<App>) -> ApiResult<Value> {
    let list = provider_list(&app).await;
    let signed_in = crate::signins::token(&app, crate::signins::CLOUD).await.is_some();
    Ok(Json(json!({ "providers": list, "cloud_signed_in": signed_in })))
}

pub async fn provider_list(app: &App) -> Vec<Value> {
    let cache = app.store.home().root().join("tmp").join("providers.json");
    let fresh = std::fs::metadata(&cache).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age < Duration::from_secs(3600));
    let cached = || std::fs::read_to_string(&cache).ok().and_then(|t| serde_json::from_str::<Vec<Value>>(&t).ok()).unwrap_or_default();
    if fresh {
        return cached();
    }
    let url = format!("{}/api/v1/connect/providers", app.config.read().await.cloud.url.trim_end_matches('/'));
    let got = async {
        let resp = reqwest::Client::builder().user_agent(kiqr_core::fetch::USER_AGENT).timeout(Duration::from_secs(8)).build()?.get(&url).send().await?.error_for_status()?;
        let v: Value = resp.json().await?;
        anyhow::Ok(v["providers"].as_array().cloned().unwrap_or_default())
    }
    .await;
    match got {
        Ok(list) => {
            if let Some(dir) = cache.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&cache, serde_json::to_string(&list).unwrap_or_default());
            list
        }
        Err(e) => {
            tracing::info!(error = %e, "kiqr.cloud's providers could not be read; using the last list");
            cached()
        }
    }
}

#[derive(Deserialize)]
struct ConnectBody {
    /// The UI's origin; the callback is under it.
    origin: String,
    /// The access to ask for; empty: everything the provider offers.
    #[serde(default)]
    scopes: Vec<String>,
    /// A credential to sign in again (more access, a new token).
    #[serde(default)]
    credential: Option<String>,
    #[serde(default)]
    label: String,
    /// Grants to make when the credential comes back: `{project, ability}`.
    #[serde(default)]
    grant: Vec<crate::connect::GrantTo>,
}

async fn connect_start(State(app): State<App>, Path(provider): Path<String>, Json(b): Json<ConnectBody>) -> ApiResult<Value> {
    let list = provider_list(&app).await;
    let p = list.iter().find(|p| p["id"] == provider.as_str()).ok_or_else(|| bad(format!("kiqr.cloud does not sign in to {provider}")))?;
    let offered: Vec<String> = p["scopes"].as_array().into_iter().flatten().filter_map(|s| s["scope"].as_str().map(str::to_string)).collect();
    let domains: Vec<String> = p["domains"].as_array().into_iter().flatten().filter_map(|d| d.as_str().map(str::to_string)).collect();
    let mut scopes: Vec<String> = if b.scopes.is_empty() { offered.clone() } else { b.scopes.clone() };
    // Signing in again keeps what it had, so one ability's access never drops another's.
    if let Some(id) = b.credential.as_deref() {
        let c = app.vault.get(id).map_err(locked)?.ok_or_else(|| bad(format!("no credential {id}")))?;
        for s in c.scopes() {
            if !scopes.contains(&s) {
                scopes.push(s);
            }
        }
    }
    if let Some(extra) = scopes.iter().find(|s| !offered.contains(s)) {
        return Err(bad(format!("kiqr.cloud does not offer {extra} for {provider}")));
    }
    let name = p["name"].as_str().unwrap_or(&provider).to_string();
    let target = crate::connect::VaultSignIn { provider: provider.clone(), name, domains, scopes, credential: b.credential, label: b.label, grant: b.grant };
    let started = crate::connect::start_vault(&app, &b.origin, target).await.map_err(|e| bad(format!("{e:#}")))?;
    Ok(Json(started))
}

// ------------------------------------------------------------------ favicons

/// The favicon of a credential's domain, fetched once and kept a week; a
/// domain with none answers 404 and the UI draws its letter.
async fn favicon(State(app): State<App>, Path(domain): Path<String>) -> Response {
    let Some(host) = vault::normalise_host(&domain).filter(|h| h.contains('.') && h.parse::<std::net::IpAddr>().is_err() && !h.starts_with('[') && h.len() <= 253) else {
        return status(StatusCode::BAD_REQUEST);
    };
    let dir = app.store.home().favicons_dir();
    let file = dir.join(&host);
    let week = Duration::from_secs(7 * 24 * 3600);
    let fresh = std::fs::metadata(&file).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age < week);
    let bytes = if fresh {
        std::fs::read(&file).unwrap_or_default()
    } else {
        let got = fetch_icon(&host).await.unwrap_or_default();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(&file, &got);
        got
    };
    let Some(mime) = image_type(&bytes) else { return status(StatusCode::NOT_FOUND) };
    Response::builder().header(header::CONTENT_TYPE, mime).header(header::CACHE_CONTROL, "max-age=86400")
        // A site's SVG could carry script: shown as an image only, never run as a page here.
        .header(header::CONTENT_SECURITY_POLICY, "default-src 'none'; style-src 'unsafe-inline'; sandbox")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(bytes)).unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
}

fn status(s: StatusCode) -> Response {
    Response::builder().status(s).body(Body::empty()).unwrap_or_default()
}

/// The site's declared icon (`<link rel="icon">`, the largest given), else `/favicon.ico`.
async fn fetch_icon(host: &str) -> Option<Vec<u8>> {
    let client = reqwest::Client::builder().user_agent(kiqr_core::fetch::USER_AGENT).timeout(Duration::from_secs(8)).build().ok()?;
    let base = url::Url::parse(&format!("https://{host}/")).ok()?;
    let mut tries: Vec<url::Url> = vec![];
    if let Ok(resp) = client.get(base.clone()).send().await {
        let page = resp.url().clone();
        if let Ok(html) = resp.text().await {
            tries.extend(vault::icon_links(&html, &page));
        }
    }
    tries.push(base.join("/favicon.ico").ok()?);
    for u in tries.into_iter().take(4) {
        if !matches!(u.scheme(), "https" | "http") {
            continue;
        }
        let Ok(resp) = client.get(u).send().await else { continue };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(b) = resp.bytes().await else { continue };
        if b.len() <= 512 * 1024 && image_type(&b).is_some() {
            return Some(b.to_vec());
        }
    }
    None
}

fn image_type(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if b.starts_with(&[0, 0, 1, 0]) {
        Some("image/x-icon")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF8") {
        Some("image/gif")
    } else if b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else if String::from_utf8_lossy(&b[..b.len().min(512)]).to_lowercase().contains("<svg") {
        Some("image/svg+xml")
    } else {
        None
    }
}
