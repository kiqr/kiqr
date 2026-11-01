//! Signing an account in on a hosted program (a browser) and keeping it so.
//! The manifest's `session` says how: `check` steps tell whether the account
//! is signed in, `login` steps type the credentials, `blocked_when` texts mean
//! a person is needed. The credentials come from the account's login: saved
//! in KIQR (its own `.env`) or an item in a password manager account
//! (`provides.credential`). Only this runner ever reads them, right before
//! typing; the model, transcripts, events and logs see names only, and what
//! was typed is struck from every program answer while it is in use. When
//! the steps are blocked (a captcha, a device check) or fail twice, the
//! account needs a hand: a badge, a To do item, one Seemo line, and acting
//! tools refuse until a person finished the sign-in in the live view.

use crate::api::{ApiError, ApiResult};
use crate::App;
use anyhow::{anyhow, bail, Result};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use kiqr_core::connections::{Connection, Login, SessionStatus};
use kiqr_core::events::EventKind;
use kiqr_core::extensions::Manifest;
use serde_json::{json, Value};

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/connections/{id}/session", get(status_route))
        .route("/api/projects/{slug}/connections/{id}/signin", post(signin_route))
        .route("/api/projects/{slug}/connections/{id}/signout", post(signout_route))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

/// A login's values, for one sign-in. Dropped right after.
pub struct Credential {
    pub username: String,
    pub password: String,
    pub otp: Option<String>,
}

impl Credential {
    fn values(&self) -> Vec<String> {
        let mut v = vec![self.password.clone()];
        if let Some(o) = &self.otp {
            v.push(o.clone());
        }
        v.into_iter().filter(|s| s.len() >= 4).collect()
    }
}

/// The manifest of the service with a `session`, when one is on here.
pub async fn session_manifest(app: &App, slug: &str, c: &Connection) -> Option<Manifest> {
    let reg = app.registry.read().await;
    let off = crate::ext::project_off(app, slug);
    reg.of_service(slug, c.service()).into_iter().find(|m| m.session.is_some() && !off.contains(&m.id)).cloned()
}

/// Read the account's login: from its own `.env` (`store: kiqr`, with the
/// one-time code computed from the saved secret), or from a password manager
/// account through the `credential` contract.
pub async fn credential(app: &App, slug: &str, c: &Connection) -> Result<Credential> {
    let login = c.login.clone().ok_or_else(|| anyhow!("{} has no login saved; the user adds one on the account", c.label))?;
    let home = app.store.home();
    if login.store == Login::KIQR || login.store.is_empty() {
        let keys = c.keys(home, slug);
        let username = keys.get(Login::USERNAME).cloned().filter(|s| !s.trim().is_empty()).ok_or_else(|| anyhow!("{} has no user name saved", c.label))?;
        let password = keys.get(Login::PASSWORD).cloned().filter(|s| !s.is_empty()).ok_or_else(|| anyhow!("{} has no password saved", c.label))?;
        let otp = keys.get(Login::TOTP).filter(|s| !s.trim().is_empty()).map(|s| kiqr_core::totp::code(s).ok_or_else(|| anyhow!("the one-time code secret saved for {} is not valid", c.label))).transpose()?;
        return Ok(Credential { username, password, otp });
    }
    let manager = Connection::load(&app.store, slug, &login.store).map_err(|_| anyhow!("the password manager account '{}' is gone; pick where {}'s login lives again", login.store, c.label))?;
    let (m, provide) = {
        let reg = app.registry.read().await;
        reg.of_service(slug, manager.service()).into_iter().find_map(|m| m.provides.get(kiqr_core::abilities::CREDENTIAL).map(|p| (m.clone(), p.clone()))).ok_or_else(|| anyhow!("nothing on here reads logins from {}", manager.label))?
    };
    let out = crate::ext::run_through(app, slug, &m, &provide, None, None, &json!({ "op": "get", "ref": login.ref_ }), Some(&manager)).await?;
    let v = &out.json;
    let username = v["username"].as_str().unwrap_or("").to_string();
    let password = v["password"].as_str().unwrap_or("").to_string();
    if username.is_empty() || password.is_empty() {
        bail!("{} did not hand back a user name and password for {}", manager.label, login.ref_);
    }
    let otp = v["otp"].as_str().map(str::to_string).or_else(|| v["totp"].as_str().and_then(kiqr_core::totp::code));
    Ok(Credential { username, password, otp })
}

