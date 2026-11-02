//! Accounts: who Seemo signs in as. One named account of a service inside a
//! project, used by every installed ability of that service (its manifest's
//! `service`, else its id): one sign-in for all of them. An account knows its
//! service, its name, where its secrets are kept and what Seemo may do through
//! it (`allow`, one per role: on its own, ask first, off). It never knows where
//! content goes: that is the Audience (`crate::audience`), and a task or a call
//! names the spot itself.
//!
//! `projects/<slug>/accounts/<id>/account.yaml`, committed; beside it `.env`
//! (keys kept in KIQR, and tokens a sign-in got), `data/` (a hosted program's
//! state, the sign-in status) and `activity.jsonl`, never committed.

use crate::extensions::Manifest;
use crate::paths::Home;
use crate::secrets;
use crate::Store;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// How far Seemo may go with one role on an account: on its own (the card
/// is the record), after a yes (the card waits), or not at all. Roles that
/// never act (`read`, `learn`, `measure`) know only `auto` (on) and `off`.
/// Acting on its own applies only to spots in the Audience; anywhere else asks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Allow {
    Auto,
    #[default]
    Ask,
    Off,
}

impl<'de> Deserialize<'de> for Allow {
    /// `auto`, `on`, `ask`, `off`; and the booleans YAML readers make of `on` and `off`.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Flag(bool),
            Word(String),
        }
        match Raw::deserialize(d)? {
            Raw::Flag(true) => Ok(Allow::Auto),
            Raw::Flag(false) => Ok(Allow::Off),
            Raw::Word(w) => Allow::parse(&w).ok_or_else(|| serde::de::Error::custom(format!("'{w}' is not auto, ask or off"))),
        }
    }
}

/// Everything an account can be allowed: the tool roles plus learning the
/// business from it and measuring (reading its numbers).
pub const PERMISSIONS: &[&str] = &["read", "reply", "publish", "manage", "learn", "measure"];
/// The roles that change something out there, and so ask by default.
pub const ACTING: &[&str] = &["reply", "publish", "manage"];

impl Allow {
    pub fn acts(role: &str) -> bool {
        ACTING.contains(&role)
    }
    /// What an account allows when nothing was chosen: acting asks, the rest is on.
    pub fn default_for(role: &str) -> Allow {
        if Allow::acts(role) { Allow::Ask } else { Allow::Auto }
    }
    pub fn parse(s: &str) -> Option<Allow> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "on" => Some(Allow::Auto),
            "ask" => Some(Allow::Ask),
            "off" => Some(Allow::Off),
            _ => None,
        }
    }
}

/// Where an account's secrets are kept: `kiqr` (its own `.env`) or the id of
/// a password manager account in the project, with `item` naming the entry
/// there (`op://Work/Reddit`). A login (`LOGIN_*`) and typed keys are read
/// from the store at call time; a token KIQR got from a sign-in is always in
/// the `.env`. Neither field is a secret, so both are committed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Secrets {
    pub store: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub item: String,
}

impl Default for Secrets {
    fn default() -> Self {
        Secrets { store: Self::KIQR.into(), item: String::new() }
    }
}

impl Secrets {
    pub const KIQR: &'static str = "kiqr";
    pub const USERNAME: &'static str = "LOGIN_USERNAME";
    pub const PASSWORD: &'static str = "LOGIN_PASSWORD";
    pub const TOTP: &'static str = "LOGIN_TOTP_SECRET";
    /// The key names of a login: never shown, never in a template, read by the sign-in runner alone.
    pub const LOGIN: &'static [&'static str] = &[Self::USERNAME, Self::PASSWORD, Self::TOTP];
    pub fn in_kiqr(&self) -> bool {
        self.store.is_empty() || self.store == Self::KIQR
    }
    fn is_default(&self) -> bool {
        self.in_kiqr() && self.item.is_empty()
    }
}

/// How an account's sign-in on a hosted program stands: kept in the
/// account's `data/status.json` (never committed).
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

