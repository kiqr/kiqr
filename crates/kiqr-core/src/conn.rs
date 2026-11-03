//! What a call goes through: the credentials a project granted an ability,
//! one per need it declares (`extensions::CredentialNeed`). The first is the
//! call's identity: its browser session (`vault/sessions/<id>/`), its turn
//! (one exclusive call at a time), its activity log and daily cap
//! (`vault/activity/<id>.jsonl`), all machine-wide, so two projects that use
//! one login share them. Values reach a call only as the names its manifest
//! maps them to, and never a field only the sign-in runner may read.

use crate::extensions::{CredentialNeed, Manifest};
use crate::paths::Home;
use crate::vault::Credential;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The address of the website a call is made for: a call in a website
/// channel gets the channel's (`https://myblog.com`), so an ability that
/// publishes a site (`hosts: ["${SITE_URL}"]`) never keeps one of its own.
pub const SITE_URL: &str = "SITE_URL";

/// The credentials a call goes through: each need that was given one, and the
/// website channel it is made in, when it is.
#[derive(Clone, Debug, Default)]
pub struct Conn {
    pub given: Vec<(CredentialNeed, Credential)>,
    pub site: Option<String>,
}

impl Conn {
    /// The credential the call is made as.
    pub fn primary(&self) -> &Credential {
        &self.given[0].1
    }
    pub fn id(&self) -> &str {
        &self.primary().id
    }
    pub fn label(&self) -> &str {
        &self.primary().label
    }

    /// The names a call gets (`FAL_KEY`) with their values, and the domains each may go to.
    pub fn env(&self) -> BTreeMap<String, (String, Vec<String>)> {
        let mut out = BTreeMap::new();
        for (need, c) in &self.given {
            for (name, field) in &need.env {
                if let Some(v) = c.call_value(field) {
                    out.insert(name.clone(), (v, c.domains.clone()));
                }
            }
        }
        out
    }

    /// The names whose values are secrets (anything but plain text, a URL or a number).
    pub fn secret_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (need, c) in &self.given {
            for (name, field) in &need.env {
                let visible = field.split('|').all(|f| c.field(f.trim()).is_some_and(|f| f.kind.visible()));
                if !visible {
                    out.push(name.clone());
                }
            }
        }
        out
    }

    pub fn data_dir(&self, home: &Home) -> PathBuf {
        home.credential_session_dir(self.id())
    }
    pub fn session(&self, home: &Home) -> SessionStatus {
        session_of(home, self.id())
    }
    pub fn activity(&self, home: &Home) -> Vec<Activity> {
        activity_of(home, self.id())
    }
    pub fn record(&self, home: &Home, a: &Activity) -> Result<()> {
        use std::io::Write;
        let f = home.credential_activity_file(self.id());
        if let Some(d) = f.parent() {
            crate::vault::private_dir(d);
        }
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(f)?;
        writeln!(file, "{}", serde_json::to_string(a)?)?;
        Ok(())
    }
    /// The earlier action with this role on this target, in any project.
    pub fn acted_on(&self, home: &Home, role: &str, target: &str) -> Option<Activity> {
        self.activity(home).into_iter().rev().find(|a| a.role == role && a.target == target)
    }
    /// Actions (anything but reading) since midnight UTC, in every project.
    pub fn actions_today(&self, home: &Home) -> usize {
        let today = Utc::now().date_naive();
        self.activity(home).iter().filter(|a| a.role != "read" && a.at.date_naive() == today).count()
    }
}

