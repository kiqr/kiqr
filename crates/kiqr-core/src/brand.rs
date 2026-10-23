//! Brand assets: logo, icon, cover image, colours, fonts, imagery and social
//! links, collected from the crawl cache (raw HTML) plus up to three of the
//! home page's own stylesheets. No model involved. Stored at
//! `profile/brand.yaml`.

use crate::crawl::CachedPage;
use crate::events::{EventBus, EventKind};
use crate::project::Project;
use crate::Store;
use anyhow::Result;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use url::Url;

const MAX_COLORS: usize = 6;
const MAX_FONTS: usize = 3;
const MAX_IMAGES: usize = 12;
const MAX_STYLESHEETS: usize = 3;
const CSS_TIMEOUT: Duration = Duration::from_secs(5);
const CSS_MAX_BYTES: usize = 2 * 1024 * 1024;

/// What the business looks like. Every field defaults so older files load.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Brand {
    pub site_name: String,
    pub tagline: String,
    pub logo: Option<String>,
    pub icon: Option<String>,
    pub cover: Option<String>,
    /// Lowercase `#rrggbb`, most characteristic first.
    pub colors: Vec<String>,
    pub fonts: Vec<String>,
    pub images: Vec<BrandImage>,
    pub social: Vec<SocialLink>,
    /// RFC 3339.
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BrandImage {
    pub url: String,
    pub alt: String,
    pub page: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SocialLink {
    pub network: String,
    pub url: String,
}

impl Brand {
    pub fn load(store: &Store, slug: &str) -> Result<Option<Brand>> {
        store.read_yaml_opt(&store.home().brand_file(slug))
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        store.write_yaml(&store.home().brand_file(slug), self)
    }

    /// `logo, 3 colours, Inter, Caveat, 9 images` — for commit messages and the CLI.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.logo.is_some() {
            parts.push("logo".to_string());
        }
        parts.push(format!("{} colour{}", self.colors.len(), if self.colors.len() == 1 { "" } else { "s" }));
        if self.fonts.is_empty() {
            parts.push("no fonts".into());
        } else {
            parts.push(self.fonts.join(", "));
        }
        parts.push(format!("{} image{}", self.images.len(), if self.images.len() == 1 { "" } else { "s" }));
        parts.join(", ")
    }

    pub fn is_empty(&self) -> bool {
        self.site_name.is_empty() && self.logo.is_none() && self.icon.is_none() && self.colors.is_empty() && self.images.is_empty()
    }
}

/// Everything one page says about the brand.
#[derive(Clone, Debug, Default)]
pub struct PageBrandHints {
    pub og_site_name: Option<String>,
    pub ld_name: Option<String>,
    pub title: Option<String>,
    pub og_description: Option<String>,
    pub meta_description: Option<String>,
    pub ld_logo: Option<String>,
    pub img_logo: Option<String>,
    /// (url, score); higher is better.
    pub icons: Vec<(String, i64)>,
    pub og_image: Option<String>,
    pub og_image_alt: Option<String>,
    pub twitter_image: Option<String>,
    /// theme-color, msapplication-TileColor, mask-icon colour, as written.
    pub theme_colors: Vec<String>,
    pub google_fonts: Vec<String>,
    /// Same-origin stylesheets, in document order.
    pub stylesheets: Vec<Url>,
    /// Inline `<style>` blocks and `style=""` attributes, as CSS.
    pub inline_css: String,
    pub images: Vec<BrandImage>,
    pub social: Vec<SocialLink>,
}

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("static selector")
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn nonempty(s: Option<&str>) -> Option<String> {
    s.map(clean).filter(|s| !s.is_empty())
}

fn resolve(base: &Url, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with("data:") || href.starts_with("javascript:") {
        return None;
    }
    let mut u = base.join(href).ok()?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return None;
    }
    u.set_fragment(None);
    Some(u.to_string())
}

fn host_key(u: &Url) -> String {
    u.host_str().unwrap_or("").trim_start_matches("www.").to_ascii_lowercase()
}

