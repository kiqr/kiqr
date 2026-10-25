//! Secrets by name, never by value: `~/.kiqr/.env` for the machine and
//! `projects/<slug>/.env` for one project. A project's value wins over the
//! global one, so an email account can differ per project while one API key
//! serves them all. Values are written by the daemon, read when a call is
//! made, and never shown or sent to the model.

use crate::paths::Home;
use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;

/// Parse an env file: `NAME=value`, optional `export`, quotes stripped,
/// comments and blanks skipped.
pub fn read_env_file(path: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(path) else { return out };
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let t = t.strip_prefix("export ").map(str::trim_start).unwrap_or(t);
        let Some((k, v)) = t.split_once('=') else { continue };
        let k = k.trim();
        if !valid_name(k) {
            continue;
        }
        let v = v.trim();
        let v = v.strip_prefix('"').and_then(|r| r.strip_suffix('"')).or_else(|| v.strip_prefix('\'').and_then(|r| r.strip_suffix('\''))).unwrap_or(v);
        if !v.is_empty() {
            out.insert(k.to_string(), v.to_string());
        }
    }
    out
}

pub fn valid_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 64 && n.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') && !n.starts_with(|c: char| c.is_ascii_digit())
}

/// The value of one secret for a project: the project's `.env`, else the
/// process environment (the global `.env` is loaded into it at start).
pub fn lookup(home: &Home, slug: Option<&str>, name: &str) -> Option<String> {
    if let Some(slug) = slug.filter(|s| !s.is_empty()) {
        if let Some(v) = read_env_file(&home.project_env_file(slug)).remove(name) {
            return Some(v);
        }
    }
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

pub fn has(home: &Home, slug: Option<&str>, name: &str) -> bool {
    lookup(home, slug, name).is_some()
}

/// Where a secret is set: `project`, `global`, or nothing.
pub fn scope_of(home: &Home, slug: Option<&str>, name: &str) -> Option<&'static str> {
    if let Some(slug) = slug.filter(|s| !s.is_empty()) {
        if read_env_file(&home.project_env_file(slug)).contains_key(name) {
            return Some("project");
        }
    }
    std::env::var(name).ok().filter(|v| !v.trim().is_empty()).map(|_| "global")
}

/// A lookup closure for templates (`${NAME}`), bound to one project.
pub fn env_fn(home: &Home, slug: Option<&str>) -> impl Fn(&str) -> Option<String> + Clone {
    let home = home.clone();
    let slug = slug.map(str::to_string);
    move |k: &str| lookup(&home, slug.as_deref(), k)
}

/// Replace or append `NAME=value` in an env file, keeping everything else
/// and mode 0600. An empty value removes the line.
pub fn write_env(path: &Path, name: &str, value: &str) -> Result<()> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    let mut done = false;
    for l in existing.lines() {
        let t = l.trim_start();
        let is_ours = t.strip_prefix(name).is_some_and(|r| r.starts_with('=')) || t.strip_prefix("export ").and_then(|r| r.trim_start().strip_prefix(name)).is_some_and(|r| r.starts_with('='));
        let is_commented = t.strip_prefix('#').map(|r| r.trim_start()).and_then(|r| r.strip_prefix(name)).is_some_and(|r| r.starts_with('='));
        if is_ours || (is_commented && !done && !value.is_empty()) {
            if !done && !value.is_empty() {
                lines.push(format!("{name}={value}"));
                done = true;
            }
            continue;
        }
        lines.push(l.to_string());
    }
    if !done && !value.is_empty() {
        lines.push(format!("{name}={value}"));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_values_win_and_files_round_trip() {
        let dir = std::env::temp_dir().join(format!("kiqr-secrets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let home = Home::at(&dir);
        let p = home.project_env_file("acme");
        write_env(&p, "SMTP_HOST", "smtp.acme.test").unwrap();
        write_env(&p, "SMTP_USER", "\"jane\"").unwrap();
        assert_eq!(lookup(&home, Some("acme"), "SMTP_HOST").as_deref(), Some("smtp.acme.test"));
        assert_eq!(lookup(&home, Some("other"), "SMTP_HOST"), None);
        assert_eq!(scope_of(&home, Some("acme"), "SMTP_HOST"), Some("project"));
        write_env(&p, "SMTP_HOST", "").unwrap();
        assert_eq!(lookup(&home, Some("acme"), "SMTP_HOST"), None);
        assert!(read_env_file(&p).contains_key("SMTP_USER"));
        assert!(valid_name("FAL_KEY") && !valid_name("fal") && !valid_name("1X"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
