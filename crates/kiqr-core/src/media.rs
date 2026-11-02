//! Media: the business's own pictures, video, audio and documents, kept as
//! assets of type `media` under the Library. They come from sources: the
//! website (built in: what the brand crawl found, fetched only from the
//! business's own hosts), and any ability that declares a `media:` block (a
//! folder, a drive, a bucket), later uploads. KIQR knows the shape of an item
//! (an id, a name, a type, a size, when it changed, a URL) and nothing about
//! any service: the manifest says which request lists the items and which
//! fetches one.
//!
//! ```yaml
//! media:
//!   list:
//!     request: { method: POST, url: "https://api.example.com/files/list", json: { path: "{{folder}}" } }
//!     items: /entries                 # the array in the answer; empty: the answer is the array
//!     id: /id                         # pointers into an entry
//!     name: /name
//!     mime: /mime_type                # optional; sniffed from the bytes when missing
//!     bytes: /size                    # optional
//!     modified: /server_modified      # optional
//!     url: /thumbnail_url             # optional: a picture of the item for the picker
//!   get:
//!     request: { method: POST, url: "https://content.example.com/files/download", headers: { "X-Path": "{{id}}" } }
//!     # The answer is the file itself, or `result: { file_url: /link }` / `file_b64`.
//! ```
//!
//! The website's items come from the brand (`website_items`) and its `get` is
//! `fetch_own`, which refuses any host that is not the business's own.

use crate::assets::{self, Asset, AssetFile, NewAsset};
use crate::brand::{self, Brand};
use crate::extensions::{fill, pointer, HttpRequest, Provide};
use crate::store::Store;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Duration;
use url::Url;

/// The built-in source's id.
pub const WEBSITE: &str = "website";
/// The asset type media lands in.
pub const TYPE: &str = "media";
/// A fetched file may be this big.
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const TIMEOUT: Duration = Duration::from_secs(15);
/// Smaller on both sides is an icon or a thumbnail, not a picture.
pub const MIN_SIDE: u32 = 400;
/// How many of the site's pictures are brought in on their own.
pub const SITE_LIMIT: usize = 12;

/// `media:` on a manifest: how to list a source's items and fetch one.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MediaSpec {
    pub list: MediaList,
    pub get: Provide,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MediaList {
    pub request: HttpRequest,
    /// Pointer to the array of items; empty: the answer is the array.
    pub items: String,
    pub id: String,
    pub name: String,
    pub mime: String,
    pub bytes: String,
    pub modified: String,
    /// A URL that shows the item (a thumbnail, the file itself when public).
    pub url: String,
}

/// One thing a source offers.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MediaItem {
    pub source: String,
    pub id: String,
    pub name: String,
    pub mime: String,
    pub bytes: Option<u64>,
    pub modified: Option<String>,
    pub url: Option<String>,
    /// What it shows, when the source says (an alt text).
    pub alt: String,
    /// The page it was seen on (the website).
    pub page: Option<String>,
    /// The media asset it already is, when imported.
    pub imported: Option<String>,
    /// The site's share image: listed first, the picture of the business as a whole.
    pub cover: bool,
}

pub fn validate(s: &MediaSpec) -> Result<()> {
    if s.list.request.url.trim().is_empty() {
        bail!("media: `list` needs a request");
    }
    if s.list.id.trim().is_empty() || s.list.name.trim().is_empty() {
        bail!("media: `list` needs `id` and `name` pointers into an item");
    }
    if s.get.request.is_none() && s.get.steps.is_empty() && s.get.command.is_none() && s.get.tool.is_none() {
        bail!("media: `get` needs a request (or steps, a command or a tool) that fetches one item by `{{id}}`");
    }
    Ok(())
}

