//! KIQR updating itself: the daemon reads the latest release's manifest
//! (`latest.json`, published by the release workflow next to the archives),
//! says when a newer version exists, and on the user's word downloads the
//! archive for this platform, checks its SHA-256, swaps the binary and
//! restarts in place. Nothing is installed without being asked.

use crate::api::{ApiError, ApiResult};
use crate::App;
use anyhow::{anyhow, bail, Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use kiqr_core::events::EventKind;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

/// The latest stable release's manifest. `KIQR_UPDATE_URL` points elsewhere (a mirror, or a pre-release to test with).
const MANIFEST_URL: &str = "https://github.com/kiqr/kiqr/releases/latest/download/latest.json";
/// The platform this binary was built for, set by `build.rs`.
const TARGET: &str = env!("KIQR_TARGET");
const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// How often the daemon looks for a new version on its own.
const EVERY: Duration = Duration::from_secs(6 * 3600);

pub fn routes() -> Router<App> {
    Router::new().route("/api/update", get(status).post(apply))
}

/// What `latest.json` holds.
#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    pub version: String,
    #[serde(default)]
    pub notes_url: Option<String>,
    #[serde(default)]
    pub assets: HashMap<String, Asset>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    pub url: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub current: &'static str,
    pub latest: Option<String>,
    pub available: bool,
    pub notes_url: Option<String>,
    pub checked_at: Option<DateTime<Utc>>,
    /// Whether this installation can replace its own binary; `reason` says why not.
    pub can_self_update: bool,
    pub reason: Option<String>,
    /// The last check or update that failed.
    pub error: Option<String>,
    pub updating: bool,
}

/// What the daemon knows about releases, shared by the routes and the background check.
pub struct Updates {
    status: RwLock<Status>,
    manifest: RwLock<Option<Manifest>>,
    busy: Mutex<()>,
}

impl Updates {
    pub fn new(exe: &Path) -> Self {
        let reason = blocker(exe);
        Self {
            status: RwLock::new(Status { current: CURRENT, latest: None, available: false, notes_url: None, checked_at: None, can_self_update: reason.is_none(), reason, error: None, updating: false }),
            manifest: RwLock::new(None),
            busy: Mutex::new(()),
        }
    }

    pub async fn status(&self) -> Status {
        self.status.read().await.clone()
    }
}

/// Why this binary cannot replace itself, if it cannot.
fn blocker(exe: &Path) -> Option<String> {
    let s = exe.to_string_lossy().replace('\\', "/");
    if Path::new("/.dockerenv").exists() || std::env::var_os("KIQR_IN_DOCKER").is_some() {
        return Some("KIQR runs in a container here; pull the new image instead.".into());
    }
    if s.contains("/target/debug/") || s.contains("/target/release/") {
        return Some("This is a development build; build the new version from source.".into());
    }
    if s.contains("/.cargo/bin/") {
        return Some("KIQR was installed with Cargo; run the installer, or cargo install again.".into());
    }
    let dir = exe.parent()?;
    let probe = dir.join(format!(".kiqr-write-test-{}", std::process::id()));
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            None
        }
        Err(_) => Some(format!("KIQR cannot write to {}; run the installer again to update.", dir.display())),
    }
}

/// The release asset for this binary. Linux builds are static (musl) so they
/// run anywhere; a local glibc or MinGW build takes the same archive.
fn target_key() -> String {
    TARGET.replace("-unknown-linux-gnu", "-unknown-linux-musl").replace("-pc-windows-gnu", "-pc-windows-msvc")
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent(format!("KIQR/{CURRENT}")).timeout(Duration::from_secs(300)).build()?)
}

fn newer(latest: &str) -> bool {
    match (semver::Version::parse(latest.trim_start_matches('v')), semver::Version::parse(CURRENT)) {
        (Ok(l), Ok(c)) => l > c,
        _ => false,
    }
}

