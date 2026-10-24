//! The built-in renderer behind "Make images": Seemo writes SVG (or HTML),
//! KIQR turns it into a PNG. SVG goes through resvg with the system's fonts
//! plus a bundled Inter, so it works on a fresh machine with nothing
//! installed. HTML goes through a Chrome found on the machine, headless, with
//! the network switched off, when there is one.

use anyhow::{anyhow, bail, Context, Result};
use resvg::{tiny_skia, usvg};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use usvg::fontdb;

const INTER: [(&str, u16, &[u8]); 4] = [
    ("Inter-Regular.ttf", 400, include_bytes!("../../../assets/fonts/Inter-Regular.ttf")),
    ("Inter-SemiBold.ttf", 600, include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf")),
    ("Inter-Bold.ttf", 700, include_bytes!("../../../assets/fonts/Inter-Bold.ttf")),
    ("Inter-ExtraBold.ttf", 800, include_bytes!("../../../assets/fonts/Inter-ExtraBold.ttf")),
];

/// Browsers that can take a screenshot headless, most common first.
const BROWSERS: &[&str] = &["chromium", "chromium-browser", "google-chrome", "google-chrome-stable", "brave", "brave-browser", "microsoft-edge", "chrome"];

pub const MAX_SIDE: u32 = 4096;

pub struct Renderer {
    db: Arc<fontdb::Database>,
    chrome: Option<PathBuf>,
    render_dir: PathBuf,
}

impl Renderer {
    /// Load fonts once (the system's and the bundled Inter) and look for a browser.
    pub fn new(render_dir: PathBuf) -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        for (_, _, data) in INTER {
            db.load_font_data(data.to_vec());
        }
        db.set_sans_serif_family("Inter");
        // The browser cannot see fonts inside this binary: keep copies on disk for @font-face.
        let fonts = render_dir.join("fonts");
        for (name, _, data) in INTER {
            let p = fonts.join(name);
            if !p.exists() {
                let _ = std::fs::create_dir_all(&fonts).and_then(|_| std::fs::write(&p, data));
            }
        }
        Self { db: Arc::new(db), chrome: find_browser(), render_dir }
    }

    /// `@font-face` rules for the bundled Inter, for HTML designs.
    pub fn font_css(&self) -> String {
        INTER
            .iter()
            .map(|(name, weight, _)| format!("@font-face{{font-family:Inter;font-weight:{weight};src:url(\"file://{}\") format(\"truetype\")}}", self.render_dir.join("fonts").join(name).display()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Is this font family available to the SVG renderer, by name?
    pub fn has_family(&self, name: &str) -> bool {
        let q = fontdb::Query { families: &[fontdb::Family::Name(name.trim())], ..Default::default() };
        self.db.query(&q).is_some()
    }

    /// The family names the renderer knows, for the brand kit to match against.
    pub fn families(&self) -> Vec<String> {
        let mut out: Vec<String> = self.db.faces().flat_map(|f| f.families.iter().map(|(n, _)| n.clone())).collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    pub fn chrome(&self) -> Option<&Path> {
        self.chrome.as_deref()
    }

    pub fn html_available(&self) -> bool {
        self.chrome.is_some()
    }

    /// Render SVG text to a PNG of exactly `width`×`height`, scaling the
    /// drawing to fit. `<image>` files may only come from `allowed_dirs`.
    pub fn render_svg(&self, svg: &str, width: u32, height: u32, allowed_dirs: &[PathBuf]) -> Result<Vec<u8>> {
        check_size(width, height)?;
        let allowed: Vec<PathBuf> = allowed_dirs.iter().filter_map(|d| d.canonicalize().ok()).collect();
        let resolver = usvg::ImageHrefResolver {
            resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(move |href: &str, opts: &usvg::Options| load_local_image(href, &allowed, opts)),
        };
        let opt = usvg::Options { fontdb: self.db.clone(), font_family: "Inter".into(), image_href_resolver: resolver, ..Default::default() };
        let tree = usvg::Tree::from_str(svg, &opt).map_err(|e| anyhow!("the SVG does not parse: {e}"))?;
        let size = tree.size();
        if size.width() <= 0.0 || size.height() <= 0.0 {
            bail!("the SVG has no size; set width and height (or a viewBox)");
        }
        let sx = width as f32 / size.width();
        let sy = height as f32 / size.height();
        let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| anyhow!("could not allocate a {width}×{height} image"))?;
        resvg::render(&tree, tiny_skia::Transform::from_scale(sx, sy), &mut pixmap.as_mut());
        pixmap.encode_png().context("encoding the PNG")
    }

    /// Render an HTML document to a PNG with the headless browser, network off.
    pub async fn render_html(&self, html: &str, width: u32, height: u32) -> Result<Vec<u8>> {
        check_size(width, height)?;
        let Some(chrome) = &self.chrome else {
            bail!("HTML rendering needs a Chromium, Chrome, Brave or Edge on this machine; none was found. Use SVG.");
        };
        std::fs::create_dir_all(&self.render_dir)?;
        let id = uuid::Uuid::new_v4().to_string();
        let input = self.render_dir.join(format!("{id}.html"));
        let output = self.render_dir.join(format!("{id}.png"));
        std::fs::write(&input, html)?;
        let url = format!("file://{}", input.display());
        let base = |no_sandbox: bool| {
            let mut c = tokio::process::Command::new(chrome);
            c.args(["--headless=new", "--disable-gpu", "--hide-scrollbars", "--no-first-run", "--no-default-browser-check", "--disable-extensions", "--force-device-scale-factor=1", "--host-resolver-rules=MAP * ~NOTFOUND", "--virtual-time-budget=1500"]);
            if no_sandbox {
                c.arg("--no-sandbox");
            }
            c.arg(format!("--window-size={width},{height}")).arg(format!("--screenshot={}", output.display())).arg(&url);
            c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
            c
        };
        let mut result = tokio::time::timeout(std::time::Duration::from_secs(40), base(false).output()).await;
        if let Ok(Ok(o)) = &result {
            let err = String::from_utf8_lossy(&o.stderr);
            if !output.exists() && (err.contains("sandbox") || err.contains("SUID")) {
                result = tokio::time::timeout(std::time::Duration::from_secs(40), base(true).output()).await;
            }
        }
        let _ = std::fs::remove_file(&input);
        let png = match result {
            Ok(Ok(_)) if output.exists() => std::fs::read(&output),
            Ok(Ok(o)) => {
                let err = String::from_utf8_lossy(&o.stderr);
                bail!("the browser produced no image: {}", crate::profile::truncate_chars(err.trim(), 300));
            }
            Ok(Err(e)) => bail!("could not start the browser: {e}"),
            Err(_) => bail!("the browser took too long"),
        };
        let _ = std::fs::remove_file(&output);
        Ok(png?)
    }
}

fn check_size(width: u32, height: u32) -> Result<()> {
    if width < 16 || height < 16 || width > MAX_SIDE || height > MAX_SIDE {
        bail!("the size must be between 16 and {MAX_SIDE} pixels a side (got {width}×{height})");
    }
    Ok(())
}

/// `<image href>` files: only under the allowed folders, by sniffed type.
fn load_local_image(href: &str, allowed: &[PathBuf], opts: &usvg::Options) -> Option<usvg::ImageKind> {
    let raw = href.strip_prefix("file://").unwrap_or(href);
    let path = Path::new(raw).canonicalize().ok()?;
    if !allowed.iter().any(|d| path.starts_with(d)) {
        tracing::warn!(href, "image outside the allowed folders");
        return None;
    }
    let data = std::fs::read(&path).ok()?;
    let kind = if data.starts_with(b"\x89PNG") {
        usvg::ImageKind::PNG(Arc::new(data))
    } else if data.starts_with(b"\xFF\xD8") {
        usvg::ImageKind::JPEG(Arc::new(data))
    } else if data.starts_with(b"GIF8") {
        usvg::ImageKind::GIF(Arc::new(data))
    } else if data.starts_with(b"RIFF") {
        usvg::ImageKind::WEBP(Arc::new(data))
    } else {
        let text = String::from_utf8_lossy(&data);
        if text.contains("<svg") {
            let sub = usvg::Options { fontdb: opts.fontdb.clone(), ..Default::default() };
            usvg::ImageKind::SVG(usvg::Tree::from_data(&data, &sub).ok()?)
        } else {
            return None;
        }
    };
    Some(kind)
}

/// A headless-capable browser on PATH (or in the usual macOS place).
pub fn find_browser() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for name in BROWSERS {
        for dir in std::env::split_paths(&path) {
            let p = dir.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let mac = Path::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
    if mac.is_file() {
        return Some(mac.to_path_buf());
    }
    None
}

/// Relative luminance of `#rrggbb`, 0 (black) to 1 (white).
pub fn luminance(hex: &str) -> Option<f32> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|v| v as f32 / 255.0);
    let (r, g, b) = (c(0)?, c(2)?, c(4)?);
    let lin = |v: f32| if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    Some(0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_card_with_bundled_inter() {
        let r = Renderer::new(std::env::temp_dir().join("kiqr-render-test"));
        assert!(r.has_family("Inter"));
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <rect width="1600" height="900" fill="#0B1220"/>
  <text x="188" y="330" fill="#FFFFFF" font-size="72" font-weight="700">Hello there</text>
</svg>"##;
        let png = r.render_svg(svg, 800, 450, &[]).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        assert!(png.len() > 2000, "{} bytes", png.len());
        // Width and height are in the IHDR chunk.
        let w = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        let h = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        assert_eq!((w, h), (800, 450));
        assert!(r.render_svg("not an svg at all", 100, 100, &[]).is_err());
        assert!(r.render_svg(svg, 10, 10, &[]).is_err());
    }

    #[test]
    fn images_outside_allowed_folders_are_dropped() {
        let r = Renderer::new(std::env::temp_dir().join("kiqr-render-test"));
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><image href="file:///etc/hostname" width="50" height="50"/></svg>"##;
        // Does not fail, just renders without the image.
        assert!(r.render_svg(svg, 100, 100, &[]).is_ok());
        assert!(luminance("#ffffff").unwrap() > 0.99);
        assert!(luminance("#000000").unwrap() < 0.01);
    }
}
