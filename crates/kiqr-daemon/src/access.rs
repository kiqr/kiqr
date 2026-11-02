//! Grants over HTTP: which of the Vault's credentials a project's abilities
//! may use (`kiqr_core::access`). Allowing is always the user's act (or the
//! side effect they were told about: picking a channel's publisher, a
//! "Sign in with …" for named abilities); every change is committed and
//! emitted.

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use kiqr_core::access::Access;
use kiqr_core::events::EventKind;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/access", get(list))
        .route("/api/projects/{slug}/access/grant", post(grant_route).delete(revoke_route))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

async fn list(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    let a = Access::load(&app.store, &slug).map_err(|e| bad(e.to_string()))?;
    Ok(Json(json!(a)))
}

#[derive(Deserialize)]
struct GrantBody {
    ability: String,
    credential: String,
}

async fn grant_route(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<GrantBody>) -> ApiResult<Value> {
    grant(&app, &slug, &b.ability, &b.credential, "user").await.map_err(|e| bad(format!("{e:#}")))?;
    list(State(app), Path(slug)).await
}

async fn revoke_route(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<GrantBody>) -> ApiResult<Value> {
    let mut a = Access::load(&app.store, &slug).map_err(|e| bad(e.to_string()))?;
    if a.revoke(&b.ability, &b.credential) {
        a.save(&app.store, &slug).map_err(|e| bad(e.to_string()))?;
        let label = credential_label(&app, &b.credential);
        let _ = app.store.commit(&format!("access: {} may no longer use {label}", b.ability)).await;
        app.bus.emit(Some(&slug), EventKind::GrantChanged { ability: b.ability.clone(), credential: b.credential.clone(), granted: false, by: "user".into() });
    }
    list(State(app), Path(slug)).await
}

fn credential_label(app: &App, id: &str) -> String {
    app.vault.view(id).ok().flatten().map(|c| c.label).unwrap_or_else(|| id.to_string())
}

/// Allow an ability of a project to use a credential. `by` says whose act it was.
pub async fn grant(app: &App, slug: &str, ability: &str, credential: &str, by: &str) -> anyhow::Result<()> {
    kiqr_core::project::Project::load(&app.store, slug).map_err(|_| anyhow::anyhow!("no project {slug}"))?;
    if app.vault.get(credential)?.is_none() {
        anyhow::bail!("no credential {credential}");
    }
    if app.registry.read().await.entry(Some(slug), ability).is_none() {
        anyhow::bail!("{ability} is not installed");
    }
    let mut a = Access::load(&app.store, slug)?;
    if a.grant(ability, credential) {
        a.save(&app.store, slug)?;
        let label = credential_label(app, credential);
        let _ = app.store.commit(&format!("access: {ability} may use {label}")).await;
        app.bus.emit(Some(slug), EventKind::GrantChanged { ability: ability.into(), credential: credential.into(), granted: true, by: by.into() });
    }
    Ok(())
}

/// A credential left the Vault: no project's ability keeps a grant to it.
pub async fn forget(app: &App, credential: &str) {
    let Ok(projects) = kiqr_core::project::Project::list(&app.store) else { return };
    let mut changed = false;
    for p in projects {
        let Ok(mut a) = Access::load(&app.store, &p.slug) else { continue };
        let abilities = a.forget(credential);
        if abilities.is_empty() || a.save(&app.store, &p.slug).is_err() {
            continue;
        }
        changed = true;
        for ability in abilities {
            app.bus.emit(Some(&p.slug), EventKind::GrantChanged { ability, credential: credential.into(), granted: false, by: "user".into() });
        }
    }
    if changed {
        let _ = app.store.commit("access: a deleted credential is no longer granted").await;
    }
}