/// Read the manifest and record what it says. Emits `UpdateAvailable` the first time a version is seen.
pub async fn check(app: &App) -> Result<Status> {
    let url = std::env::var("KIQR_UPDATE_URL").unwrap_or_else(|_| MANIFEST_URL.to_string());
    let fetched: Result<Manifest> = async {
        let res = client()?.get(&url).send().await?.error_for_status()?;
        Ok(res.json::<Manifest>().await?)
    }
    .await;
    let u = &app.updates;
    let mut st = u.status.write().await;
    st.checked_at = Some(Utc::now());
    match fetched {
        Ok(m) => {
            let available = newer(&m.version);
            let first = available && st.latest.as_deref() != Some(&m.version);
            st.latest = Some(m.version.clone());
            st.available = available;
            st.notes_url = m.notes_url.clone();
            st.error = None;
            if first {
                app.bus.emit(None, EventKind::UpdateAvailable { version: m.version.clone() });
            }
            *u.manifest.write().await = Some(m);
        }
        Err(e) => st.error = Some(format!("could not read {url}: {e}")),
    }
    Ok(st.clone())
}

/// Looks for a new version shortly after start and then every few hours. `KIQR_UPDATE_CHECK=0` turns it off.
pub async fn run(app: App) {
    if std::env::var("KIQR_UPDATE_CHECK").is_ok_and(|v| v == "0" || v == "false") {
        return;
    }
    tokio::time::sleep(Duration::from_secs(20)).await;
    loop {
        match check(&app).await {
            Ok(st) if st.available && st.reason.is_none() && app.config.read().await.updates.auto => {
                // Installing on its own (Settings, or `kiqr update --auto on`): only when nothing is running, else soon again.
                if app.turns.read().await.is_empty() && app.jobs.read().await.is_empty() {
                    tracing::info!(version = ?st.latest, "installing the new version on its own");
                    if let Err(e) = update_now(&app).await {
                        tracing::warn!(error = %e.1, "automatic update failed");
                    }
                }
                tokio::time::sleep(Duration::from_secs(300)).await;
                continue;
            }
            Ok(_) => {}
            Err(e) => tracing::debug!(error = %e, "update check failed"),
        }
        // A little jitter so many installs don't ask at the same moment.
        let jitter = Duration::from_secs(u64::from(std::process::id() % 600));
        tokio::time::sleep(EVERY + jitter).await;
    }
}

#[derive(Deserialize)]
struct StatusQuery {
    #[serde(default)]
    refresh: Option<String>,
}

async fn status(State(app): State<App>, Query(q): Query<StatusQuery>) -> ApiResult<Status> {
    let st = app.updates.status().await;
    if q.refresh.is_some() || st.checked_at.is_none() {
        return Ok(Json(check(&app).await?));
    }
    Ok(Json(st))
}

/// Download, verify and install the latest version, then restart. Answers before the restart.
async fn apply(State(app): State<App>) -> Result<(StatusCode, Json<Status>), ApiError> {
    Ok((StatusCode::ACCEPTED, Json(update_now(&app).await?)))
}

