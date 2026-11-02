//! Library → Media: the business's own pictures (video, audio and documents as
//! they come) as `media` assets, and the sources they come from. The website
//! is built in: what the brand crawl found, fetched from the business's own
//! hosts only. Any ability with a `media:` block is a source too, listed and
//! fetched through its manifest like any other call (`kiqr_core::media`).

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use futures::stream::{self, StreamExt};
use kiqr_core::assets::{self, Asset};
use kiqr_core::brand::Brand;
use kiqr_core::extensions::{Manifest, Provide};
use kiqr_core::media::{self, MediaItem, WEBSITE};
use kiqr_core::project::Project;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/media/sources", get(sources))
        .route("/api/projects/{slug}/media/sources/{id}/items", get(items))
        .route("/api/projects/{slug}/media/import", post(import))
        .route("/api/projects/{slug}/media/import-site", post(import_site_route))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

/// Who asked, from the MCP server's header; a person otherwise.
fn by_of(headers: &HeaderMap) -> String {
    headers.get("x-kiqr-by").and_then(|v| v.to_str().ok()).map(str::to_string).unwrap_or_else(|| "user".into())
}

/// The connections that offer media to this project: on, with a `media:` block.
async fn media_sources(app: &App, slug: &str) -> Vec<(Manifest, bool)> {
    let reg = app.registry.read().await;
    let off = crate::ext::project_off(app, slug);
    reg.connections(slug).into_iter().filter(|(_, m)| m.media.is_some() && !off.contains(&m.id)).map(|(_, m)| (m.clone(), crate::ext::ready_in(app, Some(slug), m))).collect()
}

/// `GET …/media/sources`: the website first, then every ability that offers media.
async fn sources(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    let project = Project::load(&app.store, &slug)?;
    let brand = Brand::load(&app.store, &slug)?.unwrap_or_default();
    let all = assets::list(&app.store, &slug)?;
    let imported = |origin: &str| all.iter().filter(|a| a.kind == media::TYPE && !a.archived && a.source.get("origin").and_then(|v| v.as_str()) == Some(origin)).count();
    let mut out = vec![json!({
        "id": WEBSITE,
        "name": "Website",
        "icon": "globe",
        "builtin": true,
        "ready": !brand.is_empty(),
        "found": media::website_items(&brand, &project.url).len(),
        "imported": imported(WEBSITE),
        "note": if brand.is_empty() { "Crawl the site first; its pictures come from the crawl." } else { "" },
    })];
    for (m, ready) in media_sources(&app, &slug).await {
        out.push(json!({ "id": m.id, "name": m.name, "icon": if m.icon.is_empty() { "folder" } else { m.icon.as_str() }, "builtin": false, "ready": ready, "imported": imported(&m.id), "note": if ready { "" } else { "Not set up yet: grant it a credential under Setup → Abilities." } }));
    }
    Ok(Json(json!({ "sources": out })))
}

/// The items one source offers, marked with the media asset each already is.
async fn list_items(app: &App, slug: &str, source: &str) -> Result<Vec<MediaItem>, ApiError> {
    let project = Project::load(&app.store, slug)?;
    let mut items = if source == WEBSITE {
        let brand = Brand::load(&app.store, slug)?.unwrap_or_default();
        media::website_items(&brand, &project.url)
    } else {
        let (m, ready) = media_sources(app, slug).await.into_iter().find(|(m, _)| m.id == source).ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no media source '{source}' here; the website, or an ability with media switched on for this project")))?;
        if !ready {
            return Err(ApiError(StatusCode::PRECONDITION_FAILED, format!("{} is not set up yet; grant it a credential under Setup → Abilities", m.name)));
        }
        let spec = m.media.clone().ok_or_else(|| bad("no media block"))?;
        let provide = Provide { request: Some(spec.list.request.clone()), ..Default::default() };
        let out = crate::ext::execute(app, slug, &m, &provide, None, None, &json!({}), None).await.map_err(|e| bad(format!("{} could not list its media: {e:#}", m.name)))?;
        media::items(&spec.list, &out.json, &m.id)
    };
    let all = assets::list(&app.store, slug)?;
    media::mark_imported(&mut items, &all);
    Ok(items)
}