async fn set_state(app: &App, slug: &str, c: &Connection, state: &str, message: impl Into<String>) -> SessionStatus {
    let mut st = c.session(app.store.home(), slug);
    let now = chrono::Utc::now();
    st.state = state.to_string();
    st.message = message.into();
    st.checked_at = Some(now);
    if state == SessionStatus::SIGNED_IN {
        st.signed_in_at = Some(now);
    }
    let _ = c.set_session(app.store.home(), slug, &st);
    app.bus.emit(Some(slug), EventKind::SessionChanged { connection: c.id.clone(), state: st.state.clone(), message: st.message.clone() });
    st
}

/// Run the manifest's `check` steps: signed in when the last answer holds `signed_in_when`.
async fn run_check(app: &App, slug: &str, c: &Connection, m: &Manifest) -> Result<(bool, String)> {
    let sess = m.session.as_ref().ok_or_else(|| anyhow!("{} has no session", m.name))?;
    let program = crate::ext::program_of(app, Some(slug), m).await?;
    let env = kiqr_core::connections::env_for(app.store.home(), slug, Some(c), m);
    let launch = crate::ext::mcp_env(app, slug, m, Some(c), &env);
    let (last, text) = crate::ext::mcp_steps(app, &program, &launch, m, &sess.check, &json!({}), &Default::default(), &env).await?;
    // What the program said, as plain text: a program that answers with JSON (a snapshot) often
    // puts it in a string, so every string in the answer counts, unescaped.
    let text = match text {
        Some(t) if !last.is_object() => t,
        _ => flat_text(&last),
    };
    tracing::debug!(account = %c.id, steps = sess.check.len(), text = %kiqr_core::profile::truncate_chars(&text, 400), "session check read");
    let needle = sess.signed_in_when.trim();
    let lower = text.to_lowercase();
    let signed_in = if needle.is_empty() { !sess.signed_out_when.iter().any(|t| !t.trim().is_empty() && lower.contains(&t.trim().to_lowercase())) } else { text.contains(needle) };
    Ok((signed_in, text))
}

/// Every string in a JSON answer, joined; other values as JSON.
fn flat_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(flat_text).collect::<Vec<_>>().join("\n"),
        Value::Object(o) => o.values().map(flat_text).collect::<Vec<_>>().join("\n"),
        other => other.to_string(),
    }
}

fn blocked(sess: &kiqr_core::extensions::Session, text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    sess.blocked_when.iter().find(|b| !b.trim().is_empty() && lower.contains(&b.trim().to_lowercase())).cloned()
}

