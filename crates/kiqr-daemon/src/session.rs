//! Signing an account in on a hosted program (a browser) and keeping it so.
//! The manifest's `session` says how: `check` steps tell whether the account
//! is signed in; the sign-in starts at `login_url`, where the model reads each
//! page's controls (any site, any language) and names the user, password and
//! code fields, and the runner types into those and nothing else (`guided`
//! below); or `login` steps type the credentials into fixed fields;
//! `blocked_when` texts mean a person is needed, and so does the model
//! saying the page is a captcha or a check. The credentials come from the account's login: saved
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
use kiqr_core::accounts::{Account, Secrets, SessionStatus};
use kiqr_core::events::EventKind;
use kiqr_core::extensions::Manifest;
use serde_json::{json, Value};

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/accounts/{id}/session", get(status_route))
        .route("/api/projects/{slug}/accounts/{id}/signin", post(signin_route))
        .route("/api/projects/{slug}/accounts/{id}/signout", post(signout_route))
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
pub async fn session_manifest(app: &App, slug: &str, c: &Account) -> Option<Manifest> {
    let reg = app.registry.read().await;
    let off = crate::ext::project_off(app, slug);
    reg.of_service(slug, &c.service).into_iter().find(|m| m.session.is_some() && !off.contains(&m.id)).cloned()
}

/// What a password manager holds for an account (its `secrets.item`), through
/// the manager's `credential` contract: `username`, `password`, `otp` or
/// `totp`, and `fields` (typed keys by name). Read at call time, never kept.
pub async fn from_manager(app: &App, slug: &str, c: &Account) -> Result<Value> {
    let manager = Account::load(&app.store, slug, &c.secrets.store).map_err(|_| anyhow!("the password manager account '{}' is gone; pick where {}'s secrets are kept again", c.secrets.store, c.label))?;
    let (m, provide) = {
        let reg = app.registry.read().await;
        reg.of_service(slug, &manager.service).into_iter().find_map(|m| m.provides.get(kiqr_core::abilities::CREDENTIAL).map(|p| (m.clone(), p.clone()))).ok_or_else(|| anyhow!("nothing on here reads secrets from {}", manager.label))?
    };
    let out = crate::ext::run_through(app, slug, &m, &provide, None, None, &json!({ "op": "get", "ref": c.secrets.item }), Some(&manager)).await?;
    Ok(out.json)
}

/// The typed keys a password manager holds for an account, by name; empty
/// when its secrets are kept in KIQR or the manager does not answer.
pub async fn vault_keys(app: &App, slug: &str, c: Option<&Account>) -> std::collections::BTreeMap<String, String> {
    let Some(c) = c.filter(|c| !c.secrets.in_kiqr()) else { return Default::default() };
    match Box::pin(from_manager(app, slug, c)).await {
        Ok(v) => v["fields"].as_object().map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect()).unwrap_or_default(),
        Err(e) => {
            tracing::warn!(account = %c.id, "could not read keys from the password manager: {e:#}");
            Default::default()
        }
    }
}

/// Read the account's login: from its own `.env` (kept in KIQR, with the
/// one-time code computed from the saved secret), or from its password manager.
pub async fn credential(app: &App, slug: &str, c: &Account) -> Result<Credential> {
    let home = app.store.home();
    if c.secrets.in_kiqr() {
        let keys = c.keys(home, slug);
        let username = keys.get(Secrets::USERNAME).cloned().filter(|s| !s.trim().is_empty()).ok_or_else(|| anyhow!("{} has no login saved; the user adds one on the account", c.label))?;
        let password = keys.get(Secrets::PASSWORD).cloned().filter(|s| !s.is_empty()).ok_or_else(|| anyhow!("{} has no password saved", c.label))?;
        let otp = keys.get(Secrets::TOTP).filter(|s| !s.trim().is_empty()).map(|s| kiqr_core::totp::code(s).ok_or_else(|| anyhow!("the one-time code secret saved for {} is not valid", c.label))).transpose()?;
        return Ok(Credential { username, password, otp });
    }
    let v = from_manager(app, slug, c).await?;
    let username = v["username"].as_str().unwrap_or("").to_string();
    let password = v["password"].as_str().unwrap_or("").to_string();
    if username.is_empty() || password.is_empty() {
        bail!("{} did not hand back a user name and password for {}", c.secrets.store, c.secrets.item);
    }
    let otp = v["otp"].as_str().map(str::to_string).or_else(|| v["totp"].as_str().and_then(kiqr_core::totp::code));
    Ok(Credential { username, password, otp })
}

