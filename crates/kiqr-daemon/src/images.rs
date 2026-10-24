//! Images: the brand kit Seemo designs from, the built-in renderer behind the
//! `image` ability, and a direct render route for the CLI and tests.

use crate::api::{ApiError, ApiResult};
use crate::ext::ExecOut;
use crate::App;
use anyhow::{anyhow, bail, Result};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use kiqr_core::abilities;
use kiqr_core::brand::Brand;
use kiqr_core::project::Project;
use kiqr_core::render;
use serde_json::{json, Value};
use std::path::PathBuf;

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/brand/kit", get(brand_kit))
        .route("/api/projects/{slug}/brand/files/{name}", get(brand_file))
        .route("/api/projects/{slug}/render", post(render_route))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

// --------------------------------------------------------------- brand kit

/// What Seemo needs to design on brand: colours with roles, fonts as they
/// will render here, the logo and icon as local files, sizes, and whether
/// HTML rendering is on.
async fn brand_kit(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    let project = Project::load(&app.store, &slug)?;
    let brand = Brand::load(&app.store, &slug)?.unwrap_or_default();
    let logo = cached_brand_file(&app, &slug, brand.logo.as_deref(), "logo").await;
    let icon = cached_brand_file(&app, &slug, brand.icon.as_deref(), "icon").await;
    let fonts: Vec<Value> = brand.fonts.iter().map(|f| json!({ "name": f, "available": app.renderer.has_family(f) })).collect();
    let font_family = brand.fonts.iter().find(|f| app.renderer.has_family(f)).cloned().unwrap_or_else(|| "Inter".into());
    let palette = palette(&brand.colors);
    let sizes = json!({
        "x": [1600, 900], "linkedin": [1200, 627], "instagram": [1080, 1080], "instagram_portrait": [1080, 1350], "og": [1200, 630], "cover": [1600, 900],
    });
    let mut notes = vec![];
    if brand.colors.is_empty() {
        notes.push("No brand colours were found on the site; the palette is a neutral default. Ask the user for theirs if the image matters.".to_string());
    }
    if !brand.fonts.is_empty() && !brand.fonts.iter().any(|f| app.renderer.has_family(f)) {
        notes.push(format!("The brand's fonts ({}) are not installed here, so text renders in Inter.", brand.fonts.join(", ")));
    }
    if !app.renderer.html_available() {
        notes.push("No browser was found on this machine, so only SVG renders; `format: html` would fail.".into());
    }
    Ok(Json(json!({
        "site_name": if brand.site_name.is_empty() { project.name.clone() } else { brand.site_name.clone() },
        "tagline": brand.tagline,
        "colors": brand.colors,
        "palette": palette,
        "fonts": fonts,
        "font_family": if font_family == "Inter" { "Inter, sans-serif".to_string() } else { format!("{font_family}, Inter, sans-serif") },
        "font_css": app.renderer.font_css(),
        "logo": logo.as_ref().map(|p| format!("file://{}", p.display())),
        "icon": icon.as_ref().map(|p| format!("file://{}", p.display())),
        "sizes": sizes,
        "html_available": app.renderer.html_available(),
        "notes": notes,
    })))
}

/// Roles for the brand's colours: a dark background, a light background,
/// an accent, and text colours that read on each.
fn palette(colors: &[String]) -> Value {
    let lum = |c: &String| render::luminance(c).unwrap_or(0.5);
    let darkest = colors.iter().min_by(|a, b| lum(a).partial_cmp(&lum(b)).unwrap_or(std::cmp::Ordering::Equal));
    let lightest = colors.iter().max_by(|a, b| lum(a).partial_cmp(&lum(b)).unwrap_or(std::cmp::Ordering::Equal));
    let dark = darkest.filter(|c| lum(c) < 0.2).cloned().unwrap_or_else(|| "#0b1220".into());
    let light = lightest.filter(|c| lum(c) > 0.8).cloned().unwrap_or_else(|| "#f8fafc".into());
    // The accent is the most characteristic colour that is neither background.
    let accent = colors.iter().find(|c| lum(c) >= 0.08 && lum(c) <= 0.75 && **c != dark && **c != light).cloned().or_else(|| colors.first().cloned()).unwrap_or_else(|| "#14b8a6".into());
    json!({
        "dark": dark,
        "light": light,
        "accent": accent,
        "text_on_dark": "#ffffff",
        "muted_on_dark": "#cbd5e1",
        "text_on_light": "#0b1220",
        "muted_on_light": "#475569",
    })
}

/// The logo or icon as a local file, fetched once into the project's cache.
async fn cached_brand_file(app: &App, slug: &str, url: Option<&str>, role: &str) -> Option<PathBuf> {
    let url = url?.trim();
    if url.is_empty() {
        return None;
    }
    let dir = app.store.home().brand_files_dir(slug);
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with(&format!("{role}.")) {
                return Some(e.path());
            }
        }
    }
    let parsed = url::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).user_agent(kiqr_core::fetch::USER_AGENT).build().ok()?;
    let resp = client.get(parsed).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let ctype = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").split(';').next().unwrap_or("").trim().to_string();
    let bytes = resp.bytes().await.ok()?;
    if bytes.is_empty() || bytes.len() > 5 * 1024 * 1024 {
        return None;
    }
    let mime = if ctype.starts_with("image/") { ctype } else { crate::ext::sniff_mime(&bytes) };
    let ext = match crate::ext::extension_for(&mime) {
        "bin" => return None,
        e => e,
    };
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{role}.{ext}"));
    std::fs::write(&path, &bytes).ok()?;
    Some(path)
}

