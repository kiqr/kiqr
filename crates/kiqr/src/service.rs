//! `kiqr start`, `stop` and `restart`: the daemon as a background service,
//! through whatever the installer registered it with — a systemd user unit
//! on Linux, a launchd agent on macOS, a Task Scheduler task on Windows.
//! Without one (`KIQR_NO_SERVICE=1`, a Linux without systemd) `start` runs
//! `kiqr daemon` in the background and `stop` ends that process.

use anyhow::{anyhow, bail, Result};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// What runs the daemon on this machine.
#[derive(Debug)]
pub enum Manager {
    Systemd,
    Launchd(PathBuf),
    TaskScheduler,
    /// Nothing registered: a plain background process.
    None,
}

const UNIT: &str = "kiqr.service";
const LABEL: &str = "dev.kiqr.daemon";
const TASK: &str = "KIQR daemon";

impl Manager {
    pub fn detect() -> Self {
        let home = kiqr_core::process::home_dir().unwrap_or_default();
        if cfg!(target_os = "linux") && home.join(".config/systemd/user").join(UNIT).is_file() && quiet("systemctl", &["--user", "--version"]) {
            return Self::Systemd;
        }
        let plist = home.join("Library/LaunchAgents").join(format!("{LABEL}.plist"));
        if cfg!(target_os = "macos") && plist.is_file() {
            return Self::Launchd(plist);
        }
        if cfg!(windows) && quiet("schtasks", &["/Query", "/TN", TASK]) {
            return Self::TaskScheduler;
        }
        Self::None
    }

    /// In words, for `kiqr status`.
    pub fn describe(&self) -> String {
        match self {
            Self::Systemd => format!("systemd user service ({UNIT}), starts at login"),
            Self::Launchd(_) => format!("launchd agent ({LABEL}), starts at login"),
            Self::TaskScheduler => format!("Task Scheduler task \"{TASK}\", starts at logon"),
            Self::None => "not registered as a service; `kiqr start` runs it in the background until you log out".into(),
        }
    }

    pub fn start(&self) -> Result<()> {
        match self {
            Self::Systemd => run("systemctl", &["--user", "start", UNIT]),
            Self::Launchd(plist) => {
                if loaded() {
                    run("launchctl", &["kickstart", &target()])
                } else {
                    run("launchctl", &["load", "-w", &plist.to_string_lossy()])
                }
            }
            Self::TaskScheduler => run("schtasks", &["/Run", "/TN", TASK]),
            Self::None => spawn_detached(),
        }
    }

    /// Stop through the manager. `false` for a plain process, which the caller ends by its pid.
    pub fn stop(&self) -> Result<bool> {
        match self {
            Self::Systemd => run("systemctl", &["--user", "stop", UNIT]).map(|_| true),
            // KeepAlive brings a stopped agent straight back; unloading is what stops it. `start` loads it again.
            Self::Launchd(plist) => {
                if loaded() {
                    run("launchctl", &["unload", &plist.to_string_lossy()])?;
                }
                Ok(true)
            }
            Self::TaskScheduler => {
                // Ends every instance of the task: the one started at logon and any started by an update.
                let _ = quiet("schtasks", &["/End", "/TN", TASK]);
                Ok(true)
            }
            Self::None => Ok(false),
        }
    }

    /// Restart through the manager. `false` when the caller has to stop and start itself.
    pub fn restart(&self) -> Result<bool> {
        match self {
            Self::Systemd => run("systemctl", &["--user", "restart", UNIT]).map(|_| true),
            Self::Launchd(plist) => {
                if loaded() {
                    run("launchctl", &["kickstart", "-k", &target()])?;
                } else {
                    run("launchctl", &["load", "-w", &plist.to_string_lossy()])?;
                }
                Ok(true)
            }
            Self::TaskScheduler | Self::None => Ok(false),
        }
    }
}

/// The launchd service target of this user's agent.
fn target() -> String {
    let uid = Command::new("id").arg("-u").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    format!("gui/{uid}/{LABEL}")
}

fn loaded() -> bool {
    quiet("launchctl", &["list", LABEL])
}

/// Run a command, true when it succeeded; its output is not shown.
fn quiet(program: &str, args: &[&str]) -> bool {
    Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

fn run(program: &str, args: &[&str]) -> Result<()> {
    let out = Command::new(program).args(args).stdin(Stdio::null()).output().map_err(|e| anyhow!("could not run {program}: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("{program} {} failed: {}", args.join(" "), err.trim());
    }
    Ok(())
}

/// `kiqr daemon` in the background, detached from this terminal, logging to `<home>/daemon.log`.
fn spawn_detached() -> Result<()> {
    let exe = std::env::current_exe()?;
    let home = kiqr_core::Home::resolve();
    std::fs::create_dir_all(home.root())?;
    let log = std::fs::OpenOptions::new().create(true).append(true).open(home.root().join("daemon.log"))?;
    let mut cmd = Command::new(exe);
    cmd.arg("daemon").stdin(Stdio::null()).stdout(log.try_clone()?).stderr(log);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own process group, so closing the terminal does not take it along.
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn().map_err(|e| anyhow!("could not start the daemon: {e}"))?;
    Ok(())
}