/// The update itself, for the route and for automatic updates.
async fn update_now(app: &App) -> Result<Status, ApiError> {
    let st = check(app).await?;
    if let Some(r) = &st.reason {
        return Err(ApiError(StatusCode::CONFLICT, r.clone()));
    }
    if !st.available {
        return Err(ApiError(StatusCode::CONFLICT, format!("KIQR {CURRENT} is the latest version.")));
    }
    if !app.turns.read().await.is_empty() || !app.jobs.read().await.is_empty() {
        return Err(ApiError(StatusCode::CONFLICT, "Seemo is working right now; update when it is done.".into()));
    }
    let Ok(guard) = app.updates.busy.try_lock() else {
        return Err(ApiError(StatusCode::CONFLICT, "An update is already in progress.".into()));
    };
    app.updates.status.write().await.updating = true;
    let manifest = app.updates.manifest.read().await.clone().ok_or_else(|| anyhow!("no release manifest"))?;
    let result = install(&app.exe, &manifest).await;
    drop(guard);
    let mut st = app.updates.status.write().await;
    match result {
        Ok(()) => {
            app.bus.emit(None, EventKind::Updated { from: CURRENT.into(), to: manifest.version.clone() });
            tracing::info!(from = CURRENT, to = %manifest.version, "updated; restarting");
            let exe = app.exe.clone();
            tokio::spawn(async move {
                // Let the response and the event go out first.
                tokio::time::sleep(Duration::from_millis(500)).await;
                crate::restart_self(&exe);
            });
            Ok(st.clone())
        }
        Err(e) => {
            st.updating = false;
            st.error = Some(format!("{e:#}"));
            app.bus.emit(None, EventKind::UpdateFailed { version: manifest.version.clone(), error: format!("{e:#}") });
            Err(ApiError(StatusCode::BAD_GATEWAY, format!("The update failed: {e:#}")))
        }
    }
}

async fn install(exe: &Path, m: &Manifest) -> Result<()> {
    let key = target_key();
    let asset = m.assets.get(&key).ok_or_else(|| anyhow!("release {} has no build for {key}", m.version))?;
    let bytes = client()?.get(&asset.url).send().await?.error_for_status()?.bytes().await.context("download")?;
    let sum = format!("{:x}", Sha256::digest(&bytes));
    if !sum.eq_ignore_ascii_case(asset.sha256.trim()) {
        bail!("the download's checksum does not match the release ({sum})");
    }
    let dir = exe.parent().ok_or_else(|| anyhow!("no folder for {}", exe.display()))?;
    let staged = dir.join(format!(".kiqr-{}.new", m.version));
    let binary = extract(&bytes)?;
    std::fs::write(&staged, binary).with_context(|| format!("write {}", staged.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))?;
    }
    // The new binary has to run here before it takes the old one's place.
    let out = tokio::process::Command::new(&staged).arg("--version").output().await.context("run the new binary")?;
    let said = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() || !said.contains(m.version.trim_start_matches('v')) {
        let _ = std::fs::remove_file(&staged);
        bail!("the new binary did not start ({})", said.trim());
    }
    swap(exe, &staged)
}

#[cfg(unix)]
fn swap(exe: &Path, staged: &Path) -> Result<()> {
    // Same folder, so the rename is atomic; the running process keeps its open copy.
    std::fs::rename(staged, exe).with_context(|| format!("replace {}", exe.display()))
}

#[cfg(windows)]
fn swap(_exe: &Path, staged: &Path) -> Result<()> {
    // A running .exe cannot be overwritten, only moved aside; self-replace does that dance.
    self_replace::self_replace(staged).context("replace the running binary")?;
    let _ = std::fs::remove_file(staged);
    Ok(())
}

/// The `kiqr` binary out of a release archive: `.tar.gz` everywhere but Windows, `.zip` there.
#[cfg(unix)]
fn extract(archive: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries()? {
        let mut entry = entry?;
        if entry.path()?.file_name().is_some_and(|n| n == "kiqr") {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }
    bail!("the archive holds no kiqr binary")
}

#[cfg(windows)]
fn extract(archive: &[u8]) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive))?;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        if Path::new(f.name()).file_name().is_some_and(|n| n.eq_ignore_ascii_case("kiqr.exe")) {
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }
    bail!("the archive holds no kiqr.exe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(newer("999.0.0"));
        assert!(newer("v999.0.0"));
        assert!(!newer(CURRENT));
        assert!(!newer("0.0.1"));
        assert!(!newer("not a version"));
    }

    #[test]
    fn local_builds_take_the_release_build() {
        let k = target_key();
        assert!(!k.ends_with("-linux-gnu") && !k.ends_with("-windows-gnu"), "{k}");
    }
}