/// Check, and sign in when signed out: the heart of the runner. Holds the
/// account's turn. Returns the state it ended in.
pub async fn signin(app: &App, slug: &str, id: &str) -> Result<SessionStatus> {
    let c = Connection::load(&app.store, slug, id)?;
    let m = session_manifest(app, slug, &c).await.ok_or_else(|| anyhow!("{} has nothing that signs in", c.label))?;
    let sess = m.session.clone().expect("session_manifest");
    let _turn = crate::ext::account_turn(slug, &c.id).await;
    set_state(app, slug, &c, SessionStatus::CHECKING, "").await;
    let (ok, text) = match run_check(app, slug, &c, &m).await {
        Ok(x) => x,
        Err(e) => return Ok(set_state(app, slug, &c, SessionStatus::NEEDS_HAND, format!("could not check the sign-in: {e:#}")).await),
    };
    if ok {
        return Ok(set_state(app, slug, &c, SessionStatus::SIGNED_IN, "").await);
    }
    if let Some(b) = blocked(&sess, &text) {
        return Ok(set_state(app, slug, &c, SessionStatus::NEEDS_HAND, format!("the site asks for a person: {b}")).await);
    }
    if sess.login.is_empty() {
        return Ok(set_state(app, slug, &c, SessionStatus::NEEDS_HAND, "signed out; sign in once in the live view").await);
    }
    let cred = match credential(app, slug, &c).await {
        Ok(cr) => cr,
        Err(e) => return Ok(set_state(app, slug, &c, SessionStatus::NEEDS_HAND, format!("{e:#}")).await),
    };
    // What is typed never comes back in an answer, a transcript or a log while it is in use.
    let struck = cred.values();
    app.redact.write().unwrap_or_else(|e| e.into_inner()).extend(struck.iter().cloned());
    let result = run_login(app, slug, &c, &m, &sess, &cred).await;
    drop(cred);
    let outcome = match result {
        Ok(Some(b)) => set_state(app, slug, &c, SessionStatus::NEEDS_HAND, format!("the site asks for a person: {b}")).await,
        Ok(None) => match run_check(app, slug, &c, &m).await {
            Ok((true, _)) => set_state(app, slug, &c, SessionStatus::SIGNED_IN, "").await,
            Ok((false, t)) => set_state(app, slug, &c, SessionStatus::NEEDS_HAND, blocked(&sess, &t).map(|b| format!("the site asks for a person: {b}")).unwrap_or_else(|| "the sign-in did not go through; finish it in the live view".into())).await,
            Err(e) => set_state(app, slug, &c, SessionStatus::NEEDS_HAND, format!("could not check the sign-in: {e:#}")).await,
        },
        Err(e) => set_state(app, slug, &c, SessionStatus::NEEDS_HAND, crate::redact(app, &format!("the sign-in failed: {e:#}"))).await,
    };
    app.redact.write().unwrap_or_else(|e| e.into_inner()).retain(|v| !struck.contains(v));
    Ok(outcome)
}

/// The `login` steps with the credentials in their context, one at a time so
/// a block is seen as soon as it shows. Returns what blocked, if anything.
async fn run_login(app: &App, slug: &str, c: &Connection, m: &Manifest, sess: &kiqr_core::extensions::Session, cred: &Credential) -> Result<Option<String>> {
    let program = crate::ext::program_of(app, Some(slug), m).await?;
    let env = kiqr_core::connections::env_for(app.store.home(), slug, Some(c), m);
    let launch = crate::ext::mcp_env(app, slug, m, Some(c), &env);
    let args = json!({ "credential": { "username": cred.username, "password": cred.password, "otp": cred.otp.clone().unwrap_or_default() } });
    for st in &sess.login {
        let one = std::slice::from_ref(st);
        let (last, text) = crate::ext::mcp_steps(app, &program, &launch, m, one, &args, &Default::default(), &env).await?;
        let said = match text { Some(t) if !last.is_object() => t, _ => flat_text(&last) };
        if let Some(b) = blocked(sess, &said) {
            return Ok(Some(b));
        }
    }
    Ok(None)
}

/// Forget the sign-in: stop the account's programs and wipe what they kept
/// (the browser profile). The login stays.
pub async fn signout(app: &App, slug: &str, id: &str) -> Result<SessionStatus> {
    let c = Connection::load(&app.store, slug, id)?;
    app.mcp.stop_connection(slug, &c.id).await;
    let dir = c.data_dir(app.store.home(), slug);
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            // The log and the status stay; everything a program kept goes.
            if e.file_name() != "status.json" {
                let _ = if e.path().is_dir() { std::fs::remove_dir_all(e.path()) } else { std::fs::remove_file(e.path()) };
            }
        }
    }
    Ok(set_state(app, slug, &c, SessionStatus::SIGNED_OUT, "signed out").await)
}

