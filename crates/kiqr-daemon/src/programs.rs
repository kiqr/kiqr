//! Programs KIQR installs for abilities (`requires` in a manifest): a pinned
//! version, fetched from a URL for this platform and checked against its
//! sha256 before it is unpacked into `~/.kiqr/bin`, or a `uv` tool installed
//! through the `uv` KIQR installed the same way, or a system package shown to
//! the user as a command and never run. `~/.kiqr/bin` goes first on the
//! daemon's PATH, so every program KIQR starts finds them. Honoured only for
//! official abilities (shipped, or from KIQR's own marketplace): a manifest
//! the user or Seemo wrote never gets to download and run anything.

use crate::api::ApiError;
use crate::App;
use anyhow::{bail, Context, Result};
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{Manifest, Requirement};
use kiqr_core::paths::Home;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Put `~/.kiqr/bin` first on PATH, once, at start.
pub fn prepend_path(home: &Home) {
    let _ = TOOLS_DIR.set(home.root().join("tools"));
    let bin = home.bin_dir();
    let _ = std::fs::create_dir_all(&bin);
    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<PathBuf> = std::env::split_paths(&current).collect();
    if paths.first().is_some_and(|p| *p == bin) {
        return;
    }
    paths.retain(|p| *p != bin);
    paths.insert(0, bin);
    if let Ok(joined) = std::env::join_paths(paths) {
        std::env::set_var("PATH", joined);
    }
}

/// Where uv keeps the tools it installed for KIQR (`~/.kiqr/tools`); set once the daemon knows its home.
static TOOLS_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

fn tools_dir() -> PathBuf {
    TOOLS_DIR.get().cloned().unwrap_or_else(|| PathBuf::from("tools"))
}

/// `linux` / `macos` / `windows`, and `x86_64` / `aarch64`.
fn platform() -> (&'static str, &'static str) {
    let os = if cfg!(target_os = "macos") { "macos" } else if cfg!(windows) { "windows" } else { "linux" };
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
    (os, arch)
}

/// The program is there at its version: `check` (default `<program> --version`,
/// or `uv tool list` for a uv tool) prints something containing the version.
pub fn satisfied(r: &Requirement) -> bool {
    if r.check.is_empty() && !r.uv_tool.trim().is_empty() {
        if !crate::ext::on_path(&r.program) || !crate::ext::on_path("uv") {
            return false;
        }
        let out = std::process::Command::new(kiqr_core::process::find("uv").unwrap_or_else(|| PathBuf::from("uv"))).args(["tool", "list"]).env("UV_TOOL_DIR", tools_dir()).stdin(std::process::Stdio::null()).output();
        return out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("{} v{}", r.program, r.version.trim())));
    }
    let (cmd, args): (String, Vec<String>) = match r.check.split_first() {
        Some((c, rest)) => (c.clone(), rest.to_vec()),
        None => (r.program.clone(), vec!["--version".into()]),
    };
    if !crate::ext::on_path(&cmd) {
        return false;
    }
    let out = std::process::Command::new(kiqr_core::process::find(&cmd).unwrap_or_else(|| PathBuf::from(&cmd))).args(&args).stdin(std::process::Stdio::null()).output();
    match out {
        Ok(o) => {
            let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            text.contains(r.version.trim()) || r.version.trim().is_empty()
        }
        Err(_) => false,
    }
}

/// What the ability needs and what is there, for the ability view.
pub fn status(m: &Manifest) -> Value {
    json!(m.requires.iter().map(|r| json!({ "program": r.program, "version": r.version, "found": satisfied(r), "how": if !r.install.is_empty() { "download" } else if !r.uv_tool.is_empty() { "uv" } else { "system" }, "system": r.system })).collect::<Vec<_>>())
}

/// Shipped, or from KIQR's own marketplace: the only abilities whose `requires` runs.
pub async fn official(app: &App, slug: Option<&str>, id: &str) -> bool {
    let reg = app.registry.read().await;
    reg.entry(slug, id).is_some_and(|e| e.builtin || e.source.as_deref().is_some_and(|s| s.contains("github.com/kiqr/marketplace")))
}