/// The items in a `list` answer, by the spec's pointers.
pub fn items(spec: &MediaList, answer: &Value, source: &str) -> Vec<MediaItem> {
    let list = match pointer(answer, &spec.items) {
        Some(Value::Array(a)) => a.clone(),
        _ => vec![],
    };
    let text = |e: &Value, p: &str| -> Option<String> {
        if p.trim().is_empty() {
            return None;
        }
        pointer(e, p).map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string().trim_matches('"').to_string())).filter(|s| !s.is_empty())
    };
    list.iter()
        .filter_map(|e| {
            let id = text(e, &spec.id)?;
            let name = text(e, &spec.name).unwrap_or_else(|| id.clone());
            Some(MediaItem {
                source: source.to_string(),
                id,
                name,
                mime: text(e, &spec.mime).unwrap_or_default(),
                bytes: text(e, &spec.bytes).and_then(|s| s.parse().ok()),
                modified: text(e, &spec.modified),
                url: text(e, &spec.url),
                ..Default::default()
            })
        })
        .collect()
}

/// The arguments a `get` request is filled with for one item.
pub fn get_args(item: &MediaItem) -> Value {
    json!({ "id": item.id, "name": item.name, "url": item.url, "mime": item.mime })
}

/// A short id for a URL: the first 12 hex of its hash.
pub fn url_id(url: &str) -> String {
    Sha256::digest(url.as_bytes()).iter().take(6).map(|b| format!("{b:02x}")).collect()
}

/// What the website offers: the home page's share image first, then the
/// pictures the brand crawl kept, each with the page it was seen on.
pub fn website_items(brand: &Brand, site_url: &str) -> Vec<MediaItem> {
    let mut out: Vec<MediaItem> = vec![];
    let mut seen: Vec<String> = vec![];
    let mut push = |url: &str, alt: &str, page: &str, cover: bool| {
        let key = brand::image_key_of(url);
        if seen.contains(&key) || !brand::image_ok(url) {
            return;
        }
        seen.push(key);
        out.push(MediaItem {
            source: WEBSITE.into(),
            id: url_id(url),
            name: file_name_of(url),
            url: Some(url.to_string()),
            alt: alt.trim().to_string(),
            page: Some(page.to_string()),
            cover,
            ..Default::default()
        });
    };
    if let Some(c) = &brand.cover {
        // The crawl kept the share image's own alt when the page had one; the site's name otherwise.
        let known = brand.images.iter().find(|i| brand::image_key_of(&i.url) == brand::image_key_of(c)).filter(|i| !i.alt.trim().is_empty());
        push(c, known.map(|i| i.alt.as_str()).unwrap_or(&brand.site_name), known.map(|i| i.page.as_str()).unwrap_or(site_url), true);
    }
    for i in &brand.images {
        push(&i.url, &i.alt, &i.page, false);
    }
    out
}