/// Whether the account has a login to sign in with: saved in KIQR, or kept in a password manager.
pub fn has_login(app: &App, slug: &str, c: &Account) -> bool {
    !c.secrets.in_kiqr() || c.keys(app.store.home(), slug).get(Secrets::USERNAME).is_some_and(|u| !u.trim().is_empty())
}

async fn set_state(app: &App, slug: &str, c: &Account, state: &str, message: impl Into<String>) -> SessionStatus {
    let mut st = c.session(app.store.home(), slug);
    let now = chrono::Utc::now();
    st.state = state.to_string();
    st.message = message.into();
    st.checked_at = Some(now);
    // Signing in again is the user's wish (or the scheduler's, which never runs after a sign-out by the user).
    st.by_user = false;
    if state == SessionStatus::SIGNED_IN {
        st.signed_in_at = Some(now);
    }
    let _ = c.set_session(app.store.home(), slug, &st);
    app.bus.emit(Some(slug), EventKind::AccountStateChanged { account: c.id.clone(), state: st.state.clone(), message: st.message.clone() });
    st
}

/// Run the manifest's `check` steps: signed in when the last answer holds `signed_in_when`.
/// With no marker the model reads the page, and its words come back as the third value.
async fn run_check(app: &App, slug: &str, c: &Account, m: &Manifest) -> Result<(bool, String, Option<String>)> {
    let sess = m.session.as_ref().ok_or_else(|| anyhow!("{} has no session", m.name))?;
    let program = crate::ext::program_of(app, Some(slug), m).await?;
    let env = kiqr_core::accounts::env_for(app.store.home(), slug, Some(c), m, Default::default());
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
    let markers = sess.signed_out_when.iter().filter(|t| !t.trim().is_empty()).count();
    let mut said = None;
    let signed_in = if !needle.is_empty() {
        text.contains(needle)
    } else if markers > 0 {
        !sess.signed_out_when.iter().any(|t| !t.trim().is_empty() && lower.contains(&t.trim().to_lowercase()))
    } else {
        // No marker to look for: the model reads the page.
        let read = guided::read_page(app, &guided::page_of(&last, &text), &[]).await?;
        said = Some(read.reason.trim().to_string()).filter(|r| !r.is_empty());
        read.state == guided::State::SignedIn
    };
    Ok((signed_in, text, said))
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
    let c = Account::load(&app.store, slug, id)?;
    let m = session_manifest(app, slug, &c).await.ok_or_else(|| anyhow!("{} has nothing that signs in", c.label))?;
    let sess = m.session.clone().expect("session_manifest");
    let _turn = crate::ext::account_turn(slug, &c.id).await;
    set_state(app, slug, &c, SessionStatus::CHECKING, "").await;
    let (ok, text, _) = match run_check(app, slug, &c, &m).await {
        Ok(x) => x,
        Err(e) => return Ok(set_state(app, slug, &c, SessionStatus::NEEDS_HAND, format!("could not check the sign-in: {e:#}")).await),
    };
    if ok {
        return Ok(set_state(app, slug, &c, SessionStatus::SIGNED_IN, "").await);
    }
    if let Some(b) = blocked(&sess, &text) {
        return Ok(set_state(app, slug, &c, SessionStatus::NEEDS_HAND, format!("the site asks for a person: {b}")).await);
    }
    if !sess.signs_in() {
        return Ok(set_state(app, slug, &c, SessionStatus::NEEDS_HAND, "signed out; sign in once in the live view").await);
    }
    let cred = match credential(app, slug, &c).await {
        Ok(cr) => cr,
        Err(e) => return Ok(set_state(app, slug, &c, SessionStatus::NEEDS_HAND, format!("{e:#}")).await),
    };
    // What is typed never comes back in an answer, a transcript or a log while it is in use.
    let struck = cred.values();
    app.redact.write().unwrap_or_else(|e| e.into_inner()).extend(struck.iter().cloned());
    set_state(app, slug, &c, SessionStatus::CHECKING, "signing in").await;
    let result = if sess.login_url.trim().is_empty() { run_login(app, slug, &c, &m, &sess, &cred).await } else { guided::run(app, slug, &c, &m, &sess, &cred).await };
    drop(cred);
    let outcome = match result {
        Ok(Some(b)) => set_state(app, slug, &c, SessionStatus::NEEDS_HAND, crate::redact(app, &format!("the site asks for a person: {b}"))).await,
        Ok(None) => match check_again_if_unread(app, slug, &c, &m, &sess, run_check(app, slug, &c, &m).await).await {
            Ok((true, _, _)) => set_state(app, slug, &c, SessionStatus::SIGNED_IN, "").await,
            // What the check saw, in the model's words when it read the page, so the message says why.
            Ok((false, t, said)) => set_state(app, slug, &c, SessionStatus::NEEDS_HAND, crate::redact(app, &blocked(&sess, &t).map(|b| format!("the site asks for a person: {b}")).unwrap_or_else(|| match said {
                Some(r) => format!("the sign-in seemed to go through, but the check afterwards found: {r} Finish it in the live view."),
                None if sess.login_url.trim().is_empty() => "the sign-in did not go through; finish it in the live view".into(),
                None => "the sign-in seemed to go through, but the check afterwards did not find you signed in; finish it in the live view".into(),
            }))).await,
            Err(e) => set_state(app, slug, &c, SessionStatus::NEEDS_HAND, format!("could not check the sign-in: {e:#}")).await,
        },
        Err(e) => set_state(app, slug, &c, SessionStatus::NEEDS_HAND, crate::redact(app, &format!("the sign-in failed: {e:#}"))).await,
    };
    app.redact.write().unwrap_or_else(|e| e.into_inner()).retain(|v| !struck.contains(v));
    Ok(outcome)
}