/// Extract brand hints from one page's raw HTML.
pub fn hints(base: &Url, html: &str) -> PageBrandHints {
    let doc = Html::parse_document(html);
    let mut h = PageBrandHints::default();

    let meta = |key: &str| -> Option<String> {
        let s = sel(&format!("meta[property=\"{key}\"], meta[name=\"{key}\"]"));
        doc.select(&s).find_map(|m| nonempty(m.value().attr("content")))
    };
    h.og_site_name = meta("og:site_name");
    h.og_description = meta("og:description");
    h.meta_description = meta("description");
    h.og_image = meta("og:image").or_else(|| meta("og:image:url")).or_else(|| meta("og:image:secure_url")).and_then(|u| resolve(base, &u));
    h.og_image_alt = meta("og:image:alt");
    h.twitter_image = meta("twitter:image").or_else(|| meta("twitter:image:src")).and_then(|u| resolve(base, &u));
    h.title = doc.select(&sel("title")).next().map(|t| clean(&t.text().collect::<String>())).filter(|s| !s.is_empty());

    for m in doc.select(&sel("meta[name=theme-color], meta[name=msapplication-TileColor], meta[name=msapplication-tilecolor]")) {
        if let Some(c) = nonempty(m.value().attr("content")) {
            h.theme_colors.push(c);
        }
    }

    // Icons and stylesheets.
    let page_host = host_key(base);
    for l in doc.select(&sel("link[rel][href]")) {
        let rel = l.value().attr("rel").unwrap_or("").to_ascii_lowercase();
        let rels: Vec<&str> = rel.split_whitespace().collect();
        let href = l.value().attr("href").unwrap_or("");
        if rels.contains(&"mask-icon") {
            if let Some(c) = nonempty(l.value().attr("color")) {
                h.theme_colors.push(c);
            }
            continue;
        }
        if rels.contains(&"stylesheet") || (rels.contains(&"preload") && l.value().attr("as") == Some("style")) {
            if let Ok(u) = base.join(href) {
                if host_key(&u) == "fonts.googleapis.com" {
                    h.google_fonts.extend(google_families(&u));
                } else if host_key(&u) == page_host && !h.stylesheets.contains(&u) {
                    h.stylesheets.push(u);
                }
            }
            continue;
        }
        let apple = rels.iter().any(|r| r.starts_with("apple-touch-icon"));
        if apple || rels.contains(&"icon") {
            let Some(url) = resolve(base, href) else { continue };
            let size = icon_size(l.value().attr("sizes"));
            let ty = l.value().attr("type").unwrap_or("").to_ascii_lowercase();
            let path = url.to_ascii_lowercase();
            let is_svg = ty.contains("svg") || path.ends_with(".svg");
            let is_png = ty.contains("png") || path.ends_with(".png");
            let score = if apple {
                100_000 + if size > 0 { size } else { 180 }
            } else if is_svg {
                50_000
            } else if is_png {
                10_000 + size
            } else {
                size
            };
            h.icons.push((url, score));
        }
    }

    // JSON-LD: Organization / Brand name and logo.
    for s in doc.select(&sel("script[type=\"application/ld+json\"]")) {
        let text = s.text().collect::<String>();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(text.trim()) {
            walk_ld(&v, base, &mut h);
        }
    }

    // Inline CSS.
    for s in doc.select(&sel("style")) {
        h.inline_css.push_str(&s.text().collect::<String>());
        h.inline_css.push('\n');
    }
    for e in doc.select(&sel("[style]")) {
        if let Some(st) = e.value().attr("style") {
            h.inline_css.push_str("x{");
            h.inline_css.push_str(st);
            h.inline_css.push_str("}\n");
        }
    }
    // Google Fonts pulled in with @import.
    for u in google_font_urls(&h.inline_css) {
        h.google_fonts.extend(google_families(&u));
    }

    h.img_logo = find_logo(base, &doc);

    // Content images.
    for img in doc.select(&sel("img")) {
        if in_chrome(&img) {
            continue;
        }
        let v = img.value();
        let srcset = v.attr("srcset").or_else(|| v.attr("data-srcset"));
        let src = v.attr("src").filter(|s| !s.trim_start().starts_with("data:")).or_else(|| v.attr("data-src"));
        let best = srcset.and_then(|s| largest_srcset(base, s)).or_else(|| src.and_then(|s| resolve(base, s)));
        let Some(url) = best else { continue };
        let dim = |k: &str| v.attr(k).and_then(|x| x.trim().trim_end_matches("px").parse::<u32>().ok()).unwrap_or(0);
        let big = dim("width") >= 400 || dim("height") >= 400;
        let hero = ancestors_match(&img, 4, &["hero", "banner"]);
        if !(big || hero || srcset.is_some()) || !image_ok(&url) {
            continue;
        }
        if dim("width") > 0 && dim("width") < 120 && dim("height") > 0 && dim("height") < 120 {
            continue;
        }
        h.images.push(BrandImage { url, alt: clean(v.attr("alt").unwrap_or("")), page: base.to_string() });
    }

    // Social links.
    for a in doc.select(&sel("a[href]")) {
        if let Some(link) = a.value().attr("href").and_then(|href| social_link(base, href)) {
            h.social.push(link);
        }
    }
    h
}

