//! File storage under a silently maintained local git repository.
//!
//! The daemon is the single writer. Every logical change goes through
//! [`Store::commit`], which stages everything and commits with a readable
//! message. There are no remotes and KIQR never pushes; a user who wants a
//! backup adds a remote and pushes with plain git.

use crate::paths::Home;
use anyhow::{bail, Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;

const GITIGNORE: &str = "\
# KIQR — managed by the daemon. Edit freely; the daemon merges, never overwrites, this file.
.env
*.lock
tmp/
logs/
projects/*/cache/
# Generated assets and project secrets stay on this machine.
projects/*/assets/
projects/*/.env
";

const ENV_TEMPLATE: &str = "\
# KIQR secrets. This file is never committed and must stay mode 0600.
# Provider keys (only the ones you use):
# ANTHROPIC_API_KEY=
# OPENAI_API_KEY=
# CLAUDE_CODE_OAUTH_TOKEN=   # from `claude setup-token`, for Docker/headless
# Fetch escalation:
# UNBLOCKINGAPI_KEY=
# kiqr.cloud (optional, unlocks connectors and hosting):
# KIQR_CLOUD_TOKEN=
";

#[derive(Clone)]
pub struct Store {
    home: Home,
    git: Arc<Mutex<()>>,
}

impl Store {
    /// Open the store, creating and initialising the directory on first run.
    pub fn open(home: Home) -> Result<Self> {
        let store = Self { home, git: Arc::new(Mutex::new(())) };
        store.init_sync()?;
        Ok(store)
    }

    pub fn home(&self) -> &Home {
        &self.home
    }

    fn init_sync(&self) -> Result<()> {
        let root = self.home.root();
        std::fs::create_dir_all(root).with_context(|| format!("creating {}", root.display()))?;
        for dir in [self.home.projects_dir(), self.home.skills_dir(), self.home.seemo_dir(), self.home.asset_types_dir()] {
            std::fs::create_dir_all(dir)?;
        }
        let ignore = root.join(".gitignore");
        if !ignore.exists() {
            std::fs::write(&ignore, GITIGNORE)?;
        } else {
            merge_gitignore(&ignore)?;
        }
        ensure_env_file(&self.home.env_file())?;
        if !root.join(".git").exists() {
            git(root, &["init", "-q", "-b", "main"])?;
            git(root, &["config", "user.name", "KIQR"])?;
            git(root, &["config", "user.email", "kiqr@localhost"])?;
            git(root, &["add", "-A"])?;
            git(root, &["commit", "-q", "-m", "init: KIQR home", "--allow-empty"])?;
        }
        Ok(())
    }

    pub fn read_yaml<T: DeserializeOwned>(&self, path: &Path) -> Result<T> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        serde_yaml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn read_yaml_opt<T: DeserializeOwned>(&self, path: &Path) -> Result<Option<T>> {
        if path.exists() {
            Ok(Some(self.read_yaml(path)?))
        } else {
            Ok(None)
        }
    }

    pub fn write_yaml<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        let text = serde_yaml::to_string(value)?;
        self.write_text(path, &text)
    }

    pub fn write_text(&self, path: &Path, text: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Stage everything and commit. `message` should be readable in a log:
    /// `profile(acme): lock pricing.plans`. Automated changes are prefixed
    /// with `[auto]` by the caller.
    pub async fn commit(&self, message: &str) -> Result<()> {
        let _guard = self.git.lock().await;
        let root = self.home.root().to_path_buf();
        let message = message.to_string();
        tokio::task::spawn_blocking(move || -> Result<()> {
            git(&root, &["add", "-A"])?;
            let status = git(&root, &["status", "--porcelain"])?;
            if status.trim().is_empty() {
                return Ok(());
            }
            git(&root, &["commit", "-q", "-m", &message])?;
            Ok(())
        })
        .await??;
        Ok(())
    }

    /// Recent history, newest first: (hash, date, message).
    pub async fn history(&self, path_filter: Option<&str>, limit: usize) -> Result<Vec<(String, String, String)>> {
        let root = self.home.root().to_path_buf();
        let filter = path_filter.map(str::to_string);
        tokio::task::spawn_blocking(move || -> Result<Vec<(String, String, String)>> {
            let n = format!("-{limit}");
            let mut args = vec!["log", &n, "--date=iso-strict", "--pretty=format:%h\x1f%ad\x1f%s"];
            if let Some(f) = filter.as_deref() {
                args.push("--");
                args.push(f);
            }
            let out = git(&root, &args)?;
            Ok(out
                .lines()
                .filter_map(|l| {
                    let mut it = l.split('\x1f');
                    Some((it.next()?.to_string(), it.next()?.to_string(), it.next()?.to_string()))
                })
                .collect())
        })
        .await?
    }
}

fn ensure_env_file(path: &Path) -> Result<()> {
    if !path.exists() {
        std::fs::write(path, ENV_TEMPLATE)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)?.permissions().mode() & 0o777;
        if mode != 0o600 {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
    }
    Ok(())
}

fn merge_gitignore(path: &Path) -> Result<()> {
    let existing = std::fs::read_to_string(path)?;
    let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();
    let mut changed = false;
    for required in GITIGNORE.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        if !lines.iter().any(|l| l.trim() == required) {
            lines.push(required.to_string());
            changed = true;
        }
    }
    if changed {
        std::fs::write(path, lines.join("\n") + "\n")?;
    }
    Ok(())
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    // The home repo is local bookkeeping: never sign (a signing agent could
    // block the daemon waiting for a prompt).
    let out = Command::new("git")
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(args)
        .current_dir(root)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .context("running git — is git installed?")?;
    if !out.status.success() {
        bail!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
