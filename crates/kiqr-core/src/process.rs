//! Starting other programs the same way on every platform.
//!
//! On Windows, tools installed with npm (`claude`, `npx`, …) are `.cmd` shims
//! that `Command::new("claude")` does not find, and a child started with an
//! empty environment cannot even load its runtime without `SystemRoot`.

use std::path::{Path, PathBuf};

/// The variables a child keeps when its environment is otherwise cleared.
#[cfg(unix)]
pub const BASE_ENV: &[&str] = &["PATH", "HOME", "LANG", "LC_ALL", "TMPDIR", "USER"];
#[cfg(windows)]
pub const BASE_ENV: &[&str] = &[
    "PATH", "PATHEXT", "HOME", "USERPROFILE", "USERNAME", "APPDATA", "LOCALAPPDATA", "PROGRAMDATA", "ProgramFiles", "SystemRoot", "SystemDrive", "windir", "ComSpec", "TEMP", "TMP",
];

/// The user's home directory: `HOME`, or `USERPROFILE` on Windows.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// Where a program is: a path that exists, or a name found on PATH (trying
/// the PATHEXT extensions on Windows).
pub fn find(cmd: &str) -> Option<PathBuf> {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return None;
    }
    let p = Path::new(cmd);
    if p.components().count() > 1 {
        return candidates(p).into_iter().find(|c| c.is_file());
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).flat_map(|d| candidates(&d.join(cmd))).find(|c| c.is_file())
}

#[cfg(unix)]
fn candidates(p: &Path) -> Vec<PathBuf> {
    vec![p.to_path_buf()]
}

#[cfg(windows)]
fn candidates(p: &Path) -> Vec<PathBuf> {
    let mut out = vec![p.to_path_buf()];
    if p.extension().is_none() {
        let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        out.extend(exts.split(';').filter(|e| !e.is_empty()).map(|e| p.with_extension(e.trim_start_matches('.').to_ascii_lowercase())));
    }
    out
}

/// A command for a program by name, resolved on PATH where the platform needs it.
pub fn command(program: &str) -> tokio::process::Command {
    match find(program) {
        Some(p) if cfg!(windows) => tokio::process::Command::new(p),
        _ => tokio::process::Command::new(program),
    }
}

/// End a process now: SIGTERM on Unix, `taskkill /F` on Windows.
pub fn terminate(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    }
}

/// Ask a process to stop: an interrupt first, a kill when it is still there after `grace`.
pub fn interrupt(pid: u32, grace: std::time::Duration) {
    #[cfg(unix)]
    {
        unsafe {
            libc::kill(pid as i32, libc::SIGINT);
        }
        tokio::spawn(async move {
            tokio::time::sleep(grace).await;
            // If it is gone, kill(0) fails and this is a no-op; a reused pid in a few seconds is not a concern on a desktop.
            unsafe {
                if libc::kill(pid as i32, 0) == 0 {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            }
        });
    }
    #[cfg(windows)]
    {
        let quiet = move |force: bool| {
            let mut c = std::process::Command::new("taskkill");
            c.args(["/PID", &pid.to_string(), "/T"]);
            if force {
                c.arg("/F");
            }
            c.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
            c
        };
        let _ = quiet(false).status();
        tokio::spawn(async move {
            tokio::time::sleep(grace).await;
            let _ = quiet(true).status();
        });
    }
}