/// The scheduler's tick: accounts with a session whose check is due.
pub async fn due(app: &App, slug: &str) {
    for c in Connection::list(&app.store, slug).unwrap_or_default() {
        let Some(m) = session_manifest(app, slug, &c).await else { continue };
        if !crate::accounts::ready(app, slug, &c, &m) {
            continue;
        }
        let st = c.session(app.store.home(), slug);
        // A hand is needed: nothing to do until a person did it.
        if st.needs_hand() {
            continue;
        }
        let every = m.session.as_ref().map(|s| s.every()).unwrap_or_default();
        let due = st.checked_at.is_none_or(|t| chrono::Utc::now().signed_duration_since(t).to_std().unwrap_or_default() >= every);
        if !due {
            continue;
        }
        let key = format!("session:{slug}/{}", c.id);
        if !app.jobs.write().await.insert(key.clone()) {
            continue;
        }
        let (app2, slug2, id) = (app.clone(), slug.to_string(), c.id.clone());
        tokio::spawn(async move {
            match signin(&app2, &slug2, &id).await {
                Ok(st) if st.needs_hand() => app2.bus.emit(Some(&slug2), EventKind::SeemoSays { text: format!("{} needs a hand to sign in: {}. Open the account under Setup → Accounts to finish it.", id, st.message) }),
                Ok(_) => {}
                Err(e) => tracing::warn!(slug = %slug2, account = %id, "session check failed: {e:#}"),
            }
            app2.jobs.write().await.remove(&key);
        });
    }
}

/// For the account view: how the sign-in stands, and whether this account signs in at all.
pub async fn view(app: &App, slug: &str, c: &Connection) -> Value {
    let m = session_manifest(app, slug, c).await;
    json!({
        "has_session": m.is_some(),
        "session": c.session(app.store.home(), slug),
        "login": c.login,
        "live_view": m.as_ref().and_then(|m| m.live_view.as_ref()).is_some(),
        // No login steps: a person signs in once in the live view; the program keeps it.
        "by_hand": m.as_ref().and_then(|m| m.session.as_ref()).is_some_and(|s| s.login.is_empty()),
        "start_url": m.as_ref().and_then(|m| start_url(app, slug, c, m)),
    })
}

/// Where the live view opens: the first page the session's check visits, with the account's keys filled.
pub fn start_url(app: &App, slug: &str, c: &Connection, m: &Manifest) -> Option<String> {
    let env = kiqr_core::connections::env_for(app.store.home(), slug, Some(c), m);
    let st = m.session.as_ref()?.check.iter().find(|s| s.args.as_ref().is_some_and(|a| a.get("url").is_some()))?;
    let u = kiqr_core::extensions::fill(st.args.as_ref()?["url"].as_str()?, &json!({}), env);
    (!u.contains("${")).then_some(u)
}

async fn status_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let c = Connection::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    Ok(Json(view(&app, &slug, &c).await))
}

/// `POST …/signin`: check and sign in now, in the background; the status says how it went.
async fn signin_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let c = Connection::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    let m = session_manifest(&app, &slug, &c).await.ok_or_else(|| bad(format!("{} has nothing that signs in", c.label)))?;
    if !crate::accounts::ready(&app, &slug, &c, &m) {
        return Err(bad(format!("{} is not ready: its program is not installed yet", c.label)));
    }
    let key = format!("session:{slug}/{}", c.id);
    if !app.jobs.write().await.insert(key.clone()) {
        return Ok(Json(json!({ "started": false, "note": "a sign-in is running already", "session": c.session(app.store.home(), &slug) })));
    }
    let (app2, slug2, id2) = (app.clone(), slug.clone(), c.id.clone());
    tokio::spawn(async move {
        if let Err(e) = signin(&app2, &slug2, &id2).await {
            tracing::warn!(slug = %slug2, account = %id2, "sign-in failed: {e:#}");
        }
        app2.jobs.write().await.remove(&key);
    });
    Ok(Json(json!({ "started": true })))
}

async fn signout_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let st = signout(&app, &slug, &id).await.map_err(|e| bad(e.to_string()))?;
    Ok(Json(json!({ "session": st })))
}
