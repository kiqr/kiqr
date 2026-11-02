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
    Ok(Json(kit(&app, &slug).await?))
}

/// The brand kit as JSON: what `brand_kit` answers and what the built-in
/// card template draws from.
pub async fn kit(app: &App, slug: &str) -> Result<Value, ApiError> {
    let project = Project::load(&app.store, slug)?;
    let brand = Brand::load(&app.store, slug)?.unwrap_or_default();
    let logo = cached_brand_file(app, slug, brand.logo.as_deref(), "logo").await;
    let icon = cached_brand_file(app, slug, brand.icon.as_deref(), "icon").await;
    let fonts: Vec<Value> = brand.fonts.iter().map(|f| json!({ "name": f, "available": app.renderer.has_family(f) })).collect();
    let font_family = brand.fonts.iter().find(|f| app.renderer.has_family(f)).cloned().unwrap_or_else(|| "Inter".into());
    let palette = palette(&brand.colors);
    // The project's networks' own sizes, then the generic presets.
    let mut sizes = serde_json::Map::new();
    for k in crate::networks::installed(app, slug).await {
        if let Some(i) = k.image {
            sizes.entry(k.kind.clone()).or_insert(json!([i.width, i.height]));
        }
    }
    for (n, w, h) in abilities::PRESETS {
        sizes.entry(n.to_string()).or_insert(json!([w, h]));
    }
    let sizes = Value::Object(sizes);
    // The Library's pictures (Media): the business's own, as local files the renderer may embed.
    let pictures = crate::media::pictures_or_import(app, slug).await;
    let media: Vec<Value> = pictures.iter().map(|(a, f)| kiqr_core::media::picture_json(&app.store, slug, a, f)).collect();
    let mut notes = vec![];
    if media.is_empty() {
        notes.push("No pictures under Library → Media yet, so every image is a designed card. The site's pictures land there after a crawl; the user can add a folder or an upload.".to_string());
    }
    if brand.colors.is_empty() {
        notes.push("No brand colours were found on the site; the palette is a neutral default. Ask the user for theirs if the image matters.".to_string());
    }
    if !brand.fonts.is_empty() && !brand.fonts.iter().any(|f| app.renderer.has_family(f)) {
        notes.push(format!("The brand's fonts ({}) are not installed here, so text renders in Inter.", brand.fonts.join(", ")));
    }
    if !app.renderer.html_available() {
        notes.push("No browser was found on this machine, so only SVG renders; `format: html` would fail.".into());
    }
    Ok(json!({
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
        "media": media,
        "html_available": app.renderer.html_available(),
        "notes": notes,
    }))
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
    let source = args.get("source").and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| anyhow!("the built-in renderer needs `source`: the SVG (or HTML) to render. For a generated photo or illustration, connect a generator under Setup → Abilities."))?;
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
    let kinds = crate::networks::installed(&app, &slug).await;
    if let Some(o) = args.as_object_mut() {
        if let Some((w, h)) = o.get("size").and_then(|v| v.as_str()).and_then(|n| abilities::size_for(n, &kinds)) {
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

// ------------------------------------------------------------ the card

/// What a post's card is made of: the idea (headline, support, style), the
/// layout, a picture from Media when the layout uses one, and the post's
/// position in its batch, which varies the plain card so a batch never
/// repeats itself.
pub struct Card<'a> {
    pub headline: &'a str,
    pub support: &'a str,
    /// `dark`, `light` or `accent`: which brand colour is the background.
    pub style: &'a str,
    /// `card` (words on colour), `photo` (the picture full bleed, words over a
    /// scrim) or `split` (the picture one side, words the other).
    pub layout: &'a str,
    /// A local image file and its pixel size.
    pub picture: Option<(&'a std::path::Path, u32, u32)>,
    pub index: usize,
}

struct Colours {
    bg: String,
    fg: String,
    muted: String,
    bar: String,
    dark: String,
    accent: String,
}

fn colours(kit: &Value, style: &str) -> Colours {
    let pal = &kit["palette"];
    let col = |k: &str, d: &str| pal[k].as_str().unwrap_or(d).to_string();
    let (dark, light, accent) = (col("dark", "#0b1220"), col("light", "#f8fafc"), col("accent", "#14b8a6"));
    let (bg, fg, muted, bar) = match style {
        "light" => (light.clone(), col("text_on_light", "#0b1220"), col("muted_on_light", "#475569"), accent.clone()),
        "accent" => {
            let on_dark = render::luminance(&accent).unwrap_or(0.3) < 0.45;
            if on_dark {
                (accent.clone(), col("text_on_dark", "#ffffff"), col("muted_on_dark", "#cbd5e1"), light.clone())
            } else {
                (accent.clone(), col("text_on_light", "#0b1220"), col("muted_on_light", "#475569"), dark.clone())
            }
        }
        _ => (dark.clone(), col("text_on_dark", "#ffffff"), col("muted_on_dark", "#cbd5e1"), accent.clone()),
    };
    Colours { bg, fg, muted, bar, dark, accent }
}

/// A post's image, designed at 1600 wide and scaled by the viewBox to any
/// size. The layout falls back: `split` needs a landscape picture on a
/// landscape canvas, else it is `photo`; a picture-led layout with no picture
/// is the plain card.
pub fn card(kit: &Value, c: &Card, width: u32, height: u32) -> String {
    let w = 1600.0_f32;
    let h = (w * height as f32 / width.max(1) as f32).round();
    let layout = match (c.layout, c.picture) {
        ("photo", Some(_)) => "photo",
        ("split", Some((_, pw, ph))) if pw >= ph && h <= w => "split",
        ("split", Some(_)) => "photo",
        _ => "card",
    };
    let font = kit["font_family"].as_str().unwrap_or("Inter, sans-serif");
    let site = kit["site_name"].as_str().unwrap_or("").to_uppercase();
    let logo = kit["logo"].as_str().filter(|l| l.starts_with("file://"));
    let icon = kit["icon"].as_str().filter(|l| l.starts_with("file://"));
    let col = colours(kit, if layout == "photo" { "dark" } else { c.style });
    let margin = 128.0_f32;
    let footer_y = h - margin;
    let mut out = String::new();
    out.push_str(&format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 1600 {h}" font-family="{}">"#, esc(font)));
    match layout {
        "photo" => {
            let (path, _, _) = c.picture.expect("a photo layout has a picture");
            let href = format!("file://{}", path.display());
            out.push_str(&format!(r#"<image href="{}" x="0" y="0" width="1600" height="{h}" preserveAspectRatio="xMidYMid slice"/>"#, esc(&href)));
            out.push_str(&format!(r#"<defs><linearGradient id="scrim" x1="0" y1="0" x2="0" y2="1"><stop offset="0.3" stop-color="{d}" stop-opacity="0"/><stop offset="1" stop-color="{d}" stop-opacity="0.9"/></linearGradient></defs><rect width="1600" height="{h}" fill="url(#scrim)"/>"#, d = col.dark));
            let usable = w - 2.0 * margin;
            let head_size = if h > 1200.0 { 80.0 } else { 68.0 };
            let head_lines = wrap(c.headline, (usable / (head_size * 0.56)) as usize, 3);
            let sup_size = 32.0_f32;
            let sup_lines = if c.support.trim().is_empty() { vec![] } else { wrap(c.support, (usable / (sup_size * 0.5)) as usize, 2) };
            let head_h = head_lines.len() as f32 * head_size * 1.15;
            let sup_h = if sup_lines.is_empty() { 0.0 } else { 28.0 + sup_lines.len() as f32 * sup_size * 1.35 };
            let mut y = footer_y - 70.0 - sup_h - head_h + head_size;
            out.push_str(&format!(r#"<rect x="{margin}" y="{:.0}" width="10" height="{:.0}" rx="5" fill="{}"/>"#, y - head_size * 0.85, head_h + sup_h - head_size * 0.1, col.accent));
            for l in &head_lines {
                out.push_str(&format!(r##"<text x="{}" y="{y:.0}" fill="#ffffff" font-size="{head_size}" font-weight="700" letter-spacing="-1">{}</text>"##, margin + 44.0, esc(l)));
                y += head_size * 1.15;
            }
            if !sup_lines.is_empty() {
                y += 28.0;
                for l in &sup_lines {
                    out.push_str(&format!(r##"<text x="{}" y="{y:.0}" fill="#e2e8f0" font-size="{sup_size}">{}</text>"##, margin + 44.0, esc(l)));
                    y += sup_size * 1.35;
                }
            }
            footer(&mut out, &site, logo, "#ffffff", margin, footer_y, w, false);
        }
        "split" => {
            let (path, _, _) = c.picture.expect("a split layout has a picture");
            let href = format!("file://{}", path.display());
            let pic_w = 720.0_f32;
            let right = c.index % 2 == 1;
            let pic_x = if right { w - pic_w } else { 0.0 };
            out.push_str(&format!(r#"<rect width="1600" height="{h}" fill="{}"/>"#, col.bg));
            out.push_str(&format!(r#"<defs><clipPath id="pic"><rect x="{pic_x}" y="0" width="{pic_w}" height="{h}"/></clipPath></defs><image href="{}" x="{pic_x}" y="0" width="{pic_w}" height="{h}" preserveAspectRatio="xMidYMid slice" clip-path="url(#pic)"/>"#, esc(&href)));
            let text_x = if right { margin } else { pic_w + 96.0 };
            let usable = w - pic_w - 96.0 - margin;
            let head_size = 60.0_f32;
            let head_lines = wrap(c.headline, (usable / (head_size * 0.56)) as usize, 4);
            let sup_size = 30.0_f32;
            let sup_lines = if c.support.trim().is_empty() { vec![] } else { wrap(c.support, (usable / (sup_size * 0.5)) as usize, 3) };
            let head_h = head_lines.len() as f32 * head_size * 1.18;
            let sup_h = if sup_lines.is_empty() { 0.0 } else { 30.0 + sup_lines.len() as f32 * sup_size * 1.35 };
            let block = head_h + sup_h;
            let mut y = ((footer_y - 60.0 - margin - block) / 2.0 + margin).max(margin + 20.0) + head_size * 0.8;
            out.push_str(&format!(r#"<rect x="{text_x}" y="{:.0}" width="64" height="8" rx="4" fill="{}"/>"#, y - head_size - 24.0, col.bar));
            for l in &head_lines {
                out.push_str(&format!(r#"<text x="{text_x}" y="{y:.0}" fill="{}" font-size="{head_size}" font-weight="700" letter-spacing="-1">{}</text>"#, col.fg, esc(l)));
                y += head_size * 1.18;
            }
            if !sup_lines.is_empty() {
                y += 30.0;
                for l in &sup_lines {
                    out.push_str(&format!(r#"<text x="{text_x}" y="{y:.0}" fill="{}" font-size="{sup_size}">{}</text>"#, col.muted, esc(l)));
                    y += sup_size * 1.35;
                }
            }
            if !site.is_empty() {
                out.push_str(&format!(r#"<text x="{text_x}" y="{footer_y:.0}" fill="{}" font-size="26" font-weight="600" letter-spacing="2">{}</text>"#, col.bar, esc(&site)));
            }
            if let Some(l) = logo {
                let lx = if right { pic_x - margin - 220.0 } else { w - margin - 220.0 };
                out.push_str(&format!(r#"<image href="{}" x="{lx}" y="{}" width="220" height="56" preserveAspectRatio="xMaxYMid meet"/>"#, esc(l), footer_y - 46.0));
            }
        }
        _ => {
            out.push_str(&format!(r#"<rect width="1600" height="{h}" fill="{}"/>"#, col.bg));
            let variant = c.index % 3;
            let usable = w - 2.0 * margin;
            let (head_size, head_lines) = {
                let mut size = if h > 1200.0 { 88.0 } else { 76.0 };
                let mut lines = wrap(c.headline, (usable / (size * 0.56)) as usize, 3);
                if lines.len() > 2 && c.headline.split_whitespace().count() > 12 {
                    size = 64.0;
                    lines = wrap(c.headline, (usable / (size * 0.56)) as usize, 4);
                }
                (size, lines)
            };
            let sup_size = 34.0_f32;
            let sup_lines = if c.support.trim().is_empty() { vec![] } else { wrap(c.support, (usable / (sup_size * 0.5)) as usize, 2) };
            let head_h = head_lines.len() as f32 * head_size * 1.18;
            let sup_h = if sup_lines.is_empty() { 0.0 } else { 36.0 + sup_lines.len() as f32 * sup_size * 1.35 };
            let block = head_h + sup_h;
            match variant {
                1 => {
                    // Centred, an accent rule under the headline, the site name as a kicker above.
                    let cx = w / 2.0;
                    let mut y = ((footer_y - margin - block) / 2.0 + margin).max(margin + 60.0);
                    if !site.is_empty() {
                        out.push_str(&format!(r#"<text x="{cx}" y="{:.0}" fill="{}" font-size="26" font-weight="600" letter-spacing="4" text-anchor="middle">{}</text>"#, y - head_size * 0.9 - 40.0, col.bar, esc(&site)));
                    }
                    for l in &head_lines {
                        out.push_str(&format!(r#"<text x="{cx}" y="{y:.0}" fill="{}" font-size="{head_size}" font-weight="800" letter-spacing="-1" text-anchor="middle">{}</text>"#, col.fg, esc(l)));
                        y += head_size * 1.18;
                    }
                    out.push_str(&format!(r#"<rect x="{}" y="{:.0}" width="120" height="6" rx="3" fill="{}"/>"#, cx - 60.0, y - head_size * 0.6, col.bar));
                    if !sup_lines.is_empty() {
                        y += 36.0;
                        for l in &sup_lines {
                            out.push_str(&format!(r#"<text x="{cx}" y="{y:.0}" fill="{}" font-size="{sup_size}" text-anchor="middle">{}</text>"#, col.muted, esc(l)));
                            y += sup_size * 1.35;
                        }
                    }
                    if let Some(l) = logo {
                        out.push_str(&format!(r#"<image href="{}" x="{}" y="{}" width="240" height="60" preserveAspectRatio="xMidYMid meet"/>"#, esc(l), cx - 120.0, footer_y - 50.0));
                    }
                }
                2 => {
                    // Words low on the page, the brand's mark large and faded above: the icon when there is one, else a circle.
                    match icon {
                        Some(i) => out.push_str(&format!(r#"<image href="{}" x="{}" y="{}" width="560" height="560" preserveAspectRatio="xMidYMid meet" opacity="0.14"/>"#, esc(i), w - margin - 480.0, margin - 80.0)),
                        None => out.push_str(&format!(r#"<circle cx="{}" cy="{}" r="420" fill="{}" opacity="0.16"/>"#, w - 220.0, 180.0, col.bar)),
                    }
                    if !site.is_empty() {
                        out.push_str(&format!(r#"<text x="{margin}" y="{:.0}" fill="{}" font-size="26" font-weight="600" letter-spacing="4">{}</text>"#, margin + 20.0, col.bar, esc(&site)));
                    }
                    let mut y = footer_y - 40.0 - sup_h - head_h + head_size;
                    for l in &head_lines {
                        out.push_str(&format!(r#"<text x="{margin}" y="{y:.0}" fill="{}" font-size="{head_size}" font-weight="800" letter-spacing="-2">{}</text>"#, col.fg, esc(l)));
                        y += head_size * 1.18;
                    }
                    if !sup_lines.is_empty() {
                        y += 36.0;
                        for l in &sup_lines {
                            out.push_str(&format!(r#"<text x="{margin}" y="{y:.0}" fill="{}" font-size="{sup_size}">{}</text>"#, col.muted, esc(l)));
                            y += sup_size * 1.35;
                        }
                    }
                    if let Some(l) = logo {
                        out.push_str(&format!(r#"<image href="{}" x="{}" y="{}" width="280" height="64" preserveAspectRatio="xMaxYMid meet"/>"#, esc(l), w - margin - 280.0, margin - 2.0));
                    }
                }
                _ => {
                    // Left-aligned with a bar: the first template of the instructions.
                    let top = ((footer_y - 60.0 - margin - block) / 2.0 + margin).max(margin + 40.0);
                    out.push_str(&format!(r#"<rect x="{margin}" y="{}" width="12" height="{}" rx="6" fill="{}"/>"#, top - head_size * 0.85, block.max(head_size), col.bar));
                    let mut y = top;
                    for l in &head_lines {
                        out.push_str(&format!(r#"<text x="{}" y="{y:.0}" fill="{}" font-size="{head_size}" font-weight="700" letter-spacing="-1">{}</text>"#, margin + 60.0, col.fg, esc(l)));
                        y += head_size * 1.18;
                    }
                    if !sup_lines.is_empty() {
                        y += 36.0;
                        for l in &sup_lines {
                            out.push_str(&format!(r#"<text x="{}" y="{y:.0}" fill="{}" font-size="{sup_size}">{}</text>"#, margin + 60.0, col.muted, esc(l)));
                            y += sup_size * 1.35;
                        }
                    }
                    footer(&mut out, &site, logo, &col.bar, margin, footer_y, w, true);
                }
            }
        }
    }
    out.push_str("</svg>");
    out
}

/// The site name bottom-left and the logo bottom-right.
#[allow(clippy::too_many_arguments)]
fn footer(out: &mut String, site: &str, logo: Option<&str>, colour: &str, margin: f32, footer_y: f32, w: f32, spaced: bool) {
    if !site.is_empty() {
        out.push_str(&format!(r#"<text x="{margin}" y="{footer_y:.0}" fill="{colour}" font-size="30" font-weight="600" letter-spacing="{}">{}</text>"#, if spaced { 2 } else { 1 }, esc(site)));
    }
    if let Some(l) = logo {
        out.push_str(&format!(r#"<image href="{}" x="{}" y="{}" width="280" height="64" preserveAspectRatio="xMaxYMid meet"/>"#, esc(l), w - margin - 280.0, footer_y - 52.0));
    }
}

/// Greedy word wrap by a character budget, at most `max` lines; the last
/// line gets an ellipsis when words are left over.
fn wrap(text: &str, per_line: usize, max: usize) -> Vec<String> {
    let per_line = per_line.max(8);
    let mut lines: Vec<String> = vec![];
    let mut cur = String::new();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        if cur.is_empty() {
            cur = w.to_string();
        } else if cur.chars().count() + 1 + w.chars().count() <= per_line {
            cur.push(' ');
            cur.push_str(w);
        } else {
            lines.push(std::mem::take(&mut cur));
            if lines.len() == max {
                break;
            }
            continue;
        }
        i += 1;
    }
    if lines.len() < max && !cur.is_empty() {
        lines.push(cur);
    } else if lines.len() == max && i < words.len() {
        if let Some(last) = lines.last_mut() {
            let cut: String = last.chars().take(per_line.saturating_sub(1)).collect();
            *last = format!("{}…", cut.trim_end());
        }
    }
    lines
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The card size for a post: its network's (the channel kind in its first tag), else landscape.
fn card_size(tags: &[String], kinds: &[kiqr_core::extensions::Kind]) -> (u32, u32) {
    tags.first().and_then(|t| kinds.iter().find(|k| &k.kind == t)).and_then(|k| k.image).map(|i| (i.width, i.height)).or_else(|| abilities::size_preset("landscape")).unwrap_or((1600, 900))
}

/// Give each post the posts job wrote its image, when the instructions gave
/// one an idea: the picture it named from Media (full bleed or split), else a
/// card varied by its place in the batch. Best effort: a failed render is
/// logged and the post stays.
pub async fn cards_for_posts(app: &App, slug: &str, made: &[(kiqr_core::assets::Asset, Option<kiqr_core::extract::PostImage>)]) {
    if !made.iter().any(|(_, i)| i.is_some()) {
        return;
    }
    let kit = match kit(app, slug).await {
        Ok(k) => k,
        Err(e) => {
            tracing::warn!(%slug, error = %e.1, "no brand kit; posts stay without images");
            return;
        }
    };
    let allowed = vec![app.store.home().brand_files_dir(slug), app.store.home().assets_dir(slug)];
    let kinds = crate::networks::installed(app, slug).await;
    for (index, (a, img)) in made.iter().enumerate() {
        let Some(img) = img else { continue };
        let (w, h) = card_size(&a.tags, &kinds);
        let style = if matches!(img.style.as_str(), "dark" | "light" | "accent") { img.style.as_str() } else { ["dark", "light", "accent"][index % 3] };
        // Only a picture that is in the Library's Media; the model names an id, never a path.
        let picture = img.media.as_deref().filter(|m| !m.trim().is_empty()).and_then(|id| kiqr_core::assets::load(&app.store, slug, kiqr_core::media::TYPE, id.trim()).ok()).and_then(|m| {
            let f = m.files.iter().find(|f| f.role == "image")?;
            Some((app.store.home().assets_dir(slug).join(&m.kind).join(&m.id).join(&f.path), f.width.unwrap_or(0), f.height.unwrap_or(0)))
        });
        let layout = match (img.layout.as_deref().unwrap_or(""), &picture) {
            ("photo" | "split", Some(_)) => img.layout.as_deref().unwrap_or("photo"),
            (_, Some(_)) => if index % 2 == 0 { "photo" } else { "split" },
            _ => "card",
        };
        let svg = card(&kit, &Card { headline: img.headline.trim(), support: img.support.trim(), style, layout, picture: picture.as_ref().map(|(p, pw, ph)| (p.as_path(), *pw, *ph)), index }, w, h);
        match app.renderer.render_svg(&svg, w, h, &allowed) {
            Ok(png) => {
                let role = kiqr_core::assets::find_type(&app.store, &a.kind).map(|t| t.role_for("png")).unwrap_or_else(|| "media".into());
                match kiqr_core::assets::write_file(&app.store, slug, &a.kind, &a.id, &role, "card.png", &png) {
                    Ok(saved) => {
                        crate::api::emit_saved(app, slug, &saved, false);
                        app.bus.emit(Some(slug), kiqr_core::events::EventKind::MediaCreated { ability: "image".into(), asset_type: a.kind.clone(), id: a.id.clone(), file: "card.png".into(), provider: "Built-in renderer".into() });
                    }
                    Err(e) => tracing::warn!(%slug, asset = %a.id, error = %e, "could not save the card"),
                }
            }
            Err(e) => tracing::warn!(%slug, asset = %a.id, error = %e, "could not render the card"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_kit() -> Value {
        json!({ "palette": { "dark": "#101010", "light": "#fafafa", "accent": "#e11d74" }, "font_family": "Inter, sans-serif", "site_name": "Acme & Co" })
    }

    fn card_svg(kit: &Value, headline: &str, support: &str, style: &str, width: u32, height: u32) -> String {
        card(kit, &Card { headline, support, style, layout: "card", picture: None, index: 0 }, width, height)
    }

    #[test]
    fn cards_wrap_and_escape() {
        let kit = test_kit();
        let svg = card_svg(&kit, "Render only when a page needs it & nothing more", "One credit per page, whatever the site throws at you.", "accent", 1080, 1080);
        assert!(svg.contains(r#"viewBox="0 0 1600 1600""#) && svg.contains("ACME &amp; CO") && svg.contains("needs it &amp;"));
        assert_eq!(wrap("one two three four five six", 9, 2), vec!["one two", "three…"]);
        assert_eq!(wrap("short", 30, 3), vec!["short"]);
        let r = render::Renderer::new(std::env::temp_dir().join("kiqr-card-test"));
        let png = r.render_svg(&svg, 1080, 1080, &[]).expect("renders");
        assert!(png.len() > 2000);
    }

    #[test]
    fn plain_cards_vary_by_index() {
        let kit = test_kit();
        let mk = |i: usize| card(&kit, &Card { headline: "Three things to check first", support: "Before the launch.", style: "dark", layout: "card", picture: None, index: i }, 1600, 900);
        let (a, b, c, d) = (mk(0), mk(1), mk(2), mk(3));
        assert!(a != b && b != c && a != c, "three variants");
        assert_eq!(a, d, "the fourth repeats the first");
        assert!(b.contains(r#"text-anchor="middle""#), "the second is centred");
        assert!(c.contains("<circle"), "the third carries the mark");
        // A picture layout without a picture is the plain card.
        let plain = card(&kit, &Card { headline: "x", support: "", style: "dark", layout: "card", picture: None, index: 0 }, 1600, 900);
        let no_pic = card(&kit, &Card { headline: "x", support: "", style: "dark", layout: "photo", picture: None, index: 0 }, 1600, 900);
        assert_eq!(plain, no_pic);
    }

    #[test]
    fn picture_layouts_render_from_an_allowed_folder() {
        let dir = std::env::temp_dir().join(format!("kiqr-pic-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut p = render::raster::Pixmap::new(800, 500).unwrap();
        p.fill(render::raster::Color::from_rgba8(200, 40, 40, 255));
        let file = dir.join("team.png");
        std::fs::write(&file, p.encode_png().unwrap()).unwrap();
        let kit = test_kit();
        let r = render::Renderer::new(std::env::temp_dir().join("kiqr-card-test"));
        let photo = card(&kit, &Card { headline: "The team at work", support: "Every Tuesday.", style: "light", layout: "photo", picture: Some((&file, 800, 500)), index: 0 }, 1600, 900);
        assert!(photo.contains("scrim") && photo.contains("slice"));
        let png = r.render_svg(&photo, 1600, 900, &[dir.clone()]).unwrap();
        // The red picture fills the frame: far more bytes than a flat card.
        let flat = r.render_svg(&card_svg(&kit, "The team at work", "Every Tuesday.", "light", 1600, 900), 1600, 900, &[]).unwrap();
        assert!(png.len() > flat.len(), "{} vs {}", png.len(), flat.len());
        let split = card(&kit, &Card { headline: "The team at work", support: "", style: "accent", layout: "split", picture: Some((&file, 800, 500)), index: 1 }, 1600, 900);
        assert!(split.contains("clipPath") && split.contains(r#"x="880""#), "the odd index puts the picture right");
        assert!(r.render_svg(&split, 1600, 900, &[dir.clone()]).is_ok());
        // A portrait picture cannot split: it goes full bleed.
        let tall = card(&kit, &Card { headline: "x", support: "", style: "dark", layout: "split", picture: Some((&file, 500, 800)), index: 0 }, 1600, 900);
        assert!(tall.contains("scrim") && !tall.contains("clipPath"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