/// A check after signing in that found nothing to read (a page that is
/// still loading, or one whose tab the browser lost: X's home has crashed the
/// browser's page process right after a sign-in) is run once more, a few
/// seconds later, on a fresh load of the same page. The login is kept by then.
async fn check_again_if_unread(app: &App, slug: &str, c: &Account, m: &Manifest, sess: &kiqr_core::extensions::Session, first: Result<(bool, String, Option<String>)>) -> Result<(bool, String, Option<String>)> {
    match first {
        Ok((false, t, said)) if blocked(sess, &t).is_none() => {
            tracing::info!(account = %c.id, said = ?said, "the check after signing in found nothing it could read; checking once more");
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            match run_check(app, slug, c, m).await {
                Ok(again) => Ok(again),
                Err(_) => Ok((false, t, said)),
            }
        }
        other => other,
    }
}

/// The `login` steps with the credentials in their context, one at a time so
/// a block is seen as soon as it shows. Returns what blocked, if anything.
async fn run_login(app: &App, slug: &str, c: &Account, m: &Manifest, sess: &kiqr_core::extensions::Session, cred: &Credential) -> Result<Option<String>> {
    let program = crate::ext::program_of(app, Some(slug), m).await?;
    let env = kiqr_core::accounts::env_for(app.store.home(), slug, Some(c), m, Default::default());
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


/// The model-led sign-in: from `login_url`, each page is read as the program's
/// snapshot (title, URL, the visible controls, each with a selector the
/// program built; a password field's value is masked by the program), the
/// model says what the page is and which controls are the user, password and
/// code fields, and the runner types into them. The model never gets a
/// credential, and the runner types only into a control the page listed,
/// the password only into one of `type=password`, and only on the service's
/// own hosts: a page in any language works, and a site changing its markup
/// does not break it.
pub mod guided {
    use super::*;
    use kiqr_core::extensions::Step;
    use kiqr_core::provider::{self, Request};

    #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum State {
        SignedIn,
        Form,
        Code,
        Blocked,
        Unknown,
    }

    /// What the model said about one page.
    #[derive(Debug, serde::Deserialize)]
    #[serde(default)]
    pub struct Read {
        pub state: State,
        pub username: Option<String>,
        pub password: Option<String>,
        pub code: Option<String>,
        pub submit: Option<String>,
        pub reason: String,
    }

    impl Default for Read {
        fn default() -> Self {
            Read { state: State::Unknown, username: None, password: None, code: None, submit: None, reason: String::new() }
        }
    }

    /// At most this many pages per sign-in.
    const ROUNDS: usize = 6;
    /// The controls handed to the model: this many, these keys.
    const MAX_CONTROLS: usize = 80;
    const KEPT: &[&str] = &["selector", "tag", "type", "name", "id", "placeholder", "label", "text", "role", "checked"];

    const SYSTEM: &str = "You help KIQR sign a person in to a website in a browser KIQR drives. You get one page: its title, URL, its visible controls (from a snapshot) and its text (the labels, headings and notices that say which control is which), in whatever language the site uses, and which fields were typed already. You never get the password or any other value. Say what the page is and which controls to use. Answer with JSON only:\n{\"state\": \"signed_in\" | \"form\" | \"code\" | \"blocked\" | \"unknown\", \"username\": selector or null, \"password\": selector or null, \"code\": selector or null, \"submit\": selector or null, \"reason\": one short sentence in English about what the page shows}\n- signed_in: the person is signed in already (a feed, an account menu, a compose box, a welcome; no sign-in form).\n- form: a sign-in form asking for a user name, email or phone, a password, or both. A single user-name field with a Next button is still a form; the password comes on the next page.\n- code: the page asks for a one-time code, verification code or authenticator code.\n- blocked: a captcha or puzzle, an identity check, a phone or email confirmation, 'unusual activity', a suspended or locked account, a wrong-password notice, or anything else a person must do.\n- unknown: none of these.\nusername is the field for the user name, email or phone; password the password field; code the one-time code field; submit the button that sends the form (Log in, Next, Continue, Verify), or null when Enter in the last field does it.\nCopy selectors verbatim from the controls; never write one yourself. Ignore cookie banners, language pickers, 'remember me' boxes and links to register or reset a password; a cookie banner is not a block.";

    /// The page as the model gets it: from the program's snapshot when the answer is one, else the text.
    pub fn page_of(last: &Value, text: &str) -> Value {
        page_with(last, text, None)
    }

    /// The same, with the page's prose beside the controls (labels, headings, notices: what says which field is which).
    pub fn page_with(last: &Value, text: &str, prose: Option<&str>) -> Value {
        if let Some(snap) = snapshot_of(last) {
            let controls: Vec<Value> = snap["interactive_elements"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .take(MAX_CONTROLS)
                        .map(|e| Value::Object(e.as_object().map(|o| o.iter().filter(|(k, _)| KEPT.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default()))
                        .collect()
                })
                .unwrap_or_default();
            let mut page = json!({ "title": snap["title"], "url": snap["url"], "controls": controls });
            if let Some(p) = prose.map(str::trim).filter(|p| !p.is_empty()) {
                page["text"] = json!(kiqr_core::profile::truncate_chars(p, 2000));
            }
            return page;
        }
        json!({ "text": kiqr_core::profile::truncate_chars(text, 3000) })
    }

    /// The snapshot in a step's answer: the object itself, JSON in a string of
    /// it, or either inside a wrapper (`result`, `text`), however the program framed it.
    pub fn snapshot_of(last: &Value) -> Option<Value> {
        match last {
            Value::Object(o) => {
                if o.contains_key("interactive_elements") {
                    return Some(last.clone());
                }
                o.values().find_map(snapshot_of)
            }
            Value::Array(a) => a.iter().find_map(snapshot_of),
            Value::String(t) => {
                let t = t.trim();
                if !t.starts_with('{') {
                    return None;
                }
                let v: Value = serde_json::from_str(t).ok()?;
                v.get("interactive_elements").is_some().then_some(v)
            }
            _ => None,
        }
    }

    /// Ask the model what the page is and which controls to use.
    pub async fn read_page(app: &App, page: &Value, typed: &[&str]) -> Result<Read> {
        let spec = app.config.read().await.models.chat.clone();
        let (prov, model) = provider::resolve(&spec)?;
        let prompt = json!({ "page": page, "typed_already": typed }).to_string();
        let text = prov.complete(&Request { system: SYSTEM.into(), prompt, model, json: true, max_tokens: 400 }).await.map_err(|e| anyhow!("the model could not read the page: {e:#}"))?;
        let v = provider::extract_json(&text).map_err(|e| {
            tracing::debug!(answer = %kiqr_core::profile::truncate_chars(&text, 600), "the model's answer about the page was not JSON");
            e
        })?;
        let r: Read = serde_json::from_value(v).map_err(|e| anyhow!("the model's answer about the page made no sense: {e}"))?;
        tracing::info!(state = ?r.state, url = %page["url"].as_str().unwrap_or(""), user = r.username.is_some(), pass = r.password.is_some(), code = r.code.is_some(), reason = %r.reason, "sign-in page read");
        Ok(r)
    }

    /// The control the model named, as the page listed it: a text-like field
    /// (`password` only a password field, the others never one). None when
    /// the page listed nothing by that selector.
    pub fn field<'a>(snap: &'a Value, selector: Option<&str>, password: bool) -> Option<&'a Value> {
        let sel = selector?.trim();
        if sel.is_empty() {
            return None;
        }
        let e = snap["interactive_elements"].as_array()?.iter().find(|e| e["selector"].as_str() == Some(sel))?;
        let tag = e["tag"].as_str().unwrap_or("");
        let ty = e["type"].as_str().unwrap_or("text");
        let textual = tag == "textarea" || (tag == "input" && !["checkbox", "radio", "submit", "button", "file", "hidden", "image", "reset", "range", "color"].contains(&ty)) || e["role"].as_str() == Some("textbox");
        (textual && (ty == "password") == password).then_some(e)
    }

    /// A button the model named, as the page listed it.
    pub fn button<'a>(snap: &'a Value, selector: Option<&str>) -> Option<&'a Value> {
        let sel = selector?.trim();
        snap["interactive_elements"].as_array()?.iter().find(|e| e["selector"].as_str() == Some(sel) && !matches!(e["tag"].as_str(), Some("input") | Some("textarea")) || (e["selector"].as_str() == Some(sel) && matches!(e["type"].as_str(), Some("submit") | Some("button"))))
    }

    fn step(name: &str, call: &str, args: Value) -> Step {
        Step { name: name.into(), call: call.into(), args: Some(args), ..Default::default() }
    }

    /// Sign in from `login_url`. Returns what blocked, if anything; an error
    /// when the pages could not be followed.
    pub async fn run(app: &App, slug: &str, c: &Account, m: &Manifest, sess: &kiqr_core::extensions::Session, cred: &Credential) -> Result<Option<String>> {
        let program = crate::ext::program_of(app, Some(slug), m).await?;
        let env = kiqr_core::accounts::env_for(app.store.home(), slug, Some(c), m, Default::default());
        let launch = crate::ext::mcp_env(app, slug, m, Some(c), &env);
        let args = json!({ "credential": { "username": cred.username, "password": cred.password, "otp": cred.otp.clone().unwrap_or_default() } });
        let map: kiqr_core::extensions::ResultMap = Default::default();
        let go = |steps: Vec<Step>| {
            let (program, launch, args, map, env) = (&program, &launch, &args, &map, &env);
            async move { crate::ext::mcp_steps(app, program, launch, m, &steps, args, map, env).await }
        };
        let url = kiqr_core::extensions::fill(sess.login_url.trim(), &json!({}), env.clone());
        go(vec![step("open", "browser_navigate", json!({ "url": url, "wait_until": "domcontentloaded" }))]).await?;
        let mut typed: Vec<&str> = Vec::new();
        let mut forms = 0;
        for round in 0..ROUNDS {
            if round > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
            }
            let (last, text) = go(vec![step("look", "browser_snapshot", json!({}))]).await?;
            let snap = snapshot_of(&last).ok_or_else(|| anyhow!("the program's snapshot of the page could not be read"))?;
            // The page's words too: the labels and notices that say which field is which, in any language.
            let prose = go(vec![step("read", "browser_read_text", json!({ "selector": "body" }))]).await.ok().and_then(|(l, t)| t.or_else(|| l.get("text").and_then(Value::as_str).map(str::to_string)));
            let read = read_page(app, &page_with(&last, &text.unwrap_or_default(), prose.as_deref()), &typed).await?;
            match read.state {
                State::SignedIn => return Ok(None),
                State::Blocked => return Ok(Some(if read.reason.trim().is_empty() { "the page needs a person".into() } else { read.reason.trim().to_string() })),
                State::Unknown => {
                    // Before anything was typed the page may still be loading: one more look. After, it is where the sign-in ended up.
                    if typed.is_empty() && round == 0 {
                        continue;
                    }
                    bail!("after the sign-in the page at {} is not one I understand: {}", snap["url"].as_str().unwrap_or("?"), read.reason);
                }
                State::Form | State::Code => {}
            }
            // Nothing is typed off the service's own hosts.
            crate::ext::check_host(m, snap["url"].as_str().unwrap_or(""), &env)?;
            let mut steps = Vec::new();
            if read.state == State::Code {
                let Some(f) = field(&snap, read.code.as_deref(), false) else {
                    bail!("the page asks for a code but named no field for it");
                };
                if cred.otp.is_none() {
                    return Ok(Some("the site asks for a one-time code; add the code's secret to the login, or finish in the live view".into()));
                }
                steps.push(step("code", "browser_type", json!({ "selector": f["selector"], "text": "{{credential.otp}}" })));
                typed.push("code");
            } else {
                forms += 1;
                if forms > 3 {
                    bail!("the site keeps showing the sign-in form; the user name or password may be wrong");
                }
                let user = field(&snap, read.username.as_deref(), false);
                let pass = field(&snap, read.password.as_deref(), true);
                if user.is_none() && pass.is_none() {
                    bail!("the page shows a sign-in form but none of its fields could be used");
                }
                if let Some(f) = user {
                    steps.push(step("user", "browser_type", json!({ "selector": f["selector"], "text": "{{credential.username}}" })));
                    if !typed.contains(&"username") {
                        typed.push("username");
                    }
                }
                if let Some(f) = pass {
                    steps.push(step("pass", "browser_type", json!({ "selector": f["selector"], "text": "{{credential.password}}" })));
                    if !typed.contains(&"password") {
                        typed.push("password");
                    }
                }
            }
            match button(&snap, read.submit.as_deref()) {
                Some(b) => steps.push(step("go", "browser_click", json!({ "selector": b["selector"] }))),
                None => steps.push(step("go", "browser_press_key", json!({ "key": "Enter" }))),
            }
            let (last, text) = go(steps).await?;
            let said = match text { Some(t) if !last.is_object() => t, _ => flat_text(&last) };
            tracing::debug!(round, said = %crate::redact(app, &kiqr_core::profile::truncate_chars(&said, 300)), "sign-in page filled");
            if let Some(b) = blocked(sess, &said) {
                return Ok(Some(b));
            }
        }
        bail!("the sign-in did not finish within {ROUNDS} pages")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn snap() -> Value {
            json!({ "title": "Logga in", "url": "https://x.com/i/flow/login", "interactive_elements": [
                { "tag": "input", "type": "text", "name": "username_or_email", "selector": "input[name='username_or_email']", "placeholder": "Telefon, e-post eller användarnamn" },
                { "tag": "input", "type": "password", "name": "password", "selector": "input[name='password']" },
                { "tag": "button", "type": "submit", "selector": "button[type='submit']", "text": "Logga in" },
                { "tag": "input", "type": "checkbox", "selector": "input[name='remember']" },
                { "tag": "a", "selector": "a[href='/signup']", "text": "Registrera dig" }
            ] })
        }

        #[test]
        fn a_password_goes_only_into_a_password_field() {
            let s = snap();
            assert!(field(&s, Some("input[name='password']"), true).is_some());
            assert!(field(&s, Some("input[name='username_or_email']"), true).is_none(), "a text field never takes the password");
            assert!(field(&s, Some("input[name='password']"), false).is_none(), "the user name never goes into the password field");
            assert!(field(&s, Some("input[name='username_or_email']"), false).is_some());
        }

        #[test]
        fn only_controls_the_page_listed_count() {
            let s = snap();
            assert!(field(&s, Some("input[name='made_up']"), false).is_none());
            assert!(field(&s, Some("input[name='remember']"), false).is_none(), "a checkbox is not a field to type into");
            assert!(field(&s, None, false).is_none());
            assert!(button(&s, Some("button[type='submit']")).is_some());
            assert!(button(&s, Some("input[name='password']")).is_none(), "a field is not a button");
            assert!(button(&s, Some("a[href='/signup']")).is_some(), "a link can be the way on");
        }

        #[test]
        fn the_model_sees_controls_without_coordinates_and_the_text_when_there_is_no_snapshot() {
            let page = page_of(&json!({ "text": snap().to_string() }), "");
            assert_eq!(page["url"], "https://x.com/i/flow/login");
            assert_eq!(page["controls"].as_array().unwrap().len(), 5);
            assert!(page["controls"][0].get("at").is_none());
            let plain = page_of(&json!({ "text": "Welcome back, Jane" }), "Welcome back, Jane");
            assert_eq!(plain["text"], "Welcome back, Jane");
            let wrapped = page_of(&json!({ "result": snap().to_string() }), "");
            assert_eq!(wrapped["controls"].as_array().unwrap().len(), 5, "a snapshot inside a wrapper is found");
        }

        #[test]
        fn the_models_answer_parses_with_missing_fields() {
            let r: Read = serde_json::from_value(json!({ "state": "form", "password": "input[name='password']" })).unwrap();
            assert_eq!(r.state, State::Form);
            assert!(r.username.is_none());
            let r: Read = serde_json::from_value(json!({ "state": "blocked", "reason": "A captcha." })).unwrap();
            assert_eq!(r.state, State::Blocked);
        }
    }
}