fn walk_ld(v: &serde_json::Value, base: &Url, h: &mut PageBrandHints) {
    match v {
        serde_json::Value::Array(items) => items.iter().for_each(|i| walk_ld(i, base, h)),
        serde_json::Value::Object(map) => {
            let types: Vec<String> = match map.get("@type") {
                Some(serde_json::Value::String(s)) => vec![s.clone()],
                Some(serde_json::Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
                _ => vec![],
            };
            let is_org = types.iter().any(|t| {
                matches!(t.as_str(), "Organization" | "Brand" | "Corporation" | "LocalBusiness" | "NGO" | "Store" | "OnlineStore" | "OnlineBusiness")
                    || t.ends_with("Organization")
                    || t.ends_with("Business")
            });
            if is_org {
                if h.ld_name.is_none() {
                    h.ld_name = nonempty(map.get("name").and_then(|n| n.as_str()));
                }
                if h.ld_logo.is_none() {
                    let logo = match map.get("logo") {
                        Some(serde_json::Value::String(s)) => Some(s.clone()),
                        Some(serde_json::Value::Object(o)) => o.get("url").or_else(|| o.get("contentUrl")).and_then(|u| u.as_str()).map(str::to_string),
                        Some(serde_json::Value::Array(a)) => a.first().and_then(|x| x.as_str().map(str::to_string).or_else(|| x.get("url").and_then(|u| u.as_str()).map(str::to_string))),
                        _ => None,
                    };
                    h.ld_logo = logo.and_then(|l| resolve(base, &l));
                }
            }
            for (k, child) in map {
                if k != "logo" && (child.is_object() || child.is_array()) {
                    walk_ld(child, base, h);
                }
            }
        }
        _ => {}
    }
}

fn icon_size(sizes: Option<&str>) -> i64 {
    let Some(s) = sizes else { return 0 };
    s.split_whitespace()
        .map(|p| {
            if p.eq_ignore_ascii_case("any") {
                return 512;
            }
            p.to_ascii_lowercase().split('x').next().and_then(|w| w.parse::<i64>().ok()).unwrap_or(0)
        })
        .max()
        .unwrap_or(0)
}

/// Is the element inside the header, navigation or footer?
fn in_chrome(el: &ElementRef) -> bool {
    el.ancestors().filter_map(ElementRef::wrap).any(|a| {
        let name = a.value().name();
        name == "header" || name == "nav" || name == "footer" || a.value().attr("role").is_some_and(|r| r == "banner" || r == "navigation" || r == "contentinfo")
    })
}

fn in_header(el: &ElementRef) -> bool {
    el.ancestors().filter_map(ElementRef::wrap).any(|a| {
        let name = a.value().name();
        name == "header" || name == "nav" || a.value().attr("role").is_some_and(|r| r == "banner" || r == "navigation")
    })
}

fn attr_hint(el: &ElementRef, words: &[&str]) -> bool {
    let v = el.value();
    ["class", "id", "alt", "src", "aria-label", "title"]
        .iter()
        .filter_map(|k| v.attr(k))
        .any(|x| {
            let x = x.to_ascii_lowercase();
            words.iter().any(|w| x.contains(w))
        })
}

fn ancestors_match(el: &ElementRef, depth: usize, words: &[&str]) -> bool {
    el.ancestors().filter_map(ElementRef::wrap).take(depth).any(|a| {
        let v = a.value();
        ["class", "id"].iter().filter_map(|k| v.attr(k)).any(|x| {
            let x = x.to_ascii_lowercase();
            words.iter().any(|w| x.contains(w))
        })
    })
}

/// The `<a>` an element sits in, when that link points at the home page.
fn in_home_link(el: &ElementRef, base: &Url) -> bool {
    el.ancestors().filter_map(ElementRef::wrap).take(4).any(|a| {
        a.value().name() == "a"
            && a.value().attr("href").and_then(|h| base.join(h).ok()).is_some_and(|u| host_key(&u) == host_key(base) && (u.path() == "/" || u.path().is_empty()))
    })
}

fn find_logo(base: &Url, doc: &Html) -> Option<String> {
    let imgs: Vec<ElementRef> = doc.select(&sel("body img")).collect();
    let src = |img: &ElementRef| {
        let v = img.value();
        v.attr("src").filter(|s| !s.trim_start().starts_with("data:")).or_else(|| v.attr("data-src")).and_then(|s| resolve(base, s))
    };
    let hinted = |img: &ElementRef| attr_hint(img, &["logo"]) || ancestors_match(img, 3, &["logo", "brand"]);
    // 1. A logo-ish image in the header or navigation.
    if let Some(u) = imgs.iter().filter(|i| in_header(i) && hinted(i)).find_map(src) {
        return Some(u);
    }
    // 2. Any image inside a link to the home page in the header.
    if let Some(u) = imgs.iter().filter(|i| in_header(i) && in_home_link(i, base)).find_map(src) {
        return Some(u);
    }
    // 3. Near the top of the page: a logo-ish image among the first few, or one linking home.
    imgs.iter().take(3).filter(|i| hinted(i) || in_home_link(i, base)).find_map(src)
}

/// The largest candidate of a `srcset`, by width descriptor or density.
fn largest_srcset(base: &Url, srcset: &str) -> Option<String> {
    srcset
        .split(',')
        .filter_map(|c| {
            let mut parts = c.split_whitespace();
            let url = parts.next()?;
            let d = parts.next().unwrap_or("1x");
            let n: f64 = d.trim_end_matches(['w', 'x', 'h']).parse().unwrap_or(1.0);
            let weight = if d.ends_with('x') { n * 1000.0 } else { n };
            Some((resolve(base, url)?, weight))
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(u, _)| u)
}

fn image_ok(url: &str) -> bool {
    let l = url.to_ascii_lowercase();
    let path = l.split(['?', '#']).next().unwrap_or("");
    !(path.ends_with(".svg") || path.ends_with(".gif") || path.ends_with(".ico"))
        && !["avatar", "sprite", "pixel", "icon", "logo", "gravatar", "badge", "/track", "spacer"].iter().any(|w| l.contains(w))
}

fn social_link(base: &Url, href: &str) -> Option<SocialLink> {
    let u = base.join(href.trim()).ok()?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return None;
    }
    let host = host_key(&u);
    let host = host.trim_start_matches("m.").trim_start_matches("mobile.");
    let path = u.path().to_ascii_lowercase();
    let network = match host {
        "linkedin.com" => "linkedin",
        "x.com" | "twitter.com" => "x",
        "facebook.com" | "fb.com" => "facebook",
        "instagram.com" => "instagram",
        "youtube.com" | "youtu.be" => "youtube",
        "github.com" => "github",
        "tiktok.com" => "tiktok",
        _ => return None,
    };
    if path.trim_matches('/').is_empty() {
        return None;
    }
    let share = ["/intent", "/share", "/sharer", "/sharing", "/sharearticle", "/dialog", "/home?status"];
    if share.iter().any(|s| path.starts_with(s)) || u.query().is_some_and(|q| q.contains("status=") || q.contains("u=http")) {
        return None;
    }
    let mut u = u;
    u.set_fragment(None);
    Some(SocialLink { network: network.into(), url: u.to_string() })
}

// ---------- fonts ----------

/// Family names from a Google Fonts CSS URL (css and css2 APIs).
fn google_families(u: &Url) -> Vec<String> {
    if host_key(u) != "fonts.googleapis.com" {
        return vec![];
    }
    let mut out = Vec::new();
    for (k, v) in u.query_pairs() {
        if k != "family" {
            continue;
        }
        for fam in v.split('|') {
            let name = clean(fam.split(':').next().unwrap_or("").replace('+', " ").as_str());
            if !name.is_empty() && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

fn google_font_urls(css: &str) -> Vec<Url> {
    let mut out = Vec::new();
    let mut rest = css;
    while let Some(i) = rest.find("fonts.googleapis.com/css") {
        let start = rest[..i].rfind(['(', '"', '\'']).map(|j| j + 1).unwrap_or(i);
        let tail = &rest[start..];
        let end = tail.find([')', '"', '\'', ';', ' ', '\n']).unwrap_or(tail.len());
        let raw = &tail[..end];
        let raw = if raw.starts_with("//") { format!("https:{raw}") } else if raw.starts_with("http") { raw.to_string() } else { format!("https://{raw}") };
        if let Ok(u) = Url::parse(&raw) {
            out.push(u);
        }
        rest = &rest[i + 24..];
    }
    out
}

const NOT_BRAND_FONTS: &[&str] = &[
    "sans-serif", "serif", "monospace", "cursive", "fantasy", "math", "emoji", "fangsong", "system-ui", "ui-sans-serif", "ui-serif",
    "ui-monospace", "ui-rounded", "-apple-system", "apple-system", "blinkmacsystemfont", "segoe ui", "segoe ui emoji", "segoe ui symbol",
    "apple color emoji", "noto color emoji", "android emoji", "emojisymbols", "roboto", "helvetica", "helvetica neue", "arial", "arial black",
    "inherit", "initial", "unset", "revert", "revert-layer", "sf pro", "sf pro text", "sf pro display", "sf mono", "sfmono-regular", "menlo", "monaco",
    "consolas", "courier", "courier new", "liberation mono", "liberation sans", "dejavu sans", "dejavu sans mono", "oxygen", "oxygen-sans", "ubuntu",
    "cantarell", "droid sans", "times", "times new roman", "verdana", "tahoma", "lucida grande", "lucida console", "icons", "fontawesome",
    "font awesome", "font awesome 5 free", "font awesome 6 free", "material icons", "material symbols outlined", "noto sans", "noto serif",
];

fn font_ok(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    !name.is_empty()
        && !NOT_BRAND_FONTS.contains(&l.as_str())
        && !l.starts_with("var(")
        && !l.contains("icon")
        && !l.contains("awesome")
        && name.chars().next().is_some_and(|c| c.is_alphabetic())
}

/// The first non-generic family of a `font-family` value. `allow` names
/// (lowercase) count even when they are usually system fonts, e.g. Roboto
/// loaded from Google Fonts.
fn first_family(value: &str, allow: &[String]) -> Option<String> {
    let v = value.trim().trim_end_matches("!important").trim();
    if v.starts_with("var(") {
        return None;
    }
    for part in v.split(',') {
        let name = clean(part.trim().trim_matches(['"', '\'']));
        let lower = name.to_ascii_lowercase();
        let name = if lower.ends_with(" var") || lower.ends_with(" variable") {
            name.rsplit_once(' ').map(|(a, _)| a.to_string()).unwrap_or(name)
        } else {
            name
        };
        if font_ok(&name) || allow.contains(&name.to_ascii_lowercase()) {
            return Some(name);
        }
    }
    None
}

// ---------- CSS ----------

/// Walk a stylesheet's declarations: `f(selector, property, value)`.
fn declarations(css: &str, mut f: impl FnMut(&str, &str, &str)) {
    let css = strip_comments(css);
    let mut buf = String::new();
    let mut selector = String::new();
    for c in css.chars() {
        match c {
            '{' => {
                selector = buf.trim().to_ascii_lowercase();
                buf.clear();
            }
            ';' | '}' => {
                if let Some((p, v)) = buf.split_once(':') {
                    let p = p.trim();
                    if !p.is_empty() && !p.contains(' ') && !p.starts_with('@') {
                        f(&selector, &p.to_ascii_lowercase(), v.trim());
                    }
                }
                buf.clear();
            }
            _ => buf.push(c),
        }
    }
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        match rest[i + 2..].find("*/") {
            Some(j) => rest = &rest[i + 2 + j + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rgb(u8, u8, u8);

impl Rgb {
    fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
    /// (saturation, lightness) in HSL.
    fn sl(&self) -> (f64, f64) {
        let (r, g, b) = (self.0 as f64 / 255.0, self.1 as f64 / 255.0, self.2 as f64 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        let d = max - min;
        let s = if d == 0.0 { 0.0 } else { d / (1.0 - (2.0 * l - 1.0).abs()) };
        (s, l)
    }
    fn dist(&self, o: &Rgb) -> f64 {
        let d = |a: u8, b: u8| (a as f64 - b as f64).powi(2);
        (d(self.0, o.0) + d(self.1, o.1) + d(self.2, o.2)).sqrt()
    }
    fn brandish(&self) -> bool {
        let (s, l) = self.sl();
        s >= 0.25 && (0.15..=0.85).contains(&l)
    }
}

/// Every colour written in a CSS value: `#rgb[a]`, `#rrggbb[aa]`, `rgb()`,
/// `rgba()`, `hsl()`, `hsla()` and `oklch()`. Mostly transparent colours are skipped.
fn colors_in(value: &str) -> Vec<Rgb> {
    let mut out = Vec::new();
    let b = value.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'#' {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_hexdigit() {
                j += 1;
            }
            let boundary = j >= b.len() || !(b[j].is_ascii_alphanumeric() || b[j] == b'-' || b[j] == b'_');
            if boundary {
                if let Some(c) = parse_hex(&value[i + 1..j]) {
                    out.push(c);
                }
            }
            i = j.max(i + 1);
            continue;
        }
        let prev_ok = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'-');
        if prev_ok && matches!(b[i], b'r' | b'R' | b'h' | b'H' | b'o' | b'O') {
            let rest = &value[i..];
            let lower = rest.get(..6).unwrap_or(rest).to_ascii_lowercase();
            for (name, kind) in [("rgba(", 0), ("rgb(", 0), ("hsla(", 1), ("hsl(", 1), ("oklch(", 2)] {
                if lower.starts_with(name) {
                    if let Some(end) = rest.find(')') {
                        let args = &rest[name.len()..end];
                        let c = match kind {
                            0 => parse_rgb_fn(args),
                            1 => parse_hsl_fn(args),
                            _ => parse_oklch_fn(args),
                        };
                        if let Some(c) = c {
                            out.push(c);
                        }
                        i += end;
                    }
                    break;
                }
            }
        }
        i += 1;
    }
    out
}

fn parse_hex(h: &str) -> Option<Rgb> {
    let x = |s: &str| u8::from_str_radix(s, 16).ok();
    let (rgb, alpha) = match h.len() {
        3 | 4 => {
            let d: Vec<u8> = h.chars().map(|c| x(&format!("{c}{c}"))).collect::<Option<_>>()?;
            (Rgb(d[0], d[1], d[2]), d.get(3).copied().unwrap_or(255))
        }
        6 | 8 => (Rgb(x(&h[0..2])?, x(&h[2..4])?, x(&h[4..6])?), if h.len() == 8 { x(&h[6..8])? } else { 255 }),
        _ => return None,
    };
    (alpha >= 128).then_some(rgb)
}

fn fn_args(args: &str) -> Option<(Vec<&str>, f64)> {
    if args.contains("from ") || args.contains("var(") || args.contains("calc(") {
        return None;
    }
    let (main, alpha) = match args.split_once('/') {
        Some((m, a)) => (m, Some(a.trim())),
        None => (args, None),
    };
    let mut parts: Vec<&str> = main.split([',', ' ', '\t', '\n']).filter(|p| !p.is_empty()).collect();
    let alpha = match alpha {
        Some(a) => Some(a),
        None if parts.len() == 4 => parts.pop(),
        None => None,
    };
    let alpha = alpha.map(|a| num(a, 1.0)).unwrap_or(1.0);
    (parts.len() == 3).then_some((parts, alpha))
}

/// A number, with `%` scaled so that 100% equals `pct_of`.
fn num(s: &str, pct_of: f64) -> f64 {
    let s = s.trim();
    if let Some(p) = s.strip_suffix('%') {
        p.parse::<f64>().unwrap_or(0.0) / 100.0 * pct_of
    } else {
        s.trim_end_matches("deg").parse::<f64>().unwrap_or(0.0)
    }
}

fn clamp8(v: f64) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

fn parse_rgb_fn(args: &str) -> Option<Rgb> {
    let (p, a) = fn_args(args)?;
    (a >= 0.5).then(|| Rgb(clamp8(num(p[0], 255.0)), clamp8(num(p[1], 255.0)), clamp8(num(p[2], 255.0))))
}

fn parse_hsl_fn(args: &str) -> Option<Rgb> {
    let (p, a) = fn_args(args)?;
    if a < 0.5 {
        return None;
    }
    let h = num(p[0], 360.0).rem_euclid(360.0) / 360.0;
    let s = (num(p[1], 1.0)).clamp(0.0, 1.0);
    let l = (num(p[2], 1.0)).clamp(0.0, 1.0);
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let pp = 2.0 * l - q;
    let hue = |mut t: f64| {
        t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            pp + (q - pp) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            pp + (q - pp) * (2.0 / 3.0 - t) * 6.0
        } else {
            pp
        }
    };
    Some(Rgb(clamp8(hue(h + 1.0 / 3.0) * 255.0), clamp8(hue(h) * 255.0), clamp8(hue(h - 1.0 / 3.0) * 255.0)))
}

fn parse_oklch_fn(args: &str) -> Option<Rgb> {
    let (p, a) = fn_args(args)?;
    if a < 0.5 {
        return None;
    }
    let l = num(p[0], 1.0);
    let c = num(p[1], 0.4);
    let h = num(p[2], 360.0).to_radians();
    let (la, lb) = (c * h.cos(), c * h.sin());
    let l_ = (l + 0.396_337_777_4 * la + 0.215_803_757_3 * lb).powi(3);
    let m_ = (l - 0.105_561_345_8 * la - 0.063_854_172_8 * lb).powi(3);
    let s_ = (l - 0.089_484_177_5 * la - 1.291_485_548 * lb).powi(3);
    let r = 4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_;
    let g = -1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_;
    let b = -0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701 * s_;
    let gamma = |x: f64| {
        let x = x.clamp(0.0, 1.0);
        if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    };
    Some(Rgb(clamp8(gamma(r) * 255.0), clamp8(gamma(g) * 255.0), clamp8(gamma(b) * 255.0)))
}

/// Colour and font statistics over a set of stylesheets.
#[derive(Default)]
struct CssStats {
    colors: HashMap<Rgb, (u32, usize)>, // count, first seen
    fonts: HashMap<String, (u32, usize)>,
    seen: usize,
    /// Lowercase family names loaded from Google Fonts.
    allow: Vec<String>,
}

impl std::hash::Hash for Rgb {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.0, self.1, self.2).hash(state)
    }
}
impl Eq for Rgb {}

impl CssStats {
    fn add(&mut self, css: &str) {
        declarations(css, |selector, prop, value| {
            let weight = if prop.starts_with("--") && ["primary", "brand", "accent"].iter().any(|w| prop.contains(w)) { 4 } else { 1 };
            for c in colors_in(value) {
                self.seen += 1;
                let e = self.colors.entry(c).or_insert((0, self.seen));
                e.0 += weight;
            }
            let is_font = prop == "font-family" || (prop.starts_with("--") && prop.contains("font") && !prop.contains("weight") && !prop.contains("size"));
            if is_font {
                if let Some(name) = first_family(value, &self.allow) {
                    // Weight by role: the body/sans stack is the brand's main
                    // font; code and decorative accent fonts count for little.
                    let lower = value.to_ascii_lowercase();
                    let mut w = 4;
                    if lower.contains("monospace") || lower.contains("cursive") || lower.contains("fantasy") || prop.contains("mono") || prop.contains("code") {
                        w = 1;
                    } else {
                        let main_sel = selector.split(',').any(|s| matches!(s.trim(), "body" | "html" | ":root" | ":host" | "h1" | "h2"));
                        let main_prop = ["sans", "body", "heading", "display", "default-font-family", "base", "main", "primary", "text"].iter().any(|k| prop.starts_with("--") && prop.contains(k));
                        if main_sel || main_prop {
                            w += 12;
                        }
                    }
                    self.seen += 1;
                    let e = self.fonts.entry(name).or_insert((0, self.seen));
                    e.0 += w;
                }
            }
        });
    }

    fn ranked_colors(&self) -> Vec<Rgb> {
        let mut v: Vec<_> = self.colors.iter().filter(|(c, _)| c.brandish()).map(|(c, (n, first))| (*c, *n, *first)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
        v.into_iter().map(|(c, _, _)| c).collect()
    }

    /// Fonts by weighted use; Google Fonts link order breaks ties, and
    /// loaded families the CSS never names come last.
    fn ranked_fonts(&self, google: &[String]) -> Vec<String> {
        let gpos = |f: &str| google.iter().position(|g| g.eq_ignore_ascii_case(f)).unwrap_or(usize::MAX);
        let mut v: Vec<(String, u32, usize, usize)> = self.fonts.iter().map(|(f, (n, first))| (f.clone(), *n, gpos(f), *first)).collect();
        for g in google {
            if !v.iter().any(|(f, ..)| f.eq_ignore_ascii_case(g)) {
                v.push((g.clone(), 0, gpos(g), usize::MAX));
            }
        }
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)).then(a.3.cmp(&b.3)));
        v.into_iter().map(|(f, ..)| f).collect()
    }
}

// ---------- aggregation ----------

/// Build the brand from crawled pages (URL + raw HTML) and the home page's
/// fetched stylesheets. The home page is the one with the shortest path.
pub fn build(pages: &[(Url, String)], css: &[String]) -> Brand {
    let hs: Vec<(Url, PageBrandHints)> = pages.iter().map(|(u, html)| (u.clone(), hints(u, html))).collect();
    build_from_hints(hs, css)
}

fn path_rank(u: &Url) -> (usize, usize) {
    (u.path().trim_end_matches('/').len(), u.query().map(str::len).unwrap_or(0))
}

/// Same as [`build`], from hints already extracted.
pub fn build_from_hints(mut pages: Vec<(Url, PageBrandHints)>, css: &[String]) -> Brand {
    pages.sort_by_key(|(u, _)| path_rank(u));
    let mut b = Brand { updated_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true), ..Default::default() };
    let Some((home_url, home)) = pages.first() else { return b };

    b.site_name = home
        .og_site_name
        .clone()
        .or_else(|| home.ld_name.clone())
        .or_else(|| pages.iter().find_map(|(_, h)| h.ld_name.clone()))
        .or_else(|| home.title.as_deref().map(clean_title))
        .unwrap_or_else(|| home_url.host_str().unwrap_or("").trim_start_matches("www.").to_string());
    b.tagline = home.og_description.clone().or_else(|| home.meta_description.clone()).unwrap_or_default();
    b.logo = home
        .ld_logo
        .clone()
        .or_else(|| pages.iter().find_map(|(_, h)| h.ld_logo.clone()))
        .or_else(|| home.img_logo.clone())
        .or_else(|| {
            // The same header logo on most pages.
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for (_, h) in &pages {
                if let Some(l) = &h.img_logo {
                    *counts.entry(l.as_str()).or_default() += 1;
                }
            }
            counts.into_iter().max_by_key(|(_, n)| *n).map(|(l, _)| l.to_string())
        });
    b.icon = home.icons.iter().max_by_key(|(_, s)| *s).map(|(u, _)| u.clone()).or_else(|| {
        let mut o = home_url.clone();
        o.set_path("/favicon.ico");
        o.set_query(None);
        o.set_fragment(None);
        Some(o.to_string())
    });
    b.cover = home.og_image.clone().or_else(|| home.twitter_image.clone());

    // Colours: declared theme colours first, then the most frequent saturated ones.
    let mut google: Vec<String> = home.google_fonts.clone();
    for c in css {
        for u in google_font_urls(c) {
            google.extend(google_families(&u));
        }
    }
    google.retain(|f| f.eq_ignore_ascii_case("roboto") || font_ok(f));
    let mut stats = CssStats { allow: google.iter().map(|g| g.to_ascii_lowercase()).collect(), ..Default::default() };
    stats.add(&home.inline_css);
    for c in css {
        stats.add(c);
    }
    let mut picked: Vec<Rgb> = Vec::new();
    let take = |c: Rgb, picked: &mut Vec<Rgb>| {
        if picked.len() < MAX_COLORS && !picked.iter().any(|p| p.dist(&c) < 24.0) {
            picked.push(c);
        }
    };
    for t in &home.theme_colors {
        for c in colors_in(t) {
            if c.sl().0 >= 0.08 {
                take(c, &mut picked);
            }
        }
    }
    for c in stats.ranked_colors() {
        take(c, &mut picked);
    }
    b.colors = picked.iter().map(Rgb::hex).collect();

    // Fonts: by how much the CSS uses them (main font first); Google Fonts
    // link order breaks ties and fills in.
    let mut fonts: Vec<String> = Vec::new();
    for f in stats.ranked_fonts(&google) {
        if fonts.len() < MAX_FONTS && !fonts.iter().any(|x| x.eq_ignore_ascii_case(&f)) {
            fonts.push(f);
        }
    }
    b.fonts = fonts;

    // Imagery: each page's og:image (home first), then large content images.
    let mut seen: Vec<String> = b.logo.iter().cloned().collect();
    let mut add = |img: BrandImage, images: &mut Vec<BrandImage>| {
        if images.len() < MAX_IMAGES && !seen.contains(&img.url) && image_ok(&img.url) {
            seen.push(img.url.clone());
            images.push(img);
        }
    };
    for (u, h) in &pages {
        if let Some(og) = h.og_image.clone().or_else(|| h.twitter_image.clone()) {
            let alt = h.og_image_alt.clone().or_else(|| h.title.clone()).unwrap_or_default();
            add(BrandImage { url: og, alt, page: u.to_string() }, &mut b.images);
        }
    }
    for (_, h) in &pages {
        for img in &h.images {
            add(img.clone(), &mut b.images);
        }
    }

    // Social: the most linked profile per network.
    let mut social: HashMap<String, Vec<(String, usize, usize)>> = HashMap::new();
    let mut order = 0;
    for (_, h) in &pages {
        let mut page_seen: Vec<&str> = Vec::new();
        for s in &h.social {
            if page_seen.contains(&s.url.as_str()) {
                continue;
            }
            page_seen.push(&s.url);
            order += 1;
            let list = social.entry(s.network.clone()).or_default();
            let key = s.url.trim_end_matches('/').to_ascii_lowercase();
            match list.iter_mut().find(|(u, _, _)| u.trim_end_matches('/').to_ascii_lowercase() == key) {
                Some(e) => e.1 += 1,
                None => list.push((s.url.clone(), 1, order)),
            }
        }
    }
    let mut social: Vec<(String, String, usize)> = social
        .into_iter()
        .filter_map(|(net, list)| list.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.2.cmp(&a.2))).map(|(u, _, o)| (net, u, o)))
        .collect();
    social.sort_by_key(|(_, _, o)| *o);
    b.social = social.into_iter().map(|(network, url, _)| SocialLink { network, url }).collect();
    b
}

