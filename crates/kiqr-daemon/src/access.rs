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
        .route("/api/projects/{slug}/recommendations", get(recommendations))
}

/// `GET …/recommendations?credential=<id>`: for each credential in the Vault
/// (or the one named), the abilities that could use it, matched by host:
/// `allowed` here already, `installed` but not allowed, or `available` in
/// the marketplace. A signed-in app missing some access an ability asks for
/// says so (`more_access`): signing in again adds it.
async fn recommendations(State(app): State<App>, Path(slug): Path<String>, axum::extract::Query(q): axum::extract::Query<std::collections::BTreeMap<String, String>>) -> ApiResult<Value> {
    let _ = kiqr_core::project::Project::load(&app.store, &slug)?;
    let access = Access::load(&app.store, &slug).unwrap_or_default();
    let creds: Vec<kiqr_core::vault::Credential> = app.vault.all().unwrap_or_default().into_iter().filter(|c| q.get("credential").is_none_or(|id| *id == c.id)).collect();
    let installed: Vec<kiqr_core::extensions::Manifest> = {
        let reg = app.registry.read().await;
        reg.visible(&slug).into_iter().filter(|e| e.error.is_none()).filter_map(|e| e.manifest.clone()).filter(|m| !m.credentials.is_empty()).collect()
    };
    let here: Vec<String> = app.registry.read().await.visible(&slug).into_iter().map(|e| e.id().to_string()).collect();
    let catalogue = crate::market::catalogue(&app).await;
    let none = |_: &str| None;
    let mut out = Vec::new();
    for c in &creds {
        let mut abilities = Vec::new();
        // Fits but for access a sign-in did not give: still worth naming, with what is missing.
        let judge = |needs: &[kiqr_core::extensions::CredentialNeed]| -> Option<(String, String, bool)> {
            needs.iter().find_map(|n| {
                let loose = kiqr_core::extensions::CredentialNeed { scopes: vec![], ..n.clone() };
                loose.fits(c, &none).then(|| (n.describe(), n.for_.clone(), !n.fits(c, &none)))
            })
        };
        for m in &installed {
            if let Some((need, why, more)) = judge(&m.needs()) {
                let status = if access.allows(&m.id, &c.id) { "allowed" } else { "installed" };
                abilities.push(json!({ "id": m.id, "name": m.name, "icon": m.icon, "summary": m.summary, "status": status, "need": need, "for": why, "more_access": more }));
            }
        }
        for item in catalogue.iter().filter(|i| !installed.iter().any(|m| m.id == i.id) && !i.credentials.is_empty()) {
            let needs: Vec<kiqr_core::extensions::CredentialNeed> = item.credentials.iter().map(|n| if n.hosts.is_empty() { kiqr_core::extensions::CredentialNeed { hosts: item.hosts.clone(), ..n.clone() } } else { n.clone() }).collect();
            if here.contains(&item.id) {
                continue;
            }
            if let Some((need, why, more)) = judge(&needs) {
                abilities.push(json!({ "id": item.id, "name": item.name, "icon": item.icon, "summary": item.summary, "status": "available", "need": need, "for": why, "more_access": more }));
            }
        }
        if !abilities.is_empty() || q.contains_key("credential") {
            out.push(json!({ "credential": c.id, "label": c.label, "domain": c.default_domain(), "provider": c.provider, "abilities": abilities }));
        }
    }
    Ok(Json(json!({ "recommendations": out })))
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