/// Install what an ability needs and is missing, in the background, with
/// progress on the bus. Returns at once; `status` says how far it got.
pub async fn ensure(app: &App, slug: Option<&str>, m: &Manifest) -> Result<(), ApiError> {
    if m.requires.is_empty() || m.requires.iter().all(satisfied) {
        return Ok(());
    }
    if !official(app, slug, &m.id).await {
        return Err(ApiError(axum::http::StatusCode::FORBIDDEN, format!("{} asks KIQR to install programs, which only an ability from KIQR's own marketplace may; install {} yourself", m.name, m.requires.iter().map(|r| r.program.as_str()).collect::<Vec<_>>().join(", "))));
    }
    let job = format!("programs:{}", m.id);
    if !app.jobs.write().await.insert(job.clone()) {
        return Ok(());
    }
    let (app, m) = (app.clone(), m.clone());
    tokio::spawn(async move {
        let home = app.store.home().clone();
        for r in &m.requires {
            if satisfied(r) {
                continue;
            }
            app.bus.emit(None, EventKind::ProgramInstalling { program: r.program.clone(), version: r.version.clone(), note: String::new() });
            match install_one(&app, &home, r).await {
                Ok(()) => app.bus.emit(None, EventKind::ProgramInstalled { program: r.program.clone(), version: r.version.clone() }),
                Err(e) => {
                    tracing::warn!(program = %r.program, "could not install: {e:#}");
                    app.bus.emit(None, EventKind::ProgramFailed { program: r.program.clone(), version: r.version.clone(), message: format!("{e:#}") });
                    break;
                }
            }
        }
        app.jobs.write().await.remove(&job);
        crate::ext::reload(&app).await;
    });
    Ok(())
}

async fn install_one(app: &App, home: &Home, r: &Requirement) -> Result<()> {
    let (os, arch) = platform();
    if let Some(d) = r.install.iter().find(|d| d.os == os && d.arch == arch) {
        return download(app, home, r, d).await;
    }
    if !r.uv_tool.trim().is_empty() {
        return uv_tool(home, r).await;
    }
    if let Some((mgr, pkg)) = r.system.iter().next() {
        bail!("{} is installed with the system's package manager: {}", r.program, system_command(mgr, pkg));
    }
    bail!("{} has no build for {os} {arch}", r.program)
}

fn system_command(mgr: &str, pkg: &str) -> String {
    match mgr {
        "apt" => format!("sudo apt install {pkg}"),
        "brew" => format!("brew install {pkg}"),
        "winget" => format!("winget install {pkg}"),
        "dnf" => format!("sudo dnf install {pkg}"),
        "pacman" => format!("sudo pacman -S {pkg}"),
        other => format!("{other} install {pkg}"),
    }
}

/// Fetch, check the sha256, unpack the one binary into `bin/<program>-<version>/`, and put it on `bin/` as the program.
async fn download(app: &App, home: &Home, r: &Requirement, d: &kiqr_core::extensions::Download) -> Result<()> {
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(600)).build()?;
    let resp = client.get(&d.url).send().await.with_context(|| format!("fetching {}", d.url))?.error_for_status()?;
    if let Some(t) = resp.content_length() {
        app.bus.emit(None, EventKind::ProgramInstalling { program: r.program.clone(), version: r.version.clone(), note: format!("downloading {} MB", t / 1_048_576) });
    }
    let bytes = resp.bytes().await.context("downloading")?.to_vec();
    let sum = format!("{:x}", Sha256::digest(&bytes));
    if !sum.eq_ignore_ascii_case(d.sha256.trim()) {
        bail!("the download of {} does not match its checksum ({sum}); nothing was installed", r.program);
    }
    let bin_name = if d.bin.is_empty() { r.program.clone() } else { d.bin.clone() };
    let binary = match d.archive.as_str() {
        "" => bytes,
        "tar.gz" => extract_tar_gz(&bytes, &bin_name)?,
        "zip" => extract_zip(&bytes, &bin_name)?,
        other => bail!("{other} archives are not supported"),
    };
    let dir = home.bin_dir().join(format!("{}-{}", r.program, r.version));
    std::fs::create_dir_all(&dir)?;
    let exe_name = if cfg!(windows) { format!("{}.exe", r.program) } else { r.program.clone() };
    let staged = dir.join(&exe_name);
    std::fs::write(&staged, &binary).with_context(|| format!("writing {}", staged.display()))?;
    make_executable(&staged)?;
    let link = home.bin_dir().join(&exe_name);
    let _ = std::fs::remove_file(&link);
    place(&staged, &link)?;
    if !satisfied(r) {
        bail!("{} was installed but does not report version {}", r.program, r.version);
    }
    Ok(())
}

