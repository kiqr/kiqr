//! Credentials at work in a project: which ones a call of an ability goes
//! through (`pick`: the ones the project granted it that fit its needs),
//! what is still missing for an ability, whether a credential works right
//! now (`state`, one rule for the routes, the executor and the UI), renewing
//! a sign-in's token into the Vault, and what a project picks once for an
//! ability (`settings`), read through a credential.

use crate::api::{ApiError, ApiResult};
use crate::App;
use anyhow::{anyhow, bail, Result};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use kiqr_core::access::Access;
use kiqr_core::conn::{self, Conn, Facts, State as CredState};
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{CredentialNeed, Manifest, Transport};
use kiqr_core::project::Project;
use kiqr_core::vault::Credential;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/abilities/{ability}/settings", put(put_settings))
        .route("/api/projects/{slug}/abilities/{ability}/choices/{name}", get(choices))
        .route("/api/vault/{id}/activity", get(activity))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

/// The installed manifest of an ability, as this project sees it.
pub async fn manifest(app: &App, slug: &str, ability: &str) -> Option<Manifest> {
    let reg = app.registry.read().await;
    reg.entry(Some(slug), ability).filter(|e| e.error.is_none()).and_then(|e| e.manifest.clone())
}

/// What the project picked for an ability, as text by name (`property` → `123`).
pub fn settings_of(app: &App, slug: &str, ability: &str) -> BTreeMap<String, String> {
    Project::load(&app.store, slug).ok().and_then(|p| p.ability_settings.get(ability).cloned()).map(|s| s.values.into_iter().map(|(k, v)| (k, v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))).collect()).unwrap_or_default()
}

/// A lookup for a need's `${SITE}` hosts: the ability's settings by their name in capitals.
fn settings_env(settings: &BTreeMap<String, String>) -> impl Fn(&str) -> Option<String> {
    let set: BTreeMap<String, String> = settings.iter().map(|(k, v)| (k.to_ascii_uppercase(), v.clone())).collect();
    move |k: &str| set.get(k).cloned()
}

/// The credentials a project granted an ability, with their values, in the order granted.
pub fn granted(app: &App, slug: &str, ability: &str) -> Vec<Credential> {
    let access = Access::load(&app.store, slug).unwrap_or_default();
    access.granted(ability).iter().filter_map(|id| app.vault.get(id).ok().flatten()).collect()
}

/// Every credential in the Vault that fits a need (granted or not), for the Access tab and recommendations.
pub fn fitting(app: &App, slug: &str, m: &Manifest, need: &CredentialNeed) -> Vec<Credential> {
    let env = settings_env(&settings_of(app, slug, &m.id));
    app.vault.all().unwrap_or_default().into_iter().filter(|c| need.fits(c, &env)).collect()
}

/// What a call of an ability goes through: for each need, the credential named
/// (only one the project granted it), else the one its settings name, else the
/// first granted that fits. None for an ability that asks for nothing. A
/// required need with nothing granted is refused with what to do.
pub async fn pick(app: &App, slug: &str, m: &Manifest, named: Option<&str>) -> Result<Option<Conn>> {
    let needs = m.needs();
    if needs.is_empty() {
        return Ok(None);
    }
    let settings = settings_of(app, slug, &m.id);
    let env = settings_env(&settings);
    let grants = granted(app, slug, &m.id);
    let setting_cred = Project::load(&app.store, slug).ok().and_then(|p| p.ability_settings.get(&m.id).map(|s| s.credential.clone())).filter(|c| !c.is_empty());
    let named = named.map(str::trim).filter(|n| !n.is_empty());
    if let Some(n) = named {
        if !grants.iter().any(|c| c.id == n || c.label.eq_ignore_ascii_case(n)) {
            let have: Vec<String> = grants.iter().map(|c| c.label.clone()).collect();
            bail!("{} may not use '{n}' in this project{}", m.name, if have.is_empty() { String::new() } else { format!(" (it may use {})", have.join(", ")) });
        }
    }
    let mut given = Vec::new();
    for need in &needs {
        let fits: Vec<&Credential> = grants.iter().filter(|c| need.fits(c, &env)).collect();
        let chosen = named
            .and_then(|n| fits.iter().find(|c| c.id == n || c.label.eq_ignore_ascii_case(n)))
            .or_else(|| setting_cred.as_deref().and_then(|s| fits.iter().find(|c| c.id == s)))
            .or_else(|| fits.first())
            .map(|c| (*c).clone());
        match chosen {
            Some(c) => given.push((need.clone(), c)),
            None if need.optional => {}
            None => bail!("{} needs {}; the user allows one under Setup → Abilities → {}", m.name, need.describe(), m.name),
        }
    }
    if given.is_empty() {
        return Ok(None);
    }
    let mut conn = Conn { given };
    renew(app, &mut conn).await?;
    Ok(Some(conn))
}