/// Whether an account works right now, in one word, for everything that asks:
/// the routes, the abilities list, the executor, the CLI and the web UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum State {
    /// Keys its abilities need are not there.
    MissingKeys { keys: Vec<String> },
    /// A program it runs on is not installed.
    NeedsProgram { programs: Vec<String> },
    /// Never signed in, or signed out (by the user, or the site).
    SignedOut,
    /// Signing in needs a person (a captcha, a code).
    NeedsHand,
    /// Being signed in or checked now.
    Checking,
    Ready,
}

impl State {
    pub fn ready(&self) -> bool {
        *self == State::Ready
    }
    pub fn word(&self) -> &'static str {
        match self {
            State::MissingKeys { .. } => "missing_keys",
            State::NeedsProgram { .. } => "needs_program",
            State::SignedOut => "signed_out",
            State::NeedsHand => "needs_hand",
            State::Checking => "checking",
            State::Ready => "ready",
        }
    }
}

/// What the state is read from; the daemon gathers it (`accounts::state`).
#[derive(Default)]
pub struct Facts {
    pub missing_keys: Vec<String>,
    pub missing_programs: Vec<String>,
    /// The service signs in on a hosted program, so its session counts.
    pub has_session: bool,
    pub session: SessionStatus,
}

impl Facts {
    /// The one rule: keys, then programs, then the sign-in.
    pub fn state(self) -> State {
        if !self.missing_keys.is_empty() {
            return State::MissingKeys { keys: self.missing_keys };
        }
        if !self.missing_programs.is_empty() {
            return State::NeedsProgram { programs: self.missing_programs };
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Account {
    pub id: String,
    /// What the user calls it (`u/acme_dev`); the service's name when nothing else is known.
    pub label: String,
    /// The service it is an account of; every ability of that service uses it.
    pub service: String,
    /// The one a call uses when nothing names an account; one per service.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
    /// `user`, or `chat:<id>`.
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    /// What Seemo may do here, by role (`PERMISSIONS`); a role not listed has
    /// its default (`Allow::default_for`). The manifest's `allow` fills it
    /// when the account is made; the user changes it on the account.
    pub allow: BTreeMap<String, Allow>,
    /// Where its secrets are kept.
    #[serde(skip_serializing_if = "Secrets::is_default")]
    pub secrets: Secrets,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_learned: Option<DateTime<Utc>>,
    /// Documents read the last time.
    #[serde(skip_serializing_if = "is_zero")]
    pub learned: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl Default for Account {
    fn default() -> Self {
        Self { id: String::new(), label: String::new(), service: String::new(), default: false, created_by: "user".into(), created_at: Utc::now(), allow: BTreeMap::new(), secrets: Secrets::default(), last_learned: None, learned: 0 }
    }
}

impl Account {
    /// What Seemo may do here with one role: the choice made, else the default.
    pub fn allow(&self, role: &str) -> Allow {
        self.allow.get(role).copied().unwrap_or_else(|| Allow::default_for(role))
    }

    /// Learning from it is on (the account is a source).
    pub fn learns(&self) -> bool {
        self.allow("learn") != Allow::Off
    }

    /// Every account of a project, by label.
    pub fn list(store: &Store, slug: &str) -> Result<Vec<Account>> {
        let dir = store.home().accounts_dir(slug);
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                if let Ok(Some(c)) = store.read_yaml_opt::<Account>(&e.path().join("account.yaml")) {
                    out.push(c);
                }
            }
        }
        out.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    /// One service's accounts, the default first.
    pub fn of(store: &Store, slug: &str, service: &str) -> Vec<Account> {
        let mut v: Vec<Account> = Self::list(store, slug).unwrap_or_default().into_iter().filter(|c| c.service == service).collect();
        v.sort_by_key(|c| !c.default);
        v
    }

    pub fn load(store: &Store, slug: &str, id: &str) -> Result<Account> {
        if !valid_id(id) {
            bail!("no account '{id}'");
        }
        match store.read_yaml_opt::<Account>(&store.home().accounts_dir(slug).join(id).join("account.yaml"))? {
            Some(c) => Ok(c),
            None => bail!("no account '{id}'"),
        }
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        if !valid_id(&self.id) {
            bail!("an account id is letters, digits and dashes");
        }
        if self.service.is_empty() {
            bail!("an account belongs to a service");
        }
        store.write_yaml(&store.home().accounts_dir(slug).join(&self.id).join("account.yaml"), self)
    }

    /// Remove it, keys included. The next one of the same service becomes the default.
    pub fn delete(store: &Store, slug: &str, id: &str) -> Result<Account> {
        let c = Self::load(store, slug, id)?;
        std::fs::remove_dir_all(store.home().accounts_dir(slug).join(id))?;
        if c.default {
            if let Some(mut next) = Self::of(store, slug, &c.service).into_iter().next() {
                next.default = true;
                next.save(store, slug)?;
            }
        }
        Ok(c)
    }

    /// Make this one the service's default, and no other.
    pub fn make_default(store: &Store, slug: &str, id: &str) -> Result<()> {
        let c = Self::load(store, slug, id)?;
        for mut o in Self::of(store, slug, &c.service) {
            let want = o.id == c.id;
            if o.default != want {
                o.default = want;
                o.save(store, slug)?;
            }
        }
        Ok(())
    }

    /// The default of a service, else its first.
    pub fn default_of(store: &Store, slug: &str, service: &str) -> Option<Account> {
        Self::of(store, slug, service).into_iter().next()
    }

    /// A service's account by id or label (any case).
    pub fn find(store: &Store, slug: &str, service: &str, name: &str) -> Option<Account> {
        let n = name.trim();
        Self::of(store, slug, service).into_iter().find(|c| c.id == n || c.label.eq_ignore_ascii_case(n))
    }

    pub fn env_file(&self, home: &Home, slug: &str) -> PathBuf {
        env_file(home, slug, &self.id)
    }

    /// The keys kept in KIQR, by name. Values never leave the daemon.
    pub fn keys(&self, home: &Home, slug: &str) -> BTreeMap<String, String> {
        secrets::read_env_file(&self.env_file(home, slug))
    }

    /// Keys an ability needs that this account does not have. Keys kept in a
    /// password manager count as there: the manager answers at call time.
    pub fn missing(&self, home: &Home, slug: &str, m: &Manifest) -> Vec<String> {
        let keys = self.keys(home, slug);
        let elsewhere = !self.secrets.in_kiqr();
        let written: Vec<String> = m.connect.iter().flat_map(|c| c.writes()).collect();
        crate::extensions::missing_keys(m, |k| keys.contains_key(k) || (elsewhere && !written.iter().any(|w| w == k)))
    }

    /// A folder of the account's own that outlives a call: where a hosted
    /// program keeps its state (a browser profile), shared by every ability
    /// of the service and removed with the account. Never committed.
    pub fn data_dir(&self, home: &Home, slug: &str) -> PathBuf {
        home.accounts_dir(slug).join(&self.id).join("data")
    }

    /// How the sign-in stands (`data/status.json`); unknown when never checked.
    pub fn session(&self, home: &Home, slug: &str) -> SessionStatus {
        std::fs::read_to_string(self.data_dir(home, slug).join("status.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn set_session(&self, home: &Home, slug: &str, st: &SessionStatus) -> Result<()> {
        let f = self.data_dir(home, slug).join("status.json");
        if let Some(d) = f.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(f, serde_json::to_string_pretty(st)?)?;
        Ok(())
    }

    fn activity_file(&self, home: &Home, slug: &str) -> PathBuf {
        home.accounts_dir(slug).join(&self.id).join("activity.jsonl")
    }

    /// Every action through it, oldest first.
    pub fn activity(&self, home: &Home, slug: &str) -> Vec<Activity> {
        std::fs::read_to_string(self.activity_file(home, slug)).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
    }

    pub fn record(&self, home: &Home, slug: &str, a: &Activity) -> Result<()> {
        use std::io::Write;
        let f = self.activity_file(home, slug);
        if let Some(d) = f.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(f)?;
        writeln!(file, "{}", serde_json::to_string(a)?)?;
        Ok(())
    }

    /// The earlier action with this role on this target, if any.
    pub fn acted_on(&self, home: &Home, slug: &str, role: &str, target: &str) -> Option<Activity> {
        self.activity(home, slug).into_iter().rev().find(|a| a.role == role && a.target == target)
    }

    /// Actions (anything but reading) since midnight UTC.
    pub fn actions_today(&self, home: &Home, slug: &str) -> usize {
        let today = Utc::now().date_naive();
        self.activity(home, slug).iter().filter(|a| a.role != "read" && a.at.date_naive() == today).count()
    }
}

/// One action through an account: what an ability did, on what, when.
/// `accounts/<id>/activity.jsonl`, one line each, never committed.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Activity {
    pub at: DateTime<Utc>,
    pub ability: String,
    pub role: String,
    pub tool: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    /// The Audience entry it went to, when it went to one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub audience: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

pub fn env_file(home: &Home, slug: &str, id: &str) -> PathBuf {
    home.accounts_dir(slug).join(id).join(".env")
}

/// Every key name an account of this ability holds: what it asks for, what its
/// sign-in writes, what its program or commands are given.
pub fn key_names(m: &Manifest) -> Vec<String> {
    let mut v: Vec<String> = m.auth.iter().map(|a| a.env.clone()).collect();
    for c in &m.connect {
        v.extend(c.writes());
        if !c.key_file.is_empty() {
            v.push(c.key_file.clone());
        }
    }
    v.extend(m.env.iter().cloned());
    for t in &m.tools {
        if let Some(c) = &t.provide.command {
            v.extend(c.env.iter().cloned());
        }
    }
    for p in m.provides.values() {
        if let Some(c) = &p.command {
            v.extend(c.env.iter().cloned());
        }
    }
    v.retain(|n| !n.is_empty() && !crate::provider::is_core_key(n));
    v.sort();
    v.dedup();
    v
}

/// A lookup for templates (`${NAME}`) in a call through one account: the
/// ability's own key names come from the account (its `.env`, else what its
/// password manager answered, `vault`), never the project's or the machine's
/// `.env`; anything else (the core keys) as before. A login is never in it.
pub fn env_for(home: &Home, slug: &str, acct: Option<&Account>, m: &Manifest, vault: BTreeMap<String, String>) -> impl Fn(&str) -> Option<String> + Clone {
    let own = key_names(m);
    let keys = acct.map(|c| c.keys(home, slug)).unwrap_or_default();
    let rest = secrets::env_fn(home, Some(slug));
    move |k: &str| {
        if Secrets::LOGIN.contains(&k) {
            None
        } else if own.iter().any(|n| n == k) {
            keys.get(k).filter(|v| !v.is_empty()).or_else(|| vault.get(k)).cloned()
        } else {
            rest(k)
        }
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') && !id.starts_with('-')
}

/// A unique id from a label: `u-acme-dev`, `u-acme-dev-2`.
pub fn new_id(store: &Store, slug: &str, label: &str) -> String {
    let base = crate::assets::slugify(label);
    let base = if base.is_empty() { "account".to_string() } else { base };
    let dir = store.home().accounts_dir(slug);
    let mut id = base.clone();
    let mut n = 2;
    while dir.join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> (Store, Home) {
        let dir = std::env::temp_dir().join(format!("kiqr-acct-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(Home::at(&dir)).unwrap();
        let home = store.home().clone();
        (store, home)
    }

    #[test]
    fn accounts_are_identities() {
        let (store, home) = store("ident");
        let a = Account { id: "u-acme".into(), label: "u/acme".into(), service: "reddit".into(), default: true, ..Default::default() };
        a.save(&store, "acme").unwrap();
        let b = Account { id: "u-other".into(), label: "u/other".into(), service: "reddit".into(), ..Default::default() };
        b.save(&store, "acme").unwrap();
        let text = std::fs::read_to_string(home.accounts_dir("acme").join("u-acme/account.yaml")).unwrap();
        assert!(!text.contains("secrets") && !text.contains("place") && !text.contains("kind"), "{text}");
        assert_eq!(Account::of(&store, "acme", "reddit")[0].id, "u-acme");
        assert_eq!(Account::find(&store, "acme", "reddit", "U/OTHER").map(|c| c.id), Some("u-other".into()));
        Account::make_default(&store, "acme", "u-other").unwrap();
        assert_eq!(Account::default_of(&store, "acme", "reddit").map(|c| c.id), Some("u-other".into()));
        Account::delete(&store, "acme", "u-other").unwrap();
        assert!(Account::load(&store, "acme", "u-acme").unwrap().default, "the next one becomes the default");
        assert!(Account { id: "x".into(), ..Default::default() }.save(&store, "acme").is_err(), "an account belongs to a service");

        // Permissions: acting asks by default, the rest is on.
        let mut acct = Account::default();
        assert_eq!((acct.allow("publish"), acct.allow("reply"), acct.allow("read"), acct.allow("learn")), (Allow::Ask, Allow::Ask, Allow::Auto, Allow::Auto));
        acct.allow.insert("learn".into(), Allow::Off);
        assert!(!acct.learns());
        assert_eq!((Allow::parse("on"), Allow::parse("ASK"), Allow::parse("nope")), (Some(Allow::Auto), Some(Allow::Ask), None));

        // The log: what was done on what, and how much today.
        let me = Account::load(&store, "acme", "u-acme").unwrap();
        let act = |role: &str, target: &str, at: DateTime<Utc>| Activity { at, ability: "reddit".into(), role: role.into(), tool: "reply".into(), target: target.into(), audience: String::new(), url: None };
        me.record(&home, "acme", &act("reply", "post-1", Utc::now())).unwrap();
        me.record(&home, "acme", &act("reply", "post-2", Utc::now() - chrono::Duration::days(2))).unwrap();
        me.record(&home, "acme", &act("read", "", Utc::now())).unwrap();
        assert!(me.acted_on(&home, "acme", "reply", "post-2").is_some() && me.acted_on(&home, "acme", "manage", "post-1").is_none());
        assert_eq!(me.actions_today(&home, "acme"), 1, "reading and older days do not count");
        assert!(me.data_dir(&home, "acme").ends_with("accounts/u-acme/data"));
    }

    #[test]
    fn one_rule_for_whether_an_account_works() {
        let signed = |s: &str| SessionStatus { state: s.into(), ..Default::default() };
        assert_eq!(Facts { missing_keys: vec!["K".into()], has_session: true, ..Default::default() }.state(), State::MissingKeys { keys: vec!["K".into()] });
        assert_eq!(Facts { missing_programs: vec!["uv".into()], ..Default::default() }.state(), State::NeedsProgram { programs: vec!["uv".into()] });
        assert_eq!(Facts { has_session: true, ..Default::default() }.state(), State::SignedOut, "never signed in");
        assert_eq!(Facts { has_session: true, session: signed("needs_hand"), ..Default::default() }.state(), State::NeedsHand);
        assert!(Facts { has_session: true, session: signed("signed_in"), ..Default::default() }.state().ready());
        assert!(Facts::default().state().ready(), "keys and nothing to sign in on");
    }

    #[test]
    fn keys_come_from_the_account_or_its_password_manager() {
        let (_store, home) = store("env");
        let m = crate::extensions::parse_manifest("id: blog\nname: Blog\ntransport: http\nhosts: [b.test]\nauth:\n  - { env: B_KEY }\n").unwrap();
        let mut a = Account { id: "blog".into(), label: "Blog".into(), service: "blog".into(), ..Default::default() };
        assert_eq!(a.missing(&home, "acme", &m), vec!["B_KEY".to_string()]);
        a.secrets = Secrets { store: "vault".into(), item: "op://Work/Blog".into() };
        assert!(a.missing(&home, "acme", &m).is_empty(), "the manager answers at call time");
        let vault: BTreeMap<String, String> = [("B_KEY".to_string(), "v".to_string()), (Secrets::PASSWORD.to_string(), "p".to_string())].into_iter().collect();
        let env = env_for(&home, "acme", Some(&a), &m, vault);
        assert_eq!(env("B_KEY").as_deref(), Some("v"));
        assert!(env(Secrets::PASSWORD).is_none(), "a login is never in a template");
    }
}