/// The file name in a URL's path, for a title when there is no alt.
pub fn file_name_of(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or("").trim();
    if name.is_empty() {
        "picture".into()
    } else {
        percent_decode(name)
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Why a URL may not be fetched as the website's: not the business's own
/// host, not http(s), or not a picture. None: fine.
pub fn refuse(url: &str, own: &[String]) -> Option<String> {
    let Ok(u) = Url::parse(url.trim()) else { return Some(format!("{url} is not a URL")) };
    if !matches!(u.scheme(), "http" | "https") {
        return Some(format!("{url} is not a web address"));
    }
    let host = u.host_str().unwrap_or("");
    if !brand::is_own_host(host, own) {
        let site = own.first().cloned().unwrap_or_default();
        return Some(format!("{host} is not the business's own website ({site}, or a host its logo and pictures live on). Media comes only from the business's own properties: its site, a connected folder or drive, or an upload. For a picture from elsewhere, ask the user to add it."));
    }
    if !brand::image_ok(url) {
        return Some(format!("{url} looks like an icon, a logo or a tracking pixel, not a picture"));
    }
    None
}

/// Fetch one of the website's files: own hosts only, 15 s, 8 MB, a type
/// from the header or the bytes. Pictures under `MIN_SIDE` are refused.
pub async fn fetch_own(url: &str, own: &[String]) -> Result<(Vec<u8>, String)> {
    if let Some(why) = refuse(url, own) {
        bail!("{why}");
    }
    let client = reqwest::Client::builder().user_agent(crate::fetch::USER_AGENT).timeout(TIMEOUT).build()?;
    let mut resp = client.get(url).header("Accept", "image/*,*/*;q=0.5").send().await.with_context(|| format!("fetching {url}"))?;
    if !resp.status().is_success() {
        bail!("{url} answered {}", resp.status().as_u16());
    }
    if resp.content_length().is_some_and(|n| n as usize > MAX_BYTES) {
        bail!("{url} is over {} MB", MAX_BYTES / 1024 / 1024);
    }
    let ctype = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_BYTES {
            bail!("{url} is over {} MB", MAX_BYTES / 1024 / 1024);
        }
    }
    if body.is_empty() {
        bail!("{url} is empty");
    }
    let sniffed = crate::fetch::sniff_mime(&body);
    let mime = if sniffed != "application/octet-stream" { sniffed } else if ctype.starts_with("image/") || ctype.starts_with("video/") || ctype.starts_with("audio/") || ctype == "application/pdf" { ctype } else { sniffed };
    check_file(&body, &mime)?;
    Ok((body, mime))
}

/// What may become media: a known type, and a picture at least `MIN_SIDE` on one side.
pub fn check_file(bytes: &[u8], mime: &str) -> Result<()> {
    let ext = crate::fetch::extension_for(mime);
    if ext == "bin" || ext == "svg" || ext == "gif" {
        bail!("{mime} is not a kind of media KIQR keeps");
    }
    if mime.starts_with("image/") {
        match assets::image_size(bytes) {
            Some((w, h)) if w >= MIN_SIDE || h >= MIN_SIDE => {}
            Some((w, h)) => bail!("the picture is {w}×{h}, too small to use"),
            None => bail!("the picture could not be read"),
        }
    }
    Ok(())
}

/// The media asset an item already is: same source item, same URL, or the same bytes.
pub fn find_imported<'a>(all: &'a [Asset], item: &MediaItem, sha256: Option<&str>) -> Option<&'a Asset> {
    let s = |a: &Asset, k: &str| a.source.get(k).and_then(|v| v.as_str()).map(str::to_string);
    all.iter().filter(|a| a.kind == TYPE && !a.archived).find(|a| {
        (s(a, "origin").as_deref() == Some(item.source.as_str()) && s(a, "item").as_deref() == Some(item.id.as_str()))
            || (item.url.is_some() && s(a, "url") == item.url)
            || sha256.is_some_and(|h| a.files.iter().any(|f| f.sha256 == h))
    })
}

/// Mark each item with the media asset it already is.
pub fn mark_imported(items: &mut [MediaItem], all: &[Asset]) {
    for it in items.iter_mut() {
        it.imported = find_imported(all, it, None).map(|a| a.id.clone());
    }
}

/// Put a fetched file in the Library as a media asset, unless the same item
/// or the same bytes are there already (then that asset comes back).
pub fn import(store: &Store, slug: &str, item: &MediaItem, bytes: &[u8], mime: &str, by: &str) -> Result<(Asset, bool)> {
    check_file(bytes, mime)?;
    let sha: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
    let all = assets::list(store, slug)?;
    if let Some(a) = find_imported(&all, item, Some(&sha)) {
        return Ok((a.clone(), false));
    }
    let ext = crate::fetch::extension_for(mime);
    let role = assets::find_type(store, TYPE).map(|t| t.role_for(ext)).unwrap_or_else(|| "image".into());
    let stem = {
        let s = assets::slugify(item.name.rsplit_once('.').map(|(a, _)| a).unwrap_or(&item.name));
        if s.is_empty() { role.clone() } else { s }
    };
    let title = {
        let alt = crate::profile::truncate_chars(item.alt.trim(), 80);
        if !alt.is_empty() {
            alt
        } else {
            let n = item.name.rsplit_once('.').map(|(a, _)| a).unwrap_or(&item.name).replace(['-', '_'], " ");
            let n = n.trim().to_string();
            if n.is_empty() { format!("{} from {}", role, item.source) } else { n }
        }
    };
    let mut source = serde_json::Map::new();
    source.insert("origin".into(), json!(item.source));
    source.insert("item".into(), json!(item.id));
    if let Some(u) = &item.url {
        source.insert("url".into(), json!(u));
    }
    if let Some(p) = &item.page {
        source.insert("page".into(), json!(p));
    }
    if !item.alt.trim().is_empty() {
        source.insert("alt".into(), json!(item.alt.trim()));
    }
    if let Some(m) = &item.modified {
        source.insert("modified".into(), json!(m));
    }
    if item.cover {
        source.insert("cover".into(), json!(true));
    }
    source.insert("imported_at".into(), json!(chrono::Utc::now().to_rfc3339()));
    let a = assets::create(store, slug, NewAsset { kind: TYPE.into(), title, by: Some(by.to_string()), source, ..Default::default() })?;
    let a = assets::write_file(store, slug, TYPE, &a.id, &role, &format!("{stem}.{ext}"), bytes)?;
    Ok((a, true))
}

