//! Signing a credential in on a hosted program (a browser) and keeping it so.
//! The manifest's `session` says how: `check` steps tell whether the account
//! is signed in; the sign-in starts at `login_url`, where the model reads each
//! page's controls (any site, any language) and names the user, password and
//! code fields, and the runner types into those and nothing else (`guided`
//! below); or `login` steps type the credentials into fixed fields;
//! `blocked_when` texts mean a person is needed, and so does the model
//! saying the page is a captcha or a check. The login is a Vault credential;
//! its password and two-factor key are read only here (`Caller::Runner`), right before
//! typing; the model, transcripts, events and logs see names only, and what
//! was typed is struck from every program answer while it is in use. When
//! the steps are blocked (a captcha, a device check) or fail twice, the
//! credential needs a hand: a badge, a To do item, one Seemo line, and acting
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
use kiqr_core::conn::{Conn, SessionStatus};
use kiqr_core::vault::Caller;
use kiqr_core::events::EventKind;
use kiqr_core::extensions::Manifest;
use serde_json::{json, Value};

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/credentials/{id}/session", get(status_route))
        .route("/api/projects/{slug}/credentials/{id}/signin", post(signin_route))
        .route("/api/projects/{slug}/credentials/{id}/signout", post(signout_route))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

/// A login's values, for one sign-in. Dropped right after.
pub struct Login {
    pub username: String,
    pub password: String,
    pub otp: Option<String>,
}

impl Login {
    fn values(&self) -> Vec<String> {
        let mut v = vec![self.password.clone()];
        if let Some(o) = &self.otp {
            v.push(o.clone());
        }
        v.into_iter().filter(|s| s.len() >= 4).collect()
    }
}

/// The installed ability that signs a credential in, when one is on here.
pub async fn session_manifest(app: &App, slug: &str, c: &Conn) -> Option<Manifest> {
    crate::creds::session_manifest(app, slug, c.primary()).await
}

/// A credential as the runner drives it: with the need of the ability that signs it in.
pub async fn load(app: &App, slug: &str, id: &str) -> Result<Conn> {
    let c = app.vault.get(id)?.ok_or_else(|| anyhow!("there is no credential {id}"))?;
    let need = crate::creds::session_manifest(app, slug, &c).await.and_then(|m| m.needs().into_iter().find(|n| n.fits(&c, &|_| None))).unwrap_or_default();
    Ok(Conn { given: vec![(need, c)], site: None })
}

/// The login's values, read for the runner alone, with the one-time code computed from the saved key.
pub async fn credential(app: &App, _slug: &str, c: &Conn) -> Result<Login> {
    let id = c.id();
    let username = app.vault.field(id, "username", Caller::Runner)?.filter(|s| !s.trim().is_empty()).ok_or_else(|| anyhow!("{} has no username saved; the user adds it in the Vault", c.label()))?;
    let password = app.vault.field(id, "password", Caller::Runner)?.ok_or_else(|| anyhow!("{} has no password saved", c.label()))?;
    let otp = app.vault.field(id, "totp", Caller::Runner)?.filter(|s| !s.trim().is_empty()).map(|s| kiqr_core::totp::code(&s).ok_or_else(|| anyhow!("the two-factor key saved for {} is not valid", c.label()))).transpose()?;
    Ok(Login { username, password, otp })
}

/// Whether the credential has a login to sign in with.
pub fn has_login(_app: &App, _slug: &str, c: &Conn) -> bool {
    let p = c.primary();
    p.field("username").is_some_and(|f| !f.value.trim().is_empty()) && p.field("password").is_some_and(|f| !f.value.is_empty())
}

async fn set_state(app: &App, slug: &str, c: &Conn, state: &str, message: impl Into<String>) -> SessionStatus {
    let mut st = c.session(app.store.home());
    let now = chrono::Utc::now();
    st.state = state.to_string();
    st.message = message.into();
    st.checked_at = Some(now);
    // Signing in again is the user's wish (or the scheduler's, which never runs after a sign-out by the user).
    st.by_user = false;
    if state == SessionStatus::SIGNED_IN {
        st.signed_in_at = Some(now);
    }
    let _ = kiqr_core::conn::set_session(app.store.home(), c.id(), &st);
    app.bus.emit(Some(slug), EventKind::CredentialStateChanged { credential: c.id().to_string(), state: st.state.clone(), message: st.message.clone() });
    st
}

/// What a check saw.
struct Check {
    signed_in: bool,
    /// The page stayed empty (no title, no controls, no text) or the model could not tell, on every load: a
    /// single-page app that had not drawn yet, or a tab the browser lost (X's home). Says nothing either way.
    unread: bool,
    text: String,
    /// The model's words about the page, when it read it.
    said: Option<String>,
}