/// `Pricing | Acme` → `Pricing`; the part before the first separator.
fn clean_title(t: &str) -> String {
    let mut s = t;
    for sep in [" | ", " - ", " — ", " – ", " · ", " :: "] {
        if let Some((a, _)) = s.split_once(sep) {
            s = a;
        }
    }
    s.trim().to_string()
}

// ---------- from the crawl cache ----------

/// The cached pages of a project as (final URL, raw HTML), readable pages
/// only. `None` when the project has never been crawled.
pub fn cached_pages(store: &Store, slug: &str) -> Result<Option<Vec<(Url, String)>>> {
    let dir = store.home().cache_dir(slug).join("pages");
    if !dir.is_dir() {
        return Ok(None);
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let Ok(meta) = store.read_yaml::<CachedPage>(&path) else { continue };
        if meta.obstacle.is_some() || meta.status >= 400 {
            continue;
        }
        let Ok(html) = std::fs::read_to_string(path.with_extension("html")) else { continue };
        let Ok(url) = Url::parse(&meta.final_url).or_else(|_| Url::parse(&meta.url)) else { continue };
        out.push((url, html));
    }
    out.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    Ok(Some(out))
}

/// Fetch up to three stylesheets, 5 s and 2 MB each. Failures are skipped.
pub async fn fetch_stylesheets(urls: &[Url]) -> Vec<String> {
    let Ok(client) = reqwest::Client::builder().user_agent(crate::fetch::USER_AGENT).timeout(CSS_TIMEOUT).build() else { return vec![] };
    let jobs = urls.iter().take(MAX_STYLESHEETS).map(|u| {
        let client = client.clone();
        let u = u.clone();
        async move {
            let mut resp = client.get(u).header("Accept", "text/css,*/*;q=0.1").send().await.ok()?;
            if !resp.status().is_success() || resp.content_length().is_some_and(|n| n as usize > CSS_MAX_BYTES) {
                return None;
            }
            let mut body = Vec::new();
            while let Ok(Some(chunk)) = resp.chunk().await {
                body.extend_from_slice(&chunk);
                if body.len() > CSS_MAX_BYTES {
                    return None;
                }
            }
            Some(String::from_utf8_lossy(&body).into_owned())
        }
    });
    futures::future::join_all(jobs).await.into_iter().flatten().collect()
}