/// `GET …/media/sources/{id}/items`.
async fn items(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let items = list_items(&app, &slug, &id).await?;
    Ok(Json(json!({ "source": id, "items": items })))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Import {
    source: Option<String>,
    id: Option<String>,
    url: Option<String>,
    alt: Option<String>,
}

/// `POST …/media/import` `{source?, id?, url?, alt?}`: fetch one item through
/// its source and put it in the Library. The website takes a `url` (the
/// business's own hosts only); an ability takes the `id` its list gave.
async fn import(State(app): State<App>, Path(slug): Path<String>, headers: HeaderMap, Json(body): Json<Import>) -> ApiResult<Value> {
    let project = Project::load(&app.store, &slug)?;
    let by = by_of(&headers);
    let source = body.source.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| WEBSITE.into());
    let (asset, new) = if source == WEBSITE {
        let brand = Brand::load(&app.store, &slug)?.unwrap_or_default();
        let known = media::website_items(&brand, &project.url);
        let mut item = match (&body.url, &body.id) {
            (Some(u), _) if !u.trim().is_empty() => known.iter().find(|i| i.url.as_deref() == Some(u.trim())).cloned().unwrap_or_else(|| MediaItem { source: WEBSITE.into(), id: media::url_id(u.trim()), name: media::file_name_of(u.trim()), url: Some(u.trim().to_string()), ..Default::default() }),
            (_, Some(id)) => known.into_iter().find(|i| &i.id == id).ok_or_else(|| bad(format!("the website has no picture '{id}'; list_media says what it has, or give a url")))?,
            _ => return Err(bad("give the picture's `url` on the business's own website")),
        };
        if let Some(alt) = body.alt.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            item.alt = alt.to_string();
        }
        let url = item.url.clone().unwrap_or_default();
        let own = kiqr_core::brand::own_hosts(&project.url, &brand);
        let (bytes, mime) = media::fetch_own(&url, &own).await.map_err(|e| bad(format!("{e:#}")))?;
        media::import(&app.store, &slug, &item, &bytes, &mime, &by).map_err(|e| bad(format!("{e:#}")))?
    } else {
        let id = body.id.clone().filter(|s| !s.trim().is_empty()).ok_or_else(|| bad("give the item's `id` from list_media"))?;
        let items = list_items(&app, &slug, &source).await?;
        let mut item = items.into_iter().find(|i| i.id == id).ok_or_else(|| bad(format!("{source} lists no item '{id}'")))?;
        if let Some(alt) = body.alt.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            item.alt = alt.to_string();
        }
        let (m, _) = media_sources(&app, &slug).await.into_iter().find(|(m, _)| m.id == source).ok_or_else(|| bad("no such source"))?;
        let spec = m.media.clone().ok_or_else(|| bad("no media block"))?;
        let out = crate::ext::execute(&app, &slug, &m, &spec.get, None, None, &media::get_args(&item), None).await.map_err(|e| bad(format!("{} could not fetch {}: {e:#}", m.name, item.name)))?;
        let (bytes, mime) = out.file.ok_or_else(|| bad(format!("{} answered without a file for {}", m.name, item.name)))?;
        let mime = if mime.is_empty() || mime == "application/octet-stream" { if item.mime.is_empty() { kiqr_core::fetch::sniff_mime(&bytes) } else { item.mime.clone() } } else { mime };
        media::import(&app.store, &slug, &item, &bytes, &mime, &by).map_err(|e| bad(format!("{e:#}")))?
    };
    if new {
        crate::api::emit_saved(&app, &slug, &asset, true);
    }
    Ok(Json(answer(&app, &slug, &asset, new)))
}

fn answer(app: &App, slug: &str, a: &Asset, new: bool) -> Value {
    let picture = a.files.iter().find(|f| f.role == "image").map(|f| media::picture_json(&app.store, slug, a, f));
    json!({ "ok": true, "new": new, "asset": { "type": a.kind, "id": a.id, "title": a.title }, "picture": picture, "note": if new { "In the Library under Media." } else { "Already in the Library under Media." } })
}

/// Bring the website's pictures into the Library: the ones not there yet, up
/// to `SITE_LIMIT`, four at a time, best effort. What landed comes back.
pub async fn import_site(app: &App, slug: &str, by: &str) -> Vec<Asset> {
    let Ok(project) = Project::load(&app.store, slug) else { return vec![] };
    let Ok(Some(brand)) = Brand::load(&app.store, slug) else { return vec![] };
    let all = assets::list(&app.store, slug).unwrap_or_default();
    let mut items = media::website_items(&brand, &project.url);
    media::mark_imported(&mut items, &all);
    let wanted: Vec<MediaItem> = items.into_iter().filter(|i| i.imported.is_none()).take(media::SITE_LIMIT).collect();
    if wanted.is_empty() {
        return vec![];
    }
    let own = kiqr_core::brand::own_hosts(&project.url, &brand);
    let fetched: Vec<(MediaItem, Vec<u8>, String)> = stream::iter(wanted)
        .map(|item| {
            let own = own.clone();
            async move {
                let url = item.url.clone().unwrap_or_default();
                match media::fetch_own(&url, &own).await {
                    Ok((bytes, mime)) => Some((item, bytes, mime)),
                    Err(e) => {
                        tracing::debug!(%url, error = %e, "site picture skipped");
                        None
                    }
                }
            }
        })
        .buffer_unordered(4)
        .filter_map(|x| async move { x })
        .collect()
        .await;
    let mut made = vec![];
    for (item, bytes, mime) in fetched {
        match media::import(&app.store, slug, &item, &bytes, &mime, by) {
            Ok((a, true)) => {
                crate::api::emit_saved(app, slug, &a, true);
                made.push(a);
            }
            Ok((_, false)) => {}
            Err(e) => tracing::debug!(%slug, item = %item.name, error = %e, "site picture not kept"),
        }
    }
    if !made.is_empty() {
        tracing::info!(%slug, n = made.len(), "pictures from the site landed under Media");
    }
    made
}

/// `POST …/media/import-site`.
async fn import_site_route(State(app): State<App>, Path(slug): Path<String>, headers: HeaderMap) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let made = import_site(&app, &slug, &by_of(&headers)).await;
    Ok(Json(json!({ "imported": made.len(), "assets": made.iter().map(|a| json!({ "id": a.id, "title": a.title })).collect::<Vec<_>>() })))
}

/// The Library's pictures, importing the site's first when there are none
/// yet but the crawl found some (projects crawled before Media existed).
pub async fn pictures_or_import(app: &App, slug: &str) -> Vec<(Asset, kiqr_core::assets::AssetFile)> {
    let mut pics = media::pictures(&app.store, slug).unwrap_or_default();
    if pics.is_empty() {
        let has = Brand::load(&app.store, slug).ok().flatten().is_some_and(|b| b.cover.is_some() || !b.images.is_empty());
        if has {
            import_site(app, slug, "kiqr").await;
            pics = media::pictures(&app.store, slug).unwrap_or_default();
        }
    }
    pics
}