/// Forget the sign-in: stop the account's programs and wipe what they kept
/// (the browser profile). The login stays.
pub async fn signout(app: &App, slug: &str, id: &str) -> Result<SessionStatus> {
    let c = Account::load(&app.store, slug, id)?;
    app.mcp.stop_account(slug, &c.id).await;
    let dir = c.data_dir(app.store.home(), slug);
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            // The log and the status stay; everything a program kept goes.
            if e.file_name() != "status.json" {
                let _ = if e.path().is_dir() { std::fs::remove_dir_all(e.path()) } else { std::fs::remove_file(e.path()) };
            }
        }
    }
    let mut st = set_state(app, slug, &c, SessionStatus::SIGNED_OUT, "signed out").await;
    // Nothing signs it in again until the user does.
    st.by_user = true;
    let _ = c.set_session(app.store.home(), slug, &st);
    Ok(st)
}

/// Keys and programs are there, so a sign-in can run (the session aside).
async fn can_sign_in(app: &App, slug: &str, c: &Account) -> bool {
    !matches!(crate::accounts::state(app, slug, c).await, kiqr_core::accounts::State::MissingKeys { .. } | kiqr_core::accounts::State::NeedsProgram { .. })
}

/// The scheduler's tick: accounts with a session whose check is due.
pub async fn due(app: &App, slug: &str) {
    for c in Account::list(&app.store, slug).unwrap_or_default() {
        let Some(m) = session_manifest(app, slug, &c).await else { continue };
        if !can_sign_in(app, slug, &c).await {
            continue;
        }
        let st = c.session(app.store.home(), slug);
        // A hand is needed, or the user signed it out: nothing to do until a person says so.
        if st.needs_hand() || st.by_user {
            continue;
        }
        // Never checked and no login to sign in with: the user is still connecting it (the wizard signs it in).
        if st.checked_at.is_none() && m.session.as_ref().is_some_and(|s| s.signs_in()) && !has_login(app, slug, &c) {
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
        let (app2, slug2, id, label) = (app.clone(), slug.to_string(), c.id.clone(), c.label.clone());
        tokio::spawn(async move {
            match signin(&app2, &slug2, &id).await {
                Ok(st) if st.needs_hand() => app2.bus.emit(Some(&slug2), EventKind::SeemoSays { text: format!("{label} needs a hand to sign in: {}. Open it under Setup → Accounts to finish.", st.message) }),
                Ok(_) => {}
                Err(e) => tracing::warn!(slug = %slug2, account = %id, "session check failed: {e:#}"),
            }
            app2.jobs.write().await.remove(&key);
        });
    }
}

/// For the account view: how the sign-in stands, and whether this account signs in at all.
pub async fn view(app: &App, slug: &str, c: &Account) -> Value {
    let m = session_manifest(app, slug, c).await;
    json!({
        "has_session": m.is_some(),
        "session": c.session(app.store.home(), slug),
        "has_login": has_login(app, slug, c),
        "live_view": m.as_ref().and_then(|m| m.live_view.as_ref()).is_some(),
        // No login steps: a person signs in once in the live view; the program keeps it.
        "by_hand": m.as_ref().and_then(|m| m.session.as_ref()).is_some_and(|s| !s.signs_in()),
        "start_url": m.as_ref().and_then(|m| start_url(app, slug, c, m)),
    })
}

/// Where the live view opens: the first page the session's check visits, with the account's keys filled.
pub fn start_url(app: &App, slug: &str, c: &Account, m: &Manifest) -> Option<String> {
    let env = kiqr_core::accounts::env_for(app.store.home(), slug, Some(c), m, Default::default());
    let sess = m.session.as_ref()?;
    let template = match sess.check.iter().find(|s| s.args.as_ref().is_some_and(|a| a.get("url").is_some())) {
        Some(st) => st.args.as_ref()?["url"].as_str()?.to_string(),
        None if !sess.login_url.trim().is_empty() => sess.login_url.trim().to_string(),
        None => return None,
    };
    let u = kiqr_core::extensions::fill(&template, &json!({}), env);
    (!u.contains("${")).then_some(u)
}

async fn status_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let c = Account::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    Ok(Json(view(&app, &slug, &c).await))
}

/// `POST …/signin`: check and sign in now, in the background; the status says how it went.
async fn signin_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let c = Account::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    session_manifest(&app, &slug, &c).await.ok_or_else(|| bad(format!("{} has nothing that signs in", c.label)))?;
    if !can_sign_in(&app, &slug, &c).await {
        let st = crate::accounts::state(&app, &slug, &c).await;
        return Err(bad(crate::accounts::state_text(&c, &st)));
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