/// Build the brand from the crawl cache, fetching the home page's
/// stylesheets. Writes nothing. `None` when there is no usable cache.
pub async fn from_cache(store: &Store, slug: &str) -> Result<Option<Brand>> {
    let store2 = store.clone();
    let slug2 = slug.to_string();
    let hs = tokio::task::spawn_blocking(move || -> Result<Option<Vec<(Url, PageBrandHints)>>> {
        let Some(pages) = cached_pages(&store2, &slug2)? else { return Ok(None) };
        if pages.is_empty() {
            return Ok(None);
        }
        Ok(Some(pages.iter().map(|(u, html)| (u.clone(), hints(u, html))).collect()))
    })
    .await??;
    let Some(hs) = hs else { return Ok(None) };
    let sheets = hs.iter().min_by_key(|(u, _)| path_rank(u)).map(|(_, h)| h.stylesheets.clone()).unwrap_or_default();
    let css = fetch_stylesheets(&sheets).await;
    Ok(Some(tokio::task::spawn_blocking(move || build_from_hints(hs, &css)).await?))
}

/// Rebuild from the cache, save `profile/brand.yaml`, re-render PROFILE.md,
/// commit and emit `brand_updated`. `automated` adds the `[auto]` prefix.
pub async fn refresh(store: &Store, bus: &EventBus, project: &Project, automated: bool) -> Result<Option<Brand>> {
    let slug = &project.slug;
    let Some(brand) = from_cache(store, slug).await? else { return Ok(None) };
    brand.save(store, slug)?;
    crate::profile::write_markdown(store, slug, &project.name, &project.url)?;
    let prefix = if automated { "[auto] " } else { "" };
    store.commit(&format!("{prefix}brand({slug}): {}", brand.summary())).await?;
    bus.emit(Some(slug), EventKind::BrandUpdated { logo: brand.logo.clone(), colors: brand.colors.clone(), images: brand.images.len() });
    Ok(Some(brand))
}

