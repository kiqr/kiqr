//! Factory reset: erase the whole KIQR home (projects, history, settings, keys,
//! installed abilities) and start again as if just installed. The binary and the
//! background service live outside the home and stay.
//!
//! Only a person in the browser can do this. There is no tool for it, and the
//! routes refuse:
//! - anything KIQR itself sends (the CLI and `kiqr mcp`, which Seemo's tools go
//!   through, and every HTTP ability): they carry `x-kiqr-agent`;
//! - anything that is not a same-origin request from the web UI (`Sec-Fetch-Site`,
//!   which only browsers set and pages cannot fake, and `Origin`).
//!
//! And it takes two steps: `/api/reset/start` hands out a one-time token that
//! lives two minutes, with a captcha of KIQR's own (a small sum, drawn as a
//! distorted PNG; the answer never leaves the daemon), and `/api/reset` needs
//! that token, the captcha's answer and the phrase typed out by hand.

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{extract::State, http::HeaderMap, http::StatusCode, routing::post, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Set on every request KIQR makes itself; the reset refuses any request with it.
pub const AGENT_HEADER: &str = "x-kiqr-agent";
/// What the person types to confirm.
pub const PHRASE: &str = "erase everything";
const TOKEN_TTL: Duration = Duration::from_secs(120);

/// The one open confirmation: its token, when it was handed out, and the captcha's answer.
static PENDING: Mutex<Option<(String, Instant, String)>> = Mutex::new(None);

pub fn routes() -> Router<App> {
    Router::new().route("/api/reset/start", post(start)).route("/api/reset", post(reset))
}

/// A person in the web UI, and nothing else.
fn from_browser(h: &HeaderMap) -> Result<(), ApiError> {
    let refuse = || ApiError(StatusCode::FORBIDDEN, "A factory reset can only be started by a person in KIQR's web UI.".into());
    if h.contains_key(AGENT_HEADER) {
        return Err(refuse());
    }
    let get = |k: &str| h.get(k).and_then(|v| v.to_str().ok()).unwrap_or("");
    if get("sec-fetch-site") != "same-origin" {
        return Err(refuse());
    }
    let host = get("host");
    let origin = get("origin");
    if host.is_empty() || !(origin == format!("http://{host}") || origin == format!("https://{host}")) {
        return Err(refuse());
    }
    Ok(())
}

/// Never erase something that is not a KIQR home: a mistyped `KIQR_HOME` could point at a user's whole home folder.
fn safe_root(root: &Path) -> Result<(), ApiError> {
    let no = |why: &str| ApiError(StatusCode::CONFLICT, format!("Not erasing {}: {why}", root.display()));
    if root.parent().is_none() {
        return Err(no("it is the root of the file system"));
    }
    if kiqr_core::process::home_dir().is_some_and(|h| h == root) {
        return Err(no("it is your home folder"));
    }
    if !root.join("config.yaml").is_file() && !root.join(".git").is_dir() && !root.join("projects").is_dir() {
        return Err(no("it doesn't look like a KIQR home"));
    }
    Ok(())
}

async fn start(State(app): State<App>, headers: HeaderMap) -> ApiResult<Value> {
    from_browser(&headers)?;
    let root = app.store.home().root().to_path_buf();
    safe_root(&root)?;
    let token = uuid::Uuid::new_v4().simple().to_string();
    let (svg, answer) = captcha(uuid::Uuid::new_v4().as_u128());
    let png = app.renderer.render_svg(&svg, CAPTCHA_W, CAPTCHA_H, &[]).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("could not draw the captcha: {e}")))?;
    use base64::Engine as _;
    let image = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png));
    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some((token.clone(), Instant::now(), answer));
    Ok(Json(json!({ "token": token, "phrase": PHRASE, "captcha": image, "home": root.display().to_string(), "expires_in": TOKEN_TTL.as_secs() })))
}

const CAPTCHA_W: u32 = 320;
const CAPTCHA_H: u32 = 96;

/// A small random number generator for drawing the captcha (xorshift, seeded from a v4 UUID).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }
}

