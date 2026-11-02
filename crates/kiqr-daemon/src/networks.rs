//! Networks are kinds (`extensions::Kind`). Everything KIQR knows about one comes from
//! the ability that declares it: the installed abilities a project has on,
//! and, for recognising the business's profiles and suggesting what to
//! install, the abilities listed in the marketplace. Nothing here names a
//! network.

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use kiqr_core::brand::Brand;
use kiqr_core::events::EventKind;
use kiqr_core::extensions::Kind;
use kiqr_core::project::Project;
use serde_json::{json, Value};

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/networks", get(list))
        .route("/api/projects/{slug}/suggestions", get(list_suggestions))
        .route("/api/projects/{slug}/suggestions/{kind}/dismiss", post(dismiss))
}

/// The kinds of the abilities this project has on.
pub async fn installed(app: &App, slug: &str) -> Vec<Kind> {
    crate::channels::kinds(app, slug).await.into_iter().map(|(_, k)| k).collect()
}

/// The networks this project can write posts for: the kinds an ability on
/// here publishes to that are networks (not an inbox or a blog) and that a
/// channel is, once each. With no channels, every network an ability on here
/// publishes to, so the first posts can be written before anything is set up.
pub async fn for_posts(app: &App, slug: &str) -> Vec<Kind> {
    let mut kinds: Vec<String> = Vec::new();
    for ch in kiqr_core::channels::Channel::all(&app.store, slug).unwrap_or_default().into_iter().filter(|e| !e.suggested && !e.is_list()) {
        if let Some((_, k)) = crate::channels::kind_for(app, slug, &ch).await {
            kinds.push(k.kind);
        }
    }
    let mut out: Vec<Kind> = Vec::new();
    for k in installed(app, slug).await.into_iter().filter(|k| k.is_network() && k.publishes()) {
        if !out.iter().any(|o| o.kind == k.kind) && (kinds.is_empty() || kinds.contains(&k.kind)) {
            out.push(k);
        }
    }
    out
}

/// (host, kind) for every place a profile may live: installed abilities
/// first, then whatever the marketplace lists.
pub async fn profile_hosts(app: &App, slug: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut add = |h: &str, k: &str| {
        let h = h.trim().trim_start_matches("www.").to_ascii_lowercase();
        if !h.is_empty() && !out.iter().any(|(x, _)| *x == h) {
            out.push((h, k.to_string()));
        }
    };
    // A service's profile hosts belong to the kinds its abilities declare.
    for (m, k) in crate::channels::kinds(app, slug).await {
        for h in &m.service.profile_hosts {
            add(h, &k.kind);
        }
    }
    for item in crate::market::catalogue(app).await {
        for c in &item.kinds {
            for h in &c.profile_hosts {
                add(h, &c.kind);
            }
        }
    }
    out
}

/// Every network the project knows of: installed ones in full (one entry per
/// kind, with what the abilities that declare it do there), listed ones by name.
pub async fn view(app: &App, slug: &str) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let on = crate::channels::kinds(app, slug).await;
    for (_, k) in &on {
        if seen.contains(&k.kind) {
            continue;
        }
        seen.push(k.kind.clone());
        let same: Vec<&(kiqr_core::extensions::Manifest, Kind)> = on.iter().filter(|(_, o)| o.kind == k.kind).collect();
        out.push(json!({ "kind": k.kind, "name": k.display_name(), "label": k.label, "icon": k.icon, "section": k.section(), "network": k.is_network(), "text_limit": k.text_limit, "image": k.image, "preview": k.preview, "installed": true, "publishes": same.iter().any(|(_, o)| o.publishes()), "abilities": same.iter().map(|(m, o)| json!({ "id": m.id, "name": m.name, "service": m.service(), "publishes": o.publishes() })).collect::<Vec<_>>() }));
    }
    for item in crate::market::catalogue(app).await {
        for c in &item.kinds {
            if seen.contains(&c.kind) {
                continue;
            }
            seen.push(c.kind.clone());
            out.push(json!({ "kind": c.kind, "name": c.display_name(), "icon": c.icon, "network": !c.profile_hosts.is_empty(), "installed": false, "ability": item.id }));
        }
    }
    out
}

/// Profiles the brand found on a network no ability here declares, where an
/// ability in the marketplace does: worth offering to install.
pub async fn suggestions(app: &App, slug: &str) -> Vec<Value> {
    let Ok(Some(brand)) = Brand::load(&app.store, slug) else { return vec![] };
    let dismissed = Project::load(&app.store, slug).map(|p| p.suggestions_dismissed).unwrap_or_default();
    let have = installed(app, slug).await;
    let catalogue = crate::market::catalogue(app).await;
    let reg = app.registry.read().await;
    let mut out = Vec::new();
    for s in &brand.social {
        let kind = s.network.as_str();
        if have.iter().any(|k| k.kind == kind) || dismissed.iter().any(|d| d == kind) || out.iter().any(|o: &Value| o["kind"] == kind) {
            continue;
        }
        let Some((item, c)) = catalogue.iter().filter(|i| reg.installed(&i.id).is_empty()).find_map(|i| i.kinds.iter().find(|c| c.kind == kind).map(|c| (i, c))) else { continue };
        out.push(json!({ "kind": kind, "name": c.display_name(), "profile_url": s.url, "ability": item.id, "ability_name": item.name }));
    }
    out
}

/// Seemo's line after a crawl, when there is something to suggest.
pub async fn suggestion_line(app: &App, slug: &str) -> Option<String> {
    let s = suggestions(app, slug).await;
    let parts: Vec<String> = s.iter().map(|v| format!("your {} page (the marketplace has \"{}\")", v["name"].as_str().unwrap_or(""), v["ability_name"].as_str().unwrap_or(""))).collect();
    match parts.len() {
        0 => None,
        1 => Some(format!("I found {}. Install it under Setup → Abilities and I can write and publish there; it is also on To do.", parts[0])),
        _ => Some(format!("I found {}. Install any of them under Setup → Abilities and I can write and publish there; they are also on To do.", join_and(&parts))),
    }
}

fn join_and(parts: &[String]) -> String {
    match parts.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        Some((last, _)) => last.clone(),
        None => String::new(),
    }
}

async fn list(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    Ok(Json(json!({ "networks": view(&app, &slug).await })))
}

async fn list_suggestions(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    Ok(Json(json!({ "suggestions": suggestions(&app, &slug).await })))
}

async fn dismiss(State(app): State<App>, Path((slug, kind)): Path<(String, String)>) -> ApiResult<Value> {
    let mut p = Project::load(&app.store, &slug)?;
    if !kiqr_core::tools::valid_ext_id(&kind) {
        return Err(ApiError(StatusCode::BAD_REQUEST, "not a channel kind".into()));
    }
    if !p.suggestions_dismissed.contains(&kind) {
        p.suggestions_dismissed.push(kind.clone());
        p.save(&app.store)?;
        app.store.commit(&format!("project({slug}): stop suggesting an ability for {kind}")).await?;
    }
    app.bus.emit(Some(&slug), EventKind::SuggestionsChanged {});
    Ok(Json(json!({ "dismissed": kind })))
}