/// How a credential's sign-in on a hosted program stands (`status.json` in its session folder).
pub fn session_of(home: &Home, id: &str) -> SessionStatus {
    std::fs::read_to_string(home.credential_session_dir(id).join("status.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// A sign-in the daemon was in the middle of when it stopped never finishes:
/// at start, each one still "checking" says so, keeping a sign-in that held.
/// Returns how many there were.
pub fn interrupted_sessions(home: &Home) -> usize {
    let Some(root) = home.credential_session_dir("x").parent().map(|p| p.to_path_buf()) else { return 0 };
    let mut n = 0;
    for e in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let id = e.file_name().to_string_lossy().to_string();
        let mut st = session_of(home, &id);
        if st.state != "checking" {
            continue;
        }
        // Signed in before: it stays so until a check says otherwise.
        (st.state, st.message) = match st.signed_in_at {
            Some(_) => ("signed_in".into(), String::new()),
            None => ("needs_hand".into(), "the sign-in was cut off when KIQR stopped; sign in again".into()),
        };
        if set_session(home, &id, &st).is_ok() {
            n += 1;
        }
    }
    n
}

pub fn set_session(home: &Home, id: &str, st: &SessionStatus) -> Result<()> {
    let d = home.credential_session_dir(id);
    crate::vault::private_dir(&d);
    std::fs::write(d.join("status.json"), serde_json::to_string_pretty(st)?)?;
    Ok(())
}

/// Every action as a credential, oldest first.
pub fn activity_of(home: &Home, id: &str) -> Vec<Activity> {
    std::fs::read_to_string(home.credential_activity_file(id)).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// One action: what an ability did as a credential, in which project, on what.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Activity {
    pub at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub project: String,
    pub ability: String,
    pub role: String,
    pub tool: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    /// The channel it happened in, when it was in one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub channel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// How a sign-in on a hosted program stands.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionStatus {
    /// `unknown`, `checking`, `signed_in`, `signed_out`, `needs_hand`.
    pub state: String,
    pub message: String,
    pub checked_at: Option<DateTime<Utc>>,
    pub signed_in_at: Option<DateTime<Utc>>,
    /// The user signed it out: nothing signs it in again until they do.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub by_user: bool,
}

impl SessionStatus {
    pub const SIGNED_IN: &'static str = "signed_in";
    pub const SIGNED_OUT: &'static str = "signed_out";
    pub const NEEDS_HAND: &'static str = "needs_hand";
    pub const CHECKING: &'static str = "checking";
    pub fn needs_hand(&self) -> bool {
        self.state == Self::NEEDS_HAND
    }
}

/// Whether a credential works right now, in one word, for everything that asks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum State {
    /// Fields it needs are empty.
    MissingFields { fields: Vec<String> },
    /// A program its sign-in runs on is not installed.
    NeedsProgram { programs: Vec<String> },
    /// Never signed in, or signed out (by the user, or the site).
    SignedOut,
    /// Signing in needs a person (a captcha, a code).
    NeedsHand,
    /// Being signed in or checked now.
    Checking,
    /// A sign-in's token ran out and cannot be renewed.
    Expired,
    Ready,
}

impl State {
    pub fn ready(&self) -> bool {
        *self == State::Ready
    }
    pub fn word(&self) -> &'static str {
        match self {
            State::MissingFields { .. } => "missing_fields",
            State::NeedsProgram { .. } => "needs_program",
            State::SignedOut => "signed_out",
            State::NeedsHand => "needs_hand",
            State::Checking => "checking",
            State::Expired => "expired",
            State::Ready => "ready",
        }
    }
}

/// What the state is read from; the daemon gathers it.
#[derive(Default)]
pub struct Facts {
    pub missing_fields: Vec<String>,
    pub missing_programs: Vec<String>,
    /// It signs in on a hosted program, so its session counts.
    pub has_session: bool,
    pub session: SessionStatus,
    /// A sign-in whose token ran out with nothing to renew it.
    pub expired: bool,
}

impl Facts {
    /// The one rule: fields, then programs, then the sign-in.
    pub fn state(self) -> State {
        if !self.missing_fields.is_empty() {
            return State::MissingFields { fields: self.missing_fields };
        }
        if !self.missing_programs.is_empty() {
            return State::NeedsProgram { programs: self.missing_programs };
        }
        if self.expired {
            return State::Expired;
        }
        if self.has_session {
            return match self.session.state.as_str() {
                SessionStatus::SIGNED_IN => State::Ready,
                SessionStatus::NEEDS_HAND => State::NeedsHand,
                SessionStatus::CHECKING => State::Checking,
                _ => State::SignedOut,
            };
        }
        State::Ready
    }
}