/// The pictures in the Library's Media, for the renderer: each media asset's
/// image file, the site's share image first, then the biggest first.
pub fn pictures(store: &Store, slug: &str) -> Result<Vec<(Asset, AssetFile)>> {
    let mut out: Vec<(Asset, AssetFile)> = assets::list(store, slug)?
        .into_iter()
        .filter(|a| a.kind == TYPE && !a.archived)
        .filter_map(|a| a.files.iter().find(|f| f.role == "image").cloned().map(|f| (a, f)))
        .collect();
    let cover = |a: &Asset| a.source.get("cover").and_then(|v| v.as_bool()).unwrap_or(false);
    out.sort_by(|(a, fa), (b, fb)| cover(b).cmp(&cover(a)).then((fb.width.unwrap_or(0) * fb.height.unwrap_or(0)).cmp(&(fa.width.unwrap_or(0) * fa.height.unwrap_or(0)))));
    Ok(out)
}

/// A picture as Seemo and the posts job see it.
pub fn picture_json(store: &Store, slug: &str, a: &Asset, f: &AssetFile) -> Value {
    let path = store.home().assets_dir(slug).join(&a.kind).join(&a.id).join(&f.path);
    json!({
        "asset": a.id,
        "title": a.title,
        "file": format!("file://{}", path.display()),
        "url": format!("/api/projects/{slug}/assets/{}/{}/files/{}", a.kind, a.id, f.path),
        "alt": a.source.get("alt").and_then(|v| v.as_str()).unwrap_or(&a.title),
        "page": a.source.get("page").and_then(|v| v.as_str()),
        "origin": a.source.get("origin").and_then(|v| v.as_str()).unwrap_or(""),
        "width": f.width,
        "height": f.height,
        "orientation": f.orientation(),
    })
}