async fn brand_file(State(app): State<App>, Path((slug, name)): Path<(String, String)>) -> Result<axum::response::Response, ApiError> {
    let _ = Project::load(&app.store, &slug)?;
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') || name.starts_with('.') {
        return Err(bad("a file name is letters, digits, dots, dashes and underscores"));
    }
    let path = app.store.home().brand_files_dir(&slug).join(&name);
    let bytes = tokio::fs::read(&path).await.map_err(|_| ApiError(StatusCode::NOT_FOUND, format!("no brand file {name}")))?;
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    Ok(([(axum::http::header::CONTENT_TYPE, mime.to_string())], bytes).into_response())
}

// ---------------------------------------------------------------- rendering

/// Render `source` (SVG or HTML) at the asked size. The built-in connection
/// behind `create_image`; the caller puts the PNG on an asset.
pub async fn render_args(app: &App, slug: &str, args: &Value) -> Result<ExecOut> {
    let source = args.get("source").and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| anyhow!("the built-in renderer needs `source`: the SVG (or HTML) to render. For a generated photo or illustration, connect a generator under Settings → Connections."))?;
    let format = match args.get("format").and_then(|v| v.as_str()).map(|s| s.trim().to_ascii_lowercase()) {
        Some(f) if f == "svg" || f == "html" => f,
        _ => {
            let head = source.trim_start().to_ascii_lowercase();
            if head.starts_with("<!doctype") || head.starts_with("<html") || head.contains("<body") {
                "html".to_string()
            } else {
                "svg".to_string()
            }
        }
    };
    let (w, h) = match (args.get("width").and_then(|v| v.as_u64()), args.get("height").and_then(|v| v.as_u64())) {
        (Some(w), Some(h)) => (w as u32, h as u32),
        _ => match format.as_str() {
            "svg" => svg_size(source).unwrap_or((1600, 900)),
            _ => (1600, 900),
        },
    };
    if source.len() > 2_000_000 {
        bail!("the source is over 2 MB; keep designs small");
    }
    let png = if format == "html" {
        app.renderer.render_html(source, w, h).await?
    } else {
        let allowed = vec![app.store.home().brand_files_dir(slug), app.store.home().assets_dir(slug)];
        app.renderer.render_svg(source, w, h, &allowed)?
    };
    let renderer = if format == "html" { app.renderer.chrome().map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "browser".into())).unwrap_or_else(|| "browser".into()) } else { "resvg".to_string() };
    Ok(ExecOut { json: json!({ "renderer": renderer, "format": format, "width": w, "height": h }), text: None, file: Some((png, "image/png".into())) })
}

/// `width`/`height` or the viewBox of the root `<svg>`.
fn svg_size(svg: &str) -> Option<(u32, u32)> {
    let open = svg.find("<svg")?;
    let end = svg[open..].find('>')? + open;
    let tag = &svg[open..end];
    let attr = |name: &str| -> Option<String> {
        let i = tag.find(&format!("{name}=\""))? + name.len() + 2;
        let j = tag[i..].find('"')? + i;
        Some(tag[i..j].to_string())
    };
    let num = |s: &str| s.trim().trim_end_matches("px").parse::<f32>().ok().filter(|v| *v > 0.0);
    if let (Some(w), Some(h)) = (attr("width").and_then(|s| num(&s)), attr("height").and_then(|s| num(&s))) {
        return Some((w.round() as u32, h.round() as u32));
    }
    let vb = attr("viewBox")?;
    let parts: Vec<f32> = vb.split([' ', ',']).filter(|p| !p.is_empty()).filter_map(|p| p.parse().ok()).collect();
    if parts.len() == 4 && parts[2] > 0.0 && parts[3] > 0.0 {
        return Some((parts[2].round() as u32, parts[3].round() as u32));
    }
    None
}

/// `POST /api/projects/{slug}/render`: render and put the PNG on an asset,
/// like the `create_image` ability through the built-in renderer.
async fn render_route(State(app): State<App>, Path(slug): Path<String>, headers: HeaderMap, Json(args): Json<Value>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let mut args = args;
    if let Some(o) = args.as_object_mut() {
        if let Some((w, h)) = o.get("size").and_then(|v| v.as_str()).and_then(abilities::size_preset) {
            o.entry("width").or_insert(json!(w));
            o.entry("height").or_insert(json!(h));
        }
    }
    let out = render_args(&app, &slug, &args).await.map_err(|e| bad(format!("{e:#}")))?;
    let by = args.get("by").and_then(|v| v.as_str()).map(str::to_string).or_else(|| headers.get("x-kiqr-by").and_then(|v| v.to_str().ok()).map(str::to_string)).unwrap_or_else(|| "user".into());
    let (bytes, mime) = out.file.ok_or_else(|| bad("nothing was rendered"))?;
    let mut v = crate::ext::attach(&app, &slug, "image", &args, &by, bytes, mime, "Built-in renderer", "image").await?;
    v["renderer"] = out.json["renderer"].clone();
    v["width"] = out.json["width"].clone();
    v["height"] = out.json["height"].clone();
    Ok(Json(v))
}