/// A lookup for templates (`${NAME}`) in a call: names its credentials give
/// come from them; the project's settings by their name in capitals
/// (`site_url` → `${SITE_URL}`); KIQR's own keys from its `.env`. Nothing
/// else: a name the manifest maps to a credential is never read from `.env`.
pub fn env_for(home: &Home, slug: &str, conn: Option<&Conn>, m: &Manifest, settings: &BTreeMap<String, String>) -> impl Fn(&str) -> Option<String> + Clone {
    let own = m.credential_env();
    let given: BTreeMap<String, String> = conn.map(|c| c.env().into_iter().map(|(k, (v, _))| (k, v)).collect()).unwrap_or_default();
    let site = conn.and_then(|c| c.site.clone());
    let set: BTreeMap<String, String> = settings.iter().map(|(k, v)| (k.to_ascii_uppercase(), v.clone())).collect();
    let rest = crate::secrets::env_fn(home, Some(slug));
    move |k: &str| {
        if k == SITE_URL {
            if let Some(s) = &site {
                return Some(s.clone());
            }
        }
        if let Some(v) = given.get(k) {
            return Some(v.clone());
        }
        if own.iter().any(|n| n == k) {
            return None;
        }
        if let Some(v) = set.get(k).filter(|v| !v.is_empty()) {
            return Some(v.clone());
        }
        if crate::provider::is_core_key(k) {
            rest(k)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::{Field, FieldKind};

    fn cred(template: &str, fields: &[(&str, &str, bool)]) -> Credential {
        Credential {
            id: "cred_1".into(),
            label: "One".into(),
            template: template.into(),
            domains: vec!["x.com".into()],
            fields: fields.iter().map(|(n, v, ro)| Field { name: n.to_string(), label: n.to_string(), kind: if *n == "username" { FieldKind::Text } else { FieldKind::Secret }, value: v.to_string(), runner_only: *ro, optional: false }).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_call_gets_only_mapped_fields_and_never_a_password() {
        let m = crate::extensions::parse_manifest("id: t\nname: T\ntransport: http\nhosts: [api.x.com]\ncredentials:\n  - template: [oauth, api_key]\n    env: { X_TOKEN: \"token|key\" }\n").unwrap();
        let need = m.needs()[0].clone();
        let mut c = Conn { given: vec![(need, cred("api_key", &[("key", "k1", false)]))], site: None };
        let home = Home::at(std::env::temp_dir().join("kiqr-conn-test"));
        let env = env_for(&home, "p", Some(&c), &m, &BTreeMap::from([("site_url".to_string(), "https://a.b".to_string())]));
        assert_eq!(env("X_TOKEN").as_deref(), Some("k1"));
        assert_eq!(env("SITE_URL").as_deref(), Some("https://a.b"));
        // In a website channel, the channel's address wins.
        c.site = Some("https://blog.example".into());
        let env = env_for(&home, "p", Some(&c), &m, &BTreeMap::from([("site_url".to_string(), "https://a.b".to_string())]));
        assert_eq!(env("SITE_URL").as_deref(), Some("https://blog.example"));
        assert_eq!(env("HOME"), None, "nothing from the machine but KIQR's own keys");
        // A login's password never reaches a call, whatever a credential claims.
        let login = cred("login", &[("username", "u", false), ("password", "p", true)]);
        assert_eq!(login.call_value("password"), None);
        assert_eq!(login.call_value("username").as_deref(), Some("u"));
        assert!(crate::extensions::parse_manifest("id: t\nname: T\ntransport: http\ncredentials:\n  - template: login\n    env: { PW: password }\n").unwrap_err().to_string().contains("only the sign-in"));
    }
}
