//! The anonymous usage ping: every 15 minutes, and right after an ability is
//! installed, the daemon tells kiqr.cloud that this install runs. It sends a
//! random id made for the purpose, the version, OS and architecture, whether
//! it runs in Docker and is signed in to kiqr.cloud, and the abilities it has
//! that the marketplace lists (any other only as a count). No hostname,
//! project, URL, key or email. `GET /api/telemetry` shows exactly what goes;
//! `kiqr telemetry off`, `KIQR_TELEMETRY=0` or `DO_NOT_TRACK=1` stop it.

use crate::api::ApiResult;
use crate::App;
use anyhow::Result;
use axum::{extract::State, routing::get, Json, Router};
use kiqr_core::events::EventKind;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet};
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// kiqr.cloud counts an install as live when it pinged in the last 20 minutes.
const EVERY: Duration = Duration::from_secs(15 * 60);

pub fn routes() -> Router<App> {
    Router::new().route("/api/telemetry", get(show).put(set))
}

/// What switched the ping off from outside `config.yaml`, if anything.
fn off_by_env(var: impl Fn(&str) -> Option<String>) -> Option<&'static str> {
    let off = |v: Option<String>| v.map(|v| v.trim().to_ascii_lowercase()).is_some_and(|v| matches!(v.as_str(), "0" | "false" | "off" | "no"));
    let set = |v: Option<String>| v.map(|v| v.trim().to_string()).is_some_and(|v| !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false"));
    if off(var("KIQR_TELEMETRY")) {
        Some("KIQR_TELEMETRY")
    } else if set(var("DO_NOT_TRACK")) {
        Some("DO_NOT_TRACK")
    } else {
        None
    }
}

/// Why the ping is off (`KIQR_TELEMETRY`, `DO_NOT_TRACK`, `config`, `no-cloud`), or None when it is on.
async fn off_reason(app: &App) -> Option<&'static str> {
    if let Some(v) = off_by_env(|k| std::env::var(k).ok()) {
        return Some(v);
    }
    let cfg = app.config.read().await;
    if !cfg.telemetry.enabled {
        Some("config")
    } else if cfg.cloud.url.trim().is_empty() {
        Some("no-cloud")
    } else {
        None
    }
}

/// The installed abilities the marketplace (or KIQR itself) names, sorted, and
/// how many others there are: those are counted, never named.
fn split(installed: impl IntoIterator<Item = String>, known: &HashSet<String>) -> (Vec<String>, usize) {
    let all: BTreeSet<String> = installed.into_iter().collect();
    let (named, other): (Vec<String>, Vec<String>) = all.into_iter().partition(|id| known.contains(id));
    (named, other.len())
}

/// The body of the ping. `id` is None before the first one.
async fn payload(app: &App, id: Option<&str>) -> Value {
    let installed: Vec<String> = {
        let reg = app.registry.read().await;
        reg.entries.iter().filter(|e| !e.is_core() && !e.builtin).map(|e| e.id().to_string()).collect()
    };
    let known: HashSet<String> = crate::market::catalogue(app).await.into_iter().map(|i| i.id).collect();
    let (abilities, other) = split(installed, &known);
    let cloud = crate::signins::SIGNINS.iter().find(|s| s.id == crate::signins::CLOUD).is_some_and(crate::signins::signed_in);
    json!({
        "id": id.unwrap_or("(made on the first ping)"),
        "version": VERSION,
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "docker": std::env::var_os("KIQR_IN_DOCKER").is_some() || std::path::Path::new("/.dockerenv").exists(),
        "cloud": cloud,
        "abilities": abilities,
        "other_abilities": other,
    })
}

/// The install's id, made and saved the first time.
async fn install_id(app: &App) -> Result<String> {
    let mut cfg = app.config.read().await.clone();
    if !cfg.telemetry.id.is_empty() {
        return Ok(cfg.telemetry.id);
    }
    cfg.telemetry.id = uuid::Uuid::new_v4().to_string();
    let id = cfg.telemetry.id.clone();
    cfg.save(&app.store)?;
    app.store.commit("[auto] config: an id for the usage ping").await?;
    *app.config.write().await = cfg;
    app.bus.emit(None, EventKind::ConfigChanged { by: "kiqr".into(), restart: false });
    Ok(id)
}