/// Fill a `get` request's templates for one item (for the daemon's executor, which adds the keys).
pub fn fill_get(req: &HttpRequest, item: &MediaItem) -> HttpRequest {
    let args = get_args(item);
    let mut r = req.clone();
    r.url = fill(&r.url, &args, |_| None);
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Home;

    fn brand() -> Brand {
        Brand { site_name: "Acme".into(), logo: Some("https://cdn.shop.com/acme/logo.png".into()), cover: Some("https://acme.se/share.jpg".into()), images: vec![crate::brand::BrandImage { url: "https://acme.se/img/team-1200x800.jpg".into(), alt: "The team".into(), page: "https://acme.se/about".into() }], ..Default::default() }
    }

    #[test]
    fn own_hosts_and_refusals() {
        let own = brand::own_hosts("https://www.acme.se/", &brand());
        assert_eq!(own, vec!["acme.se", "cdn.shop.com"]);
        assert!(refuse("https://acme.se/a.jpg", &own).is_none());
        assert!(refuse("https://shop.acme.se/a.jpg", &own).is_none());
        assert!(refuse("https://cdn.shop.com/acme/hero.jpg", &own).is_none());
        let why = refuse("https://x.com/pic.jpg", &own).unwrap();
        assert!(why.contains("x.com is not the business's own website") && why.contains("acme.se"), "{why}");
        assert!(refuse("https://acme.se/icon.svg", &own).is_some());
        assert!(refuse("ftp://acme.se/a.jpg", &own).is_some());
    }

    #[test]
    fn website_items_put_the_cover_first_and_dedupe() {
        let items = website_items(&brand(), "https://acme.se/");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].url.as_deref(), Some("https://acme.se/share.jpg"));
        assert!(items[0].cover && items[0].alt == "Acme", "no alt known: the site's name");
        let mut b = brand();
        b.images.insert(0, crate::brand::BrandImage { url: "https://acme.se/share.jpg".into(), alt: "The workshop at dawn".into(), page: "https://acme.se/".into() });
        let items = website_items(&b, "https://acme.se/");
        assert_eq!((items.len(), items[0].alt.as_str()), (2, "The workshop at dawn"));
        assert_eq!(items[1].name, "team-1200x800.jpg");
        assert_eq!(items[1].page.as_deref(), Some("https://acme.se/about"));
        let spec = MediaList { items: "/entries".into(), id: "/id".into(), name: "/name".into(), bytes: "/size".into(), ..Default::default() };
        let got = items_from(&spec, json!({ "entries": [{ "id": "id:1", "name": "a.jpg", "size": 1234 }, { "name": "no id" }] }));
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].id.as_str(), got[0].bytes), ("id:1", Some(1234)));
    }

    fn items_from(spec: &MediaList, v: Value) -> Vec<MediaItem> {
        items(spec, &v, "dropbox")
    }

    #[test]
    fn import_makes_one_media_asset_per_file() {
        let dir = std::env::temp_dir().join(format!("kiqr-media-{}", uuid::Uuid::new_v4()));
        let store = Store::open(Home::at(&dir)).unwrap();
        assets::install_builtin_types(&store).unwrap();
        let png = {
            let mut p = resvg::tiny_skia::Pixmap::new(500, 300).unwrap();
            p.fill(resvg::tiny_skia::Color::from_rgba8(10, 20, 30, 255));
            p.encode_png().unwrap()
        };
        let item = MediaItem { source: WEBSITE.into(), id: "abc".into(), name: "Team-Photo.png".into(), url: Some("https://acme.se/team.png".into()), alt: "The team at work".into(), page: Some("https://acme.se/about".into()), ..Default::default() };
        let (a, new) = import(&store, "acme-se", &item, &png, "image/png", "kiqr").unwrap();
        assert!(new);
        assert_eq!((a.kind.as_str(), a.title.as_str()), ("media", "The team at work"));
        let f = &a.files[0];
        assert_eq!((f.role.as_str(), f.path.as_str(), f.width, f.height), ("image", "team-photo.png", Some(500), Some(300)));
        assert_eq!(a.source["origin"], "website");
        let (again, new) = import(&store, "acme-se", &item, &png, "image/png", "kiqr").unwrap();
        assert!(!new && again.id == a.id);
        let same_bytes = MediaItem { source: "dropbox".into(), id: "other".into(), name: "x.png".into(), ..Default::default() };
        assert!(!import(&store, "acme-se", &same_bytes, &png, "image/png", "kiqr").unwrap().1);
        let pics = pictures(&store, "acme-se").unwrap();
        assert_eq!(pics.len(), 1);
        let j = picture_json(&store, "acme-se", &pics[0].0, &pics[0].1);
        assert_eq!(j["orientation"], "landscape");
        assert!(j["file"].as_str().unwrap().starts_with("file://"));
        let tiny = resvg::tiny_skia::Pixmap::new(50, 50).unwrap().encode_png().unwrap();
        assert!(check_file(&tiny, "image/png").is_err());
        assert!(check_file(b"<svg/>", "image/svg+xml").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