/// Loads of the check page before it counts as unreadable, and the pause before each new load.
const CHECK_LOADS: usize = 4;
const CHECK_PAUSE: std::time::Duration = std::time::Duration::from_secs(4);

/// Run the manifest's `check` steps: signed in when the last answer holds `signed_in_when`, signed out when it
/// holds one of `signed_out_when`; with no marker the model reads the page. A page with nothing on it, or one
/// the model cannot place, is loaded again a few seconds later (the steps start with a fresh load) rather than
/// read as signed out: that would start a sign-in on an account that is signed in.
async fn run_check(app: &App, slug: &str, c: &Conn, m: &Manifest) -> Result<Check> {
    let mut last_seen = None;
    for load in 1..=CHECK_LOADS {
        if load > 1 {
            tokio::time::sleep(CHECK_PAUSE).await;
        }
        let seen = check_once(app, slug, c, m).await?;
        if !seen.unread {
            return Ok(seen);
        }
        tracing::info!(credential = %c.id(), load, said = ?seen.said, "the check page had nothing to read yet");
        last_seen = Some(seen);
    }
    Ok(last_seen.expect("CHECK_LOADS > 0"))
}

async fn check_once(app: &App, slug: &str, c: &Conn, m: &Manifest) -> Result<Check> {
    let sess = m.session.as_ref().ok_or_else(|| anyhow!("{} has no session", m.name))?;
    let program = crate::ext::program_of(app, Some(slug), m).await?;
    let env = kiqr_core::conn::env_for(app.store.home(), slug, Some(c), m, &crate::creds::settings_of(app, slug, &m.id));
    let launch = crate::ext::mcp_env(app, slug, m, Some(c), &env);
    let (last, text) = crate::ext::mcp_steps(app, &program, &launch, m, &sess.check, &json!({}), &Default::default(), &env).await?;
    // What the program said, as plain text: a program that answers with JSON (a snapshot) often
    // puts it in a string, so every string in the answer counts, unescaped.
    let text = match text {
        Some(t) if !last.is_object() => t,
        _ => flat_text(&last),
    };
    tracing::debug!(credential = %c.id(), steps = sess.check.len(), text = %kiqr_core::profile::truncate_chars(&text, 400), "session check read");
    if blank(&last, &text) {
        return Ok(Check { signed_in: false, unread: true, text, said: None });
    }
    let needle = sess.signed_in_when.trim();
    let lower = text.to_lowercase();
    let markers = sess.signed_out_when.iter().filter(|t| !t.trim().is_empty()).count();
    if !needle.is_empty() {
        return Ok(Check { signed_in: text.contains(needle), unread: false, text, said: None });
    }
    if markers > 0 {
        let signed_in = !sess.signed_out_when.iter().any(|t| !t.trim().is_empty() && lower.contains(&t.trim().to_lowercase()));
        return Ok(Check { signed_in, unread: false, text, said: None });
    }
    // No marker to look for: the model reads the page.
    let read = guided::read_page(app, &guided::page_of(&last, &text), &[]).await?;
    let said = Some(read.reason.trim().to_string()).filter(|r| !r.is_empty());
    // A blocked page is read (a person is needed); "unknown" is a page the model could not place: load it again.
    Ok(Check { signed_in: read.state == guided::State::SignedIn, unread: read.state == guided::State::Unknown && blocked(sess, &text).is_none(), text, said })
}