async fn ping(app: &App) -> Result<()> {
    let id = install_id(app).await?;
    let body = payload(app, Some(&id)).await;
    let url = format!("{}/api/v1/ping", app.config.read().await.cloud.url.trim_end_matches('/'));
    let client = reqwest::Client::builder().user_agent(format!("KIQR/{VERSION}")).timeout(Duration::from_secs(5)).build()?;
    client.post(&url).json(&body).send().await?.error_for_status()?;
    Ok(())
}

/// Pings shortly after start, every 15 minutes, and soon after an ability is installed.
pub async fn run(app: App) {
    let mut events = app.bus.subscribe();
    tokio::time::sleep(Duration::from_secs(30)).await;
    loop {
        if off_reason(&app).await.is_none() {
            if let Err(e) = ping(&app).await {
                tracing::debug!(error = %e, "usage ping failed");
            }
        }
        // A little jitter so many installs don't ping at the same moment.
        let next = tokio::time::sleep(EVERY + Duration::from_secs(u64::from(std::process::id() % 60)));
        tokio::pin!(next);
        loop {
            tokio::select! {
                _ = &mut next => break,
                ev = events.recv() => match ev {
                    Ok(ev) if matches!(ev.kind, EventKind::ExtensionInstalled { .. }) => {
                        // Let the rest of the install (accounts, reload) settle first.
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        break;
                    }
                    Err(RecvError::Closed) => return,
                    _ => {}
                },
            }
        }
    }
}

/// `GET /api/telemetry`: whether the ping is on, why not, where it goes and exactly what it sends.
async fn show(State(app): State<App>) -> ApiResult<Value> {
    let (id, url) = {
        let cfg = app.config.read().await;
        (cfg.telemetry.id.clone(), format!("{}/api/v1/ping", cfg.cloud.url.trim_end_matches('/')))
    };
    let off = off_reason(&app).await;
    Ok(Json(json!({
        "enabled": off.is_none(),
        "off_by": off,
        "endpoint": url,
        "every_minutes": EVERY.as_secs() / 60,
        "payload": payload(&app, (!id.is_empty()).then_some(id.as_str())).await,
    })))
}

#[derive(Deserialize)]
struct SetBody {
    enabled: bool,
}

/// `PUT /api/telemetry {enabled}`: switch the ping on or off in `config.yaml`.
async fn set(State(app): State<App>, Json(b): Json<SetBody>) -> ApiResult<Value> {
    let mut cfg = app.config.read().await.clone();
    cfg.telemetry.enabled = b.enabled;
    crate::api::apply_config(&app, cfg, "you", false).await?;
    show(State(app)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_listed_abilities_are_named() {
        let known: HashSet<String> = ["github", "wordpress"].into_iter().map(String::from).collect();
        let (named, other) = split(["wordpress", "my-private-crm", "github", "github"].into_iter().map(String::from), &known);
        assert_eq!(named, vec!["github", "wordpress"]);
        assert_eq!(other, 1);
    }

    #[test]
    fn switched_off_from_the_environment() {
        let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
        assert_eq!(off_by_env(env(&[])), None);
        assert_eq!(off_by_env(env(&[("KIQR_TELEMETRY", "0")])), Some("KIQR_TELEMETRY"));
        assert_eq!(off_by_env(env(&[("KIQR_TELEMETRY", "false")])), Some("KIQR_TELEMETRY"));
        assert_eq!(off_by_env(env(&[("KIQR_TELEMETRY", "1")])), None);
        assert_eq!(off_by_env(env(&[("DO_NOT_TRACK", "1")])), Some("DO_NOT_TRACK"));
        assert_eq!(off_by_env(env(&[("DO_NOT_TRACK", "0")])), None);
        assert_eq!(off_by_env(env(&[("DO_NOT_TRACK", "")])), None);
    }
}