/// What an ability still needs in a project, in words ("a login for x.com"):
/// its required needs that nothing granted fits, and granted ones not complete.
pub fn missing(app: &App, slug: &str, m: &Manifest) -> Vec<String> {
    let env = settings_env(&settings_of(app, slug, &m.id));
    let grants = granted(app, slug, &m.id);
    m.needs().iter().filter(|n| !n.optional).filter(|n| !grants.iter().any(|c| n.fits(c, &env) && c.missing().is_empty())).map(|n| n.describe()).collect()
}

/// The installed ability (in this project) that signs a credential in on a hosted program: one with a `session` that fits it.
pub async fn session_manifest(app: &App, slug: &str, c: &Credential) -> Option<Manifest> {
    let reg = app.registry.read().await;
    let env = |_: &str| None;
    let all: Vec<&Manifest> = reg.visible(slug).into_iter().filter(|e| e.error.is_none()).filter_map(|e| e.manifest.as_ref()).filter(|m| m.session.is_some()).collect();
    // One for the credential's own site first (X for an x.com login); a site each project names (the browser) only after.
    let fixed = |m: &&&Manifest| m.needs().iter().any(|n| !n.hosts.is_empty() && n.hosts.iter().all(|h| !h.contains('$')) && n.fits(c, &env));
    all.iter().find(fixed).or_else(|| all.iter().find(|m| m.needs().iter().any(|n| n.fits(c, &env)))).map(|m| (*m).clone())
}

/// Whether a credential works right now: its fields, then the program its
/// sign-in runs on, then a run-out token, then the sign-in itself.
pub async fn state(app: &App, slug: &str, c: &Credential) -> CredState {
    let home = app.store.home();
    let mut facts = Facts { missing_fields: c.missing(), session: conn::session_of(home, &c.id), ..Default::default() };
    if let Some(m) = session_manifest(app, slug, c).await {
        facts.has_session = true;
        if m.transport == Transport::Mcp && !crate::ext::program_found(app, Some(slug), &m) {
            let p = crate::ext::program_of_sync(app, Some(slug), &m).filter(|_| !m.program.is_empty()).map(|p| p.name).unwrap_or_else(|| if m.program.is_empty() { m.command.clone() } else { m.program.clone() });
            facts.missing_programs.push(p);
        }
    }
    if c.template == "oauth" {
        let expires = c.field("expires_at").and_then(|f| f.value.parse::<i64>().ok());
        let can_renew = !c.provider.as_deref().unwrap_or_default().is_empty() && c.field("refresh").is_some_and(|f| !f.value.is_empty());
        facts.expired = expires.is_some_and(|e| e < chrono::Utc::now().timestamp()) && !can_renew;
    }
    facts.state()
}

/// The state as the UI reads it: the word, the details, and a sentence ("Signed out").
pub fn state_view(s: &CredState) -> Value {
    let say = match s {
        CredState::MissingFields { fields } => format!("Missing {}", fields.join(", ")),
        CredState::NeedsProgram { programs } => format!("Needs a program installed first: {}", programs.join(", ")),
        CredState::SignedOut => "Signed out".to_string(),
        CredState::NeedsHand => "Needs a hand to sign in".to_string(),
        CredState::Checking => "Signing in".to_string(),
        CredState::Expired => "Sign in again".to_string(),
        CredState::Ready => "Ready".to_string(),
    };
    let mut v = serde_json::to_value(s).unwrap_or(Value::Null);
    v["ready"] = json!(s.ready());
    v["says"] = json!(say);
    v
}

/// The state in a sentence that names the credential, for errors and Seemo: "u/acme_dev: signed out".
pub fn state_text(label: &str, s: &CredState) -> String {
    let says = state_view(s)["says"].as_str().unwrap_or("not ready").to_string();
    let mut ch = says.chars();
    let lower = ch.next().map(|f| f.to_lowercase().collect::<String>() + ch.as_str()).unwrap_or_default();
    format!("{label}: {lower}")
}

/// A sign-in's token about to run out is renewed through kiqr.cloud (the
/// provider it came from) and kept in the Vault; the call gets the new one.
pub async fn renew(app: &App, conn: &mut Conn) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    for (_, c) in conn.given.iter_mut() {
        let provider = c.provider.clone().unwrap_or_default();
        if c.template != "oauth" || provider.is_empty() {
            continue;
        }
        let Some(exp) = c.field("expires_at").and_then(|f| f.value.parse::<i64>().ok()) else { continue };
        if exp - 120 > now {
            continue;
        }
        let Some(refresh) = c.field("refresh").map(|f| f.value.clone()).filter(|r| !r.is_empty()) else { continue };
        let body = crate::signins::cloud_connect(app, &provider, "refresh", json!({ "refresh_token": refresh })).await.map_err(|e| anyhow!("{} needs signing in again: {e:#}", c.label))?;
        let token = body["access_token"].as_str().filter(|t| !t.is_empty()).ok_or_else(|| anyhow!("{} needs signing in again", c.label))?.to_string();
        let mut values = vec![("token".to_string(), token)];
        if let Some(r) = body["refresh_token"].as_str().filter(|r| !r.is_empty()) {
            values.push(("refresh".into(), r.to_string()));
        }
        if let Some(e) = body["expires_in"].as_i64() {
            values.push(("expires_at".into(), (now + e).to_string()));
        }
        let pairs: Vec<(&str, &str)> = values.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        app.vault.set_values(&c.id, &pairs)?;
        if let Some(fresh) = app.vault.get(&c.id)? {
            *c = fresh;
        }
    }
    Ok(())
}