/// The short "Brand" block in PROFILE.md.
pub fn render_markdown(b: &Brand) -> String {
    let mut md = String::from("\n## Brand\n\n");
    if !b.site_name.is_empty() {
        md.push_str(&format!("- Name: {}\n", b.site_name));
    }
    if !b.tagline.is_empty() {
        md.push_str(&format!("- Tagline: {}\n", b.tagline));
    }
    if let Some(l) = &b.logo {
        md.push_str(&format!("- Logo: {l}\n"));
    }
    if let Some(i) = &b.icon {
        md.push_str(&format!("- Icon: {i}\n"));
    }
    if let Some(c) = &b.cover {
        md.push_str(&format!("- Cover image: {c}\n"));
    }
    if !b.colors.is_empty() {
        md.push_str(&format!("- Colours: {}\n", b.colors.join(", ")));
    }
    if !b.fonts.is_empty() {
        md.push_str(&format!("- Fonts: {}\n", b.fonts.join(", ")));
    }
    for s in &b.social {
        md.push_str(&format!("- {}: {}\n", s.network, s.url));
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = r##"<!doctype html><html><head>
<title>Acme Rockets | Fast launches for small teams</title>
<meta name="description" content="Meta description.">
<meta property="og:description" content="Rockets for everyone.">
<meta property="og:image" content="/img/og.png">
<meta name="theme-color" content="#FF5A1F">
<link rel="icon" href="/favicon-32.png" sizes="32x32" type="image/png">
<link rel="apple-touch-icon" href="/apple-touch-icon.png" sizes="180x180">
<link rel="preconnect" href="https://fonts.gstatic.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Caveat:wght@700&family=Space+Grotesk:wght@400;700&family=Inter:wght@400;600&display=swap">
<link rel="stylesheet" href="/assets/site.css">
<style>
  /* #abcdef in a comment is ignored */
  :root { --brand-primary: #1d4ed8; --grey: #888888; --font-sans: -apple-system, "Inter", sans-serif; --font-hand: "Caveat", cursive; }
  .title { font-family: "Space Grotesk", sans-serif; }
  code { font-family: "JetBrains Mono", monospace; }
  #add { color: rgb(29, 78, 216); }
  .btn { background: #1d4ed8; border-color: #1e4fd9; }
  .tag { color: hsl(150, 70%, 40%); }
  body { font-family: "Inter", system-ui, sans-serif; color: #111; }
</style>
</head><body>
<header><a href="/"><img src="/static/acme-logo.svg" alt="Acme"></a>
<nav><a href="/pricing">Pricing</a></nav></header>
<main>
  <section class="hero-banner"><img src="/img/hero.jpg" alt="A rocket at dawn"></section>
  <img src="/img/team-small.jpg" srcset="/img/team-800.jpg 800w, /img/team-1600.jpg 1600w" alt="The team">
  <img src="/img/avatar-jane.jpg" width="600" height="600" alt="Jane">
  <img src="/img/tiny.png" alt="tiny">
  <img src="/logos/customer.png" width="400" alt="Customer">
  <p style="color: #22c55e">Go</p>
</main>
<footer>
  <a href="https://www.linkedin.com/company/acme/">LinkedIn</a>
  <a href="https://twitter.com/intent/tweet?text=hi">Share</a>
  <a href="https://twitter.com/acme">Twitter</a>
  <a href="https://github.com/acme">GitHub</a>
</footer>
</body></html>"##;

    const PRICING: &str = r##"<html><head><title>Pricing - Acme</title>
<meta property="og:image" content="https://cdn.acme.test/og-pricing.jpg">
<script type="application/ld+json">{"@context":"https://schema.org","@graph":[{"@type":"Organization","name":"Acme Inc","logo":{"@type":"ImageObject","url":"https://acme.test/brand/logo.png"}}]}</script>
</head><body><footer><a href="https://x.com/acme">X</a><a href="https://www.facebook.com/sharer/sharer.php?u=x">fb</a></footer></body></html>"##;

    fn pages() -> Vec<(Url, String)> {
        vec![
            (Url::parse("https://acme.test/pricing").unwrap(), PRICING.to_string()),
            (Url::parse("https://acme.test/").unwrap(), HOME.to_string()),
        ]
    }

    #[test]
    fn page_hints() {
        let base = Url::parse("https://acme.test/").unwrap();
        let h = hints(&base, HOME);
        assert_eq!(h.img_logo.as_deref(), Some("https://acme.test/static/acme-logo.svg"));
        assert_eq!(h.google_fonts, vec!["Caveat", "Space Grotesk", "Inter"]);
        assert_eq!(h.stylesheets, vec![Url::parse("https://acme.test/assets/site.css").unwrap()]);
        assert_eq!(h.og_image.as_deref(), Some("https://acme.test/img/og.png"));
        let urls: Vec<&str> = h.images.iter().map(|i| i.url.as_str()).collect();
        assert_eq!(urls, vec!["https://acme.test/img/hero.jpg", "https://acme.test/img/team-1600.jpg"]);
    }

    #[test]
    fn builds_brand() {
        let css = vec![".x{color:#e11d48} .y{color:#e11d48} .z{color:oklch(62.7% .194 149.214)} .w{font-family:'Playfair Display',serif}".to_string()];
        let b = build(&pages(), &css);
        assert_eq!(b.site_name, "Acme Inc"); // no og:site_name on home, JSON-LD on another page
        assert_eq!(b.tagline, "Rockets for everyone.");
        assert_eq!(b.logo.as_deref(), Some("https://acme.test/brand/logo.png"));
        assert_eq!(b.icon.as_deref(), Some("https://acme.test/apple-touch-icon.png"));
        assert_eq!(b.cover.as_deref(), Some("https://acme.test/img/og.png"));
        assert_eq!(b.colors[0], "#ff5a1f");
        assert_eq!(b.colors[1], "#1d4ed8"); // weighted custom property + repeats; #1e4fd9 deduped
        assert!(b.colors.contains(&"#e11d48".to_string()));
        assert!(!b.colors.iter().any(|c| c == "#888888" || c == "#111111" || c == "#1e4fd9"));
        assert!(b.colors.len() <= 6);
        // The body font first; the decorative Caveat accent and the code font last.
        assert_eq!(b.fonts, vec!["Inter", "Space Grotesk", "Playfair Display"]);
        let imgs: Vec<&str> = b.images.iter().map(|i| i.url.as_str()).collect();
        assert_eq!(
            imgs,
            vec!["https://acme.test/img/og.png", "https://cdn.acme.test/og-pricing.jpg", "https://acme.test/img/hero.jpg", "https://acme.test/img/team-1600.jpg"]
        );
        assert_eq!(b.images[1].page, "https://acme.test/pricing");
        let social: Vec<(&str, &str)> = b.social.iter().map(|s| (s.network.as_str(), s.url.as_str())).collect();
        assert_eq!(
            social,
            vec![("linkedin", "https://www.linkedin.com/company/acme/"), ("x", "https://twitter.com/acme"), ("github", "https://github.com/acme")]
        );
        let v = serde_json::to_value(&b).unwrap();
        for k in ["site_name", "tagline", "logo", "icon", "cover", "colors", "fonts", "images", "social", "updated_at"] {
            assert!(v.get(k).is_some(), "{k}");
        }
    }

    #[test]
    fn fallbacks() {
        let html = "<html><head><title>Widgets — Home</title></head><body><img src='/a.png'></body></html>";
        let b = build(&[(Url::parse("https://w.test/").unwrap(), html.to_string())], &[]);
        assert_eq!(b.site_name, "Widgets");
        assert_eq!(b.icon.as_deref(), Some("https://w.test/favicon.ico"));
        assert_eq!(b.logo, None);
        assert_eq!(b.cover, None);
        assert!(b.colors.is_empty());
    }

    #[test]
    fn colour_parsing() {
        assert_eq!(colors_in("#0000"), vec![]);
        assert_eq!(colors_in("“quoted” red rgb(1,2,3)"), vec![Rgb(1, 2, 3)]);
        assert_eq!(colors_in("#ff2d78 rgba(255, 0, 0, .2)"), vec![Rgb(255, 45, 120)]);
        assert_eq!(colors_in("rgb(10 20 30 / 80%)"), vec![Rgb(10, 20, 30)]);
        assert_eq!(colors_in("hsl(0, 100%, 50%)"), vec![Rgb(255, 0, 0)]);
        assert_eq!(colors_in("oklch(62.8% 0.2577 29.23)"), vec![Rgb(255, 0, 0)]);
        assert_eq!(first_family("-apple-system, BlinkMacSystemFont, \"SF Pro Text\", \"Inter\", sans-serif", &[]).as_deref(), Some("Inter"));
        assert_eq!(first_family("JetBrains Mono Var, monospace", &[]).as_deref(), Some("JetBrains Mono"));
    }
}