/// "23 + 7 = ?" drawn with every character jittered and turned, over noise lines and dots.
/// Returns the SVG and the answer.
fn captcha(seed: u128) -> (String, String) {
    let mut r = Rng(((seed >> 64) as u64 ^ seed as u64) | 1);
    let a = r.range(11, 49);
    let b = r.range(2, 9);
    let text: Vec<char> = format!("{a} + {b} = ?").chars().collect();
    let mut svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{CAPTCHA_W}" height="{CAPTCHA_H}" viewBox="0 0 {CAPTCHA_W} {CAPTCHA_H}"><rect width="100%" height="100%" rx="10" fill="#111827"/>"##);
    let colours = ["#f87171", "#fca5a5", "#fbbf24", "#e5e7eb", "#fda4af"];
    for _ in 0..7 {
        let c = colours[r.range(0, 4) as usize];
        svg += &format!(
            r#"<path d="M{} {} Q{} {} {} {}" stroke="{c}" stroke-opacity="0.45" stroke-width="{}" fill="none"/>"#,
            r.range(0, 60),
            r.range(5, 90),
            r.range(80, 240),
            r.range(-20, 116),
            r.range(260, 320),
            r.range(5, 90),
            r.range(1, 3)
        );
    }
    let mut x = 22;
    for ch in text {
        if ch == ' ' {
            x += 12;
            continue;
        }
        let c = colours[r.range(0, 4) as usize];
        let size = r.range(34, 44);
        let y = r.range(58, 72);
        let turn = r.range(-24, 24);
        svg += &format!(r#"<text x="{x}" y="{y}" transform="rotate({turn} {x} {y})" font-family="Inter" font-weight="700" font-size="{size}" fill="{c}">{ch}</text>"#);
        x += r.range(22, 28);
    }
    for _ in 0..40 {
        svg += &format!(r#"<circle cx="{}" cy="{}" r="{}" fill="{}" fill-opacity="0.5"/>"#, r.range(0, 320), r.range(0, 96), r.range(1, 2), colours[r.range(0, 4) as usize]);
    }
    svg += "</svg>";
    (svg, (a + b).to_string())
}

#[derive(Deserialize)]
struct Confirm {
    token: String,
    phrase: String,
    /// The captcha's answer.
    #[serde(default)]
    captcha: String,
}

async fn reset(State(app): State<App>, headers: HeaderMap, Json(c): Json<Confirm>) -> ApiResult<Value> {
    from_browser(&headers)?;
    // One try per token, right or wrong.
    let pending = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
    let answer = match pending {
        Some((t, at, answer)) if t == c.token && at.elapsed() < TOKEN_TTL => answer,
        _ => return Err(ApiError(StatusCode::FORBIDDEN, "That confirmation has expired. Start again.".into())),
    };
    if c.captcha.trim() != answer {
        return Err(ApiError(StatusCode::BAD_REQUEST, "That's not the answer to the sum. Here is a new one.".into()));
    }
    if c.phrase.trim().to_lowercase() != PHRASE {
        return Err(ApiError(StatusCode::BAD_REQUEST, format!("Type “{PHRASE}” to confirm.")));
    }
    let root = app.store.home().root().to_path_buf();
    safe_root(&root)?;
    tracing::warn!(home = %root.display(), "factory reset requested from the web UI");

    // Stop Seemo and running work first, so nothing writes into the home while it goes.
    for t in app.turns.read().await.values() {
        t.stop();
    }
    let app2 = app.clone();
    tokio::spawn(async move {
        for _ in 0..20 {
            if app2.turns.read().await.is_empty() && app2.jobs.read().await.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        let failed = erase(&root);
        if !failed.is_empty() {
            tracing::warn!(?failed, "factory reset: could not remove everything");
        }
        tracing::warn!("factory reset done; starting again as a new installation");
        crate::restart_self(&app2.exe);
    });
    // A new installation listens on the default address (unless KIQR_HOST / KIQR_PORT say otherwise); the page goes there.
    let (_, port) = crate::listen_addr(&kiqr_core::config::DaemonConfig::default());
    Ok(Json(json!({ "resetting": true, "port": port })))
}

/// Remove everything inside the home, keeping the folder itself (it may be a mount, as `/data` in Docker).
fn erase(root: &Path) -> Vec<String> {
    let mut failed = vec![];
    let Ok(rd) = std::fs::read_dir(root) else { return vec![root.display().to_string()] };
    for e in rd.flatten() {
        let p = e.path();
        // A symlink is removed as a link; its target is left alone.
        let r = match e.file_type() {
            Ok(t) if t.is_dir() => std::fs::remove_dir_all(&p),
            _ => std::fs::remove_file(&p),
        };
        if let Err(err) = r {
            failed.push(format!("{}: {err}", p.display()));
        }
    }
    failed
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn browser() -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("host", HeaderValue::from_static("127.0.0.1:4040"));
        h.insert("origin", HeaderValue::from_static("http://127.0.0.1:4040"));
        h.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        h
    }

    #[test]
    fn only_the_web_ui_may_reset() {
        assert!(from_browser(&browser()).is_ok());
        let mut agent = browser();
        agent.insert(AGENT_HEADER, HeaderValue::from_static("cli"));
        assert!(from_browser(&agent).is_err(), "Seemo's tools and abilities carry the agent header");
        let mut cross = browser();
        cross.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
        assert!(from_browser(&cross).is_err(), "another site in the browser");
        let mut other = browser();
        other.insert("origin", HeaderValue::from_static("http://evil.test"));
        assert!(from_browser(&other).is_err());
        let mut plain = HeaderMap::new();
        plain.insert("host", HeaderValue::from_static("127.0.0.1:4040"));
        assert!(from_browser(&plain).is_err(), "curl and other programs");
    }

    #[test]
    fn the_captcha_is_a_sum_with_its_answer() {
        let (svg, answer) = captcha(0x1234_5678_9abc_def0_1122_3344_5566_7788);
        let n: i64 = answer.parse().unwrap();
        assert!((13..=58).contains(&n));
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        assert!(!svg.contains(&format!(">{answer}<")), "the answer itself is never drawn");
        assert_ne!(captcha(1).0, captcha(2).0);
    }

    #[test]
    fn erases_only_a_kiqr_home() {
        let dir = std::env::temp_dir().join(format!("kiqr-reset-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(dir.join("projects/acme")).unwrap();
        std::fs::write(dir.join("config.yaml"), "port: 4040\n").unwrap();
        std::fs::write(dir.join(".env"), "X=1\n").unwrap();
        assert!(safe_root(&dir).is_ok());
        assert!(erase(&dir).is_empty());
        assert!(dir.is_dir() && std::fs::read_dir(&dir).unwrap().next().is_none());
        assert!(safe_root(&dir).is_err(), "an empty folder is not a KIQR home");
        assert!(safe_root(Path::new("/")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