/// How a credential is named where it acts: its identity when it has one ("@acme"), else its label.
pub fn name_of(c: &Credential) -> String {
    c.identity().filter(|i| !i.is_empty()).unwrap_or_else(|| c.label.clone())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ActivityQuery {
    limit: Option<usize>,
}

/// `GET /api/vault/{id}/activity`: what abilities did as a credential, in every project, newest first.
async fn activity(State(app): State<App>, Path(id): Path<String>, Query(q): Query<ActivityQuery>) -> ApiResult<Value> {
    let c = app.vault.view(&id)?.ok_or_else(|| ApiError(StatusCode::NOT_FOUND, format!("no credential {id}")))?;
    let home = app.store.home();
    let mut all = conn::activity_of(home, &id);
    let today = chrono::Utc::now().date_naive();
    let count = all.iter().filter(|a| a.role != "read" && a.at.date_naive() == today).count();
    all.reverse();
    all.truncate(q.limit.unwrap_or(50).clamp(1, 500));
    Ok(Json(json!({ "credential": c.id, "label": c.label, "today": count, "activity": all })))
}

/// `GET …/abilities/{ability}/choices/{name}`: what a setting can be, read through the credential its calls go through.
async fn choices(State(app): State<App>, Path((slug, ability, name)): Path<(String, String, String)>, Query(q): Query<BTreeMap<String, String>>) -> ApiResult<Value> {
    let m = manifest(&app, &slug, &ability).await.ok_or_else(|| bad(format!("'{ability}' is not installed")))?;
    let conn = pick(&app, &slug, &m, q.get("credential").map(String::as_str)).await.map_err(|e| bad(e.to_string()))?.ok_or_else(|| bad(format!("{} reads its choices through a credential; allow one first", m.name)))?;
    let found = crate::ext::key_choices(&app, &slug, &m, &conn, &name).await.map_err(|e| bad(format!("{e:#}")))?;
    Ok(Json(json!({ "name": name, "choices": found })))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SettingsBody {
    /// The credential its calls go through (and the values were picked through); the first granted when empty.
    credential: String,
    values: BTreeMap<String, Value>,
}

/// `PUT …/abilities/{ability}/settings {credential, values}`: what the project
/// picks once for an ability (a GA4 property, a site's address). Kept in `project.yaml`.
async fn put_settings(State(app): State<App>, Path((slug, ability)): Path<(String, String)>, Json(b): Json<SettingsBody>) -> ApiResult<Value> {
    let mut p = Project::load(&app.store, &slug)?;
    let m = manifest(&app, &slug, &ability).await.ok_or_else(|| bad(format!("'{ability}' is not installed")))?;
    if m.settings.is_empty() {
        return Err(bad(format!("{} has nothing to pick", m.name)));
    }
    let credential = match b.credential.trim() {
        "" => String::new(),
        c => granted(&app, &slug, &m.id).into_iter().find(|x| x.id == c || x.label.eq_ignore_ascii_case(c)).map(|x| x.id).ok_or_else(|| bad(format!("{} may not use '{c}' here", m.name)))?,
    };
    let mut values = BTreeMap::new();
    for (name, spec) in &m.settings {
        let label = spec["label"].as_str().unwrap_or(name);
        match b.values.get(name).map(|v| v.as_str().map(|s| s.trim().to_string()).unwrap_or_else(|| v.to_string())).filter(|v| !v.is_empty()) {
            Some(v) => {
                if let Some(opts) = spec["enum"].as_array().filter(|o| !o.is_empty()) {
                    if !opts.iter().any(|o| o.as_str() == Some(v.as_str())) {
                        return Err(bad(format!("{label} is one of {}", opts.iter().filter_map(|o| o.as_str()).collect::<Vec<_>>().join(", "))));
                    }
                }
                values.insert(name.clone(), Value::String(v));
            }
            None if !spec["optional"].as_bool().unwrap_or(false) => return Err(bad(format!("pick the {}", label.to_lowercase()))),
            None => {}
        }
    }
    if let Some(n) = b.values.keys().find(|k| !m.settings.contains_key(*k)) {
        return Err(bad(format!("{} has no setting '{n}'", m.name)));
    }
    p.ability_settings.insert(m.id.clone(), kiqr_core::project::AbilitySetting { credential: credential.clone(), values });
    p.save(&app.store)?;
    let what: Vec<String> = p.ability_settings[&m.id].values.values().filter_map(|v| v.as_str().map(str::to_string)).collect();
    app.store.commit(&format!("project({slug}): {} set to {}", m.name, what.join(", "))).await?;
    app.bus.emit(Some(&slug), EventKind::AbilitySettingsSaved { ability: m.id.clone(), credential });
    Ok(Json(json!({ "ability": m.id, "setting": p.ability_settings[&m.id] })))
}