/// A page with nothing to read: a snapshot with no title and no controls, or no text at all.
fn blank(last: &Value, text: &str) -> bool {
    match guided::snapshot_of(last) {
        Some(snap) => snap["title"].as_str().unwrap_or("").trim().is_empty() && snap["interactive_elements"].as_array().is_none_or(|a| a.is_empty()),
        None => text.trim().is_empty(),
    }
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
    let c = load(app, slug, id).await?;
    let m = session_manifest(app, slug, &c).await.ok_or_else(|| anyhow!("{} has nothing that signs it in", c.label()))?;
    let sess = m.session.clone().expect("session_manifest");
    let _turn = crate::ext::turn(c.id()).await;
    let was = c.session(app.store.home()).state;
    set_state(app, slug, &c, SessionStatus::CHECKING, "").await;
    let seen = match run_check(app, slug, &c, &m).await {
        Ok(x) => x,
        Err(e) => return Ok(set_state(app, slug, &c, SessionStatus::NEEDS_HAND, format!("could not check the sign-in: {e:#}")).await),
    };
    if seen.signed_in {
        return Ok(set_state(app, slug, &c, SessionStatus::SIGNED_IN, "").await);
    }
    if seen.unread {
        // Signing in now could undo a sign-in that holds; an account that was signed in stays so until a check can read the page.
        let state = if was == SessionStatus::SIGNED_IN { SessionStatus::SIGNED_IN } else { SessionStatus::NEEDS_HAND };
        return Ok(set_state(app, slug, &c, state, unread_message(&seen)).await);
    }
    if let Some(b) = blocked(&sess, &seen.text) {
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
        Ok(None) => match run_check(app, slug, &c, &m).await {
            Ok(Check { signed_in: true, .. }) => set_state(app, slug, &c, SessionStatus::SIGNED_IN, "").await,
            Ok(seen) if seen.unread => set_state(app, slug, &c, SessionStatus::NEEDS_HAND, unread_message(&seen)).await,
            // What the check saw, in the model's words when it read the page, so the message says why.
            Ok(Check { text: t, said, .. }) => set_state(app, slug, &c, SessionStatus::NEEDS_HAND, crate::redact(app, &blocked(&sess, &t).map(|b| format!("the site asks for a person: {b}")).unwrap_or_else(|| match said {
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

fn unread_message(seen: &Check) -> String {
    let why = seen.said.as_deref().map(|r| format!(" ({})", r.trim_end_matches('.'))).unwrap_or_default();
    format!("the check could not read the page after {CHECK_LOADS} loads{why}; look in the live view")
}

/// The person finished the sign-in in the live view and says so: check, and
/// never type the login (it could undo what they just did). A page the check
/// cannot read takes their word for it; one that shows signed out does not.
pub async fn signed_in_by_hand(app: &App, slug: &str, id: &str) -> Result<SessionStatus> {
    let c = load(app, slug, id).await?;
    let m = session_manifest(app, slug, &c).await.ok_or_else(|| anyhow!("{} has nothing that signs it in", c.label()))?;
    let sess = m.session.clone().expect("session_manifest");
    let _turn = crate::ext::turn(c.id()).await;
    set_state(app, slug, &c, SessionStatus::CHECKING, "checking the sign-in").await;
    Ok(match run_check(app, slug, &c, &m).await {
        Ok(Check { signed_in: true, .. }) => set_state(app, slug, &c, SessionStatus::SIGNED_IN, "").await,
        Ok(Check { unread: true, .. }) => set_state(app, slug, &c, SessionStatus::SIGNED_IN, "signed in by hand; the check could not read the page to confirm it").await,
        Ok(Check { text, said, .. }) => set_state(app, slug, &c, SessionStatus::NEEDS_HAND, blocked(&sess, &text).map(|b| format!("the site asks for a person: {b}")).unwrap_or_else(|| match said {
            Some(r) => format!("the check after your sign-in found: {r} Finish it in the live view."),
            None => "the check after your sign-in did not find you signed in; finish it in the live view".into(),
        })).await,
        Err(e) => set_state(app, slug, &c, SessionStatus::SIGNED_IN, format!("signed in by hand; the check could not run: {e:#}")).await,
    })
}

/// The `login` steps with the credentials in their context, one at a time so
/// a block is seen as soon as it shows. Returns what blocked, if anything.
async fn run_login(app: &App, slug: &str, c: &Conn, m: &Manifest, sess: &kiqr_core::extensions::Session, cred: &Login) -> Result<Option<String>> {
    let program = crate::ext::program_of(app, Some(slug), m).await?;
    let env = kiqr_core::conn::env_for(app.store.home(), slug, Some(c), m, &crate::creds::settings_of(app, slug, &m.id));
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
    /// Looks at a page that has nothing to read yet before anything is typed (an app that draws itself late), 2.5 s apart.
    const SETTLE_LOOKS: usize = 8;
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
    pub async fn run(app: &App, slug: &str, c: &Conn, m: &Manifest, sess: &kiqr_core::extensions::Session, cred: &Login) -> Result<Option<String>> {
        let program = crate::ext::program_of(app, Some(slug), m).await?;
        let env = kiqr_core::conn::env_for(app.store.home(), slug, Some(c), m, &crate::creds::settings_of(app, slug, &m.id));
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
        let mut waited = 0;
        let mut round = 0;
        while round < ROUNDS {
            if round > 0 || waited > 0 {
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
                    // Before anything was typed the page may still be loading: look again. After, it is where the sign-in ended up.
                    if typed.is_empty() && waited < SETTLE_LOOKS {
                        waited += 1;
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
            round += 1;
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
    let c = load(app, slug, id).await?;
    app.mcp.stop_credential(c.id()).await;
    let dir = c.data_dir(app.store.home());
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
    let _ = kiqr_core::conn::set_session(app.store.home(), c.id(), &st);
    Ok(st)
}

/// Keys and programs are there, so a sign-in can run (the session aside).
async fn can_sign_in(app: &App, slug: &str, c: &Conn) -> bool {
    !matches!(crate::creds::state(app, slug, c.primary()).await, kiqr_core::conn::State::MissingFields { .. } | kiqr_core::conn::State::NeedsProgram { .. })
}

/// The scheduler's tick: credentials used here with a session whose check is due.
pub async fn due(app: &App, slug: &str) {
    let used = kiqr_core::access::Access::load(&app.store, slug).unwrap_or_default().credentials();
    for id in used {
        let Ok(c) = load(app, slug, &id).await else { continue };
        let Some(m) = session_manifest(app, slug, &c).await else { continue };
        if !can_sign_in(app, slug, &c).await {
            continue;
        }
        let st = c.session(app.store.home());
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
        let key = format!("session:{}", c.id());
        if !app.jobs.write().await.insert(key.clone()) {
            continue;
        }
        let (app2, slug2, id, label) = (app.clone(), slug.to_string(), c.id().to_string(), c.label().to_string());
        tokio::spawn(async move {
            match signin(&app2, &slug2, &id).await {
                Ok(st) if st.needs_hand() => app2.bus.emit(Some(&slug2), EventKind::SeemoSays { text: format!("{label} needs a hand to sign in: {}. Open it in the Vault to finish.", st.message) }),
                Ok(_) => {}
                Err(e) => tracing::warn!(slug = %slug2, credential = %id, "session check failed: {e:#}"),
            }
            app2.jobs.write().await.remove(&key);
        });
    }
}

/// For the account view: how the sign-in stands, and whether this account signs in at all.
pub async fn view(app: &App, slug: &str, c: &Conn) -> Value {
    let m = session_manifest(app, slug, c).await;
    json!({
        "has_session": m.is_some(),
        "session": c.session(app.store.home()),
        "has_login": has_login(app, slug, c),
        "live_view": m.as_ref().and_then(|m| m.live_view.as_ref()).is_some(),
        // No login steps: a person signs in once in the live view; the program keeps it.
        "by_hand": m.as_ref().and_then(|m| m.session.as_ref()).is_some_and(|s| !s.signs_in()),
        "start_url": m.as_ref().and_then(|m| start_url(app, slug, c, m)),
    })
}

/// Where the live view opens: the first page the session's check visits, with the account's keys filled.
pub fn start_url(app: &App, slug: &str, c: &Conn, m: &Manifest) -> Option<String> {
    let env = kiqr_core::conn::env_for(app.store.home(), slug, Some(c), m, &crate::creds::settings_of(app, slug, &m.id));
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
    let c = load(&app, &slug, &id).await.map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    Ok(Json(view(&app, &slug, &c).await))
}

/// `POST …/signin`: check and sign in now, in the background; the status says how it went.
async fn signin_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let c = load(&app, &slug, &id).await.map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    session_manifest(&app, &slug, &c).await.ok_or_else(|| bad(format!("{} has nothing that signs it in", c.label())))?;
    if !can_sign_in(&app, &slug, &c).await {
        let st = crate::creds::state(&app, &slug, c.primary()).await;
        return Err(bad(crate::creds::state_text(c.label(), &st)));
    }
    let key = format!("session:{}", c.id());
    if !app.jobs.write().await.insert(key.clone()) {
        return Ok(Json(json!({ "started": false, "note": "a sign-in is running already", "session": c.session(app.store.home()) })));
    }
    let (app2, slug2, id2) = (app.clone(), slug.clone(), c.id().to_string());
    tokio::spawn(async move {
        if let Err(e) = signin(&app2, &slug2, &id2).await {
            tracing::warn!(slug = %slug2, credential = %id2, "sign-in failed: {e:#}");
        }
        app2.jobs.write().await.remove(&key);
    });
    Ok(Json(json!({ "started": true })))
}

async fn signout_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let st = signout(&app, &slug, &id).await.map_err(|e| bad(e.to_string()))?;
    Ok(Json(json!({ "session": st })))
}

#[cfg(test)]
mod check_tests {
    use super::*;

    #[test]
    fn an_empty_page_is_unread_not_signed_out() {
        // X's home before it has drawn, or after the tab crashed: no title, no controls.
        let empty = json!({ "result": "{\"title\": \"\", \"url\": \"https://x.com/home\", \"interactive_elements\": []}" });
        assert!(blank(&empty, &flat_text(&empty)));
        let drawn = json!({ "title": "Home / X", "url": "https://x.com/home", "interactive_elements": [{ "tag": "a", "selector": "a[href='/compose/post']" }] });
        assert!(!blank(&drawn, &flat_text(&drawn)));
        let sign_in = json!({ "title": "", "url": "https://x.com/i/flow/login", "interactive_elements": [{ "tag": "input", "selector": "input[name='text']" }] });
        assert!(!blank(&sign_in, &flat_text(&sign_in)));
        assert!(blank(&Value::Null, "  "));
        assert!(!blank(&Value::Null, "Welcome back"));
    }
}