fn make_executable(p: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755))?;
    }
    let _ = p;
    Ok(())
}

/// `bin/<program>` points at the installed version: a symlink where there are symlinks, a copy on Windows.
fn place(staged: &Path, link: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(staged, link).with_context(|| format!("linking {}", link.display()))
    }
    #[cfg(windows)]
    {
        std::fs::copy(staged, link).with_context(|| format!("copying to {}", link.display()))?;
        Ok(())
    }
}

#[cfg(unix)]
fn extract_tar_gz(archive: &[u8], bin: &str) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        if path.ends_with(bin) || path.file_name().is_some_and(|n| n == Path::new(bin).file_name().unwrap_or_default()) {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }
    bail!("the archive holds no {bin}")
}

#[cfg(not(unix))]
fn extract_tar_gz(_archive: &[u8], _bin: &str) -> Result<Vec<u8>> {
    bail!("tar.gz archives are not supported on this platform")
}

#[cfg(windows)]
fn extract_zip(archive: &[u8], bin: &str) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive))?;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        let name = f.name().to_string();
        if name.ends_with(bin) || Path::new(&name).file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(Path::new(bin).file_name().unwrap_or_default().to_string_lossy().as_ref())) {
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }
    bail!("the archive holds no {bin}")
}

#[cfg(not(windows))]
fn extract_zip(_archive: &[u8], _bin: &str) -> Result<Vec<u8>> {
    bail!("zip archives are not supported on this platform")
}

/// `uv tool install <spec>` into `bin/`, through the `uv` KIQR installed (its own `requires` entry).
async fn uv_tool(home: &Home, r: &Requirement) -> Result<()> {
    if !crate::ext::on_path("uv") {
        bail!("{} is installed with uv, which is not there yet", r.program);
    }
    let out = kiqr_core::process::command("uv")
        .args(["tool", "install", "--force", r.uv_tool.trim()])
        .env("UV_TOOL_BIN_DIR", home.bin_dir())
        .env("UV_TOOL_DIR", tools_dir())
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .context("running uv")?;
    if !out.status.success() {
        bail!("uv could not install {}: {}", r.uv_tool, kiqr_core::profile::truncate_chars(String::from_utf8_lossy(&out.stderr).trim(), 300));
    }
    if !satisfied(r) {
        bail!("{} was installed but does not report version {}", r.program, r.version);
    }
    Ok(())
}

/// `POST /api/abilities/{id}/programs`: install what the ability needs (again).
pub async fn install_route(axum::extract::State(app): axum::extract::State<App>, axum::extract::Path(id): axum::extract::Path<String>) -> crate::api::ApiResult<Value> {
    let m = { let reg = app.registry.read().await; reg.installed(&id).into_iter().find_map(|e| e.manifest.clone()) }.ok_or_else(|| ApiError(axum::http::StatusCode::NOT_FOUND, format!("'{id}' is not installed")))?;
    ensure(&app, None, &m).await?;
    Ok(axum::Json(json!({ "id": id, "programs": status(&m) })))
}

