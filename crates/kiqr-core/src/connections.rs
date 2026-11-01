//! Connections: one named account of a service inside a project, with its
//! own keys. Every installed ability of that service (its manifest's
//! `service`, else its id) uses the project's accounts of it: one sign-in for
//! all of them. A project can have as many as it wants (two WordPress
//! sites, two analytics properties). The user sees an **account**; what Seemo
//! may do through it is its permissions (`allow`, one per role: on its own,
//! ask first, off), and where its service has several places to publish (a
//! subreddit, a repository branch, a mailing list) it has **places**. An
//! account of a service that publishes has a `kind` and a count of what went
//! out; the code still calls that a channel, the user never does. Installing
//! an ability only puts its code on the machine; everything that says *which*
//! account lives here.
//!
//! `projects/<slug>/connections/<id>/connection.yaml`, committed, and
//! `projects/<slug>/connections/<id>/.env`, the keys, never committed.

use crate::extensions::{ChannelKind, Manifest};
use crate::paths::Home;
use crate::secrets;
use crate::Store;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// How far Seemo may go with one role on an account: on its own (the card
/// is the record), after a yes (the card waits), or not at all. Roles that
/// never act (`read`, `learn`, `measure`) know only `auto` (on) and `off`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Allow {
    #[serde(alias = "on")]
    Auto,
    #[default]
    Ask,
    Off,
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

/// Somewhere inside an account that content can go: a subreddit, a branch,
/// a mailing list. Its `fields` are what the kind's `place` declares; a
/// mailing list's one field is `list`, the list's id.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Place {
    pub id: String,
    /// The channel kind it belongs to.
    pub kind: String,
    pub label: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
    /// The one a publish goes to when none is named.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
}

impl Place {
    /// `list`, a mailing list on a mail account.
    pub const LIST: &'static str = "list";
    pub fn is_list(&self) -> bool {
        self.kind == Self::LIST
    }
    pub fn list_id(&self) -> Option<&str> {
        self.is_list().then(|| self.fields.get("list").and_then(Value::as_str)).flatten()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Connection {
    pub id: String,
    pub label: String,
    /// The installed ability this was made with.
    #[serde(alias = "extension")]
    pub ability: String,
    /// The service it is an account of; every ability of that service uses it.
    /// Empty in older homes: the ability's id.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub service: String,
    /// The one a call uses when nothing names a connection; one per service.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
    /// `user`, or `chat:<id>`.
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    /// The kind its service publishes as (`email`, `wordpress`…); empty for an
    /// account of a service that does not publish. The code's "channel".
    #[serde(skip_serializing_if = "String::is_empty")]
    pub kind: String,
    /// What Seemo may do here, by role (`PERMISSIONS`); a role not listed has
    /// its default (`Allow::default_for`). The manifest's `allow` fills it
    /// when the account is made; the user changes it on the account.
    pub allow: BTreeMap<String, Allow>,
    /// Where inside the account content goes, when the kind has places.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub places: Vec<Place>,
    /// The ability that publishes here, when the user picked one of several
    /// (`pick_publisher`). Empty: the one it was made with, else the first on.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub publisher: String,
    pub publications: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_published: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_learned: Option<DateTime<Utc>>,
    /// Documents read the last time.
    #[serde(skip_serializing_if = "is_zero")]
    pub learned: u32,
    /// Records from before permissions and places, read on load and folded
    /// into `allow` and `places` by `normalize`; never written again.
    #[doc(hidden)]
    #[serde(skip_serializing)]
    pub destination: BTreeMap<String, Value>,
    #[doc(hidden)]
    #[serde(skip_serializing)]
    pub asks_first: Option<bool>,
    #[doc(hidden)]
    #[serde(skip_serializing)]
    pub learns: Option<bool>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            ability: String::new(),
            service: String::new(),
            default: false,
            created_by: "user".into(),
            created_at: Utc::now(),
            kind: String::new(),
            allow: BTreeMap::new(),
            places: Vec::new(),
            publisher: String::new(),
            publications: 0,
            last_published: None,
            last_learned: None,
            learned: 0,
            destination: BTreeMap::new(),
            asks_first: None,
            learns: None,
        }
    }
}

impl Connection {
    /// An account of a service that publishes (it has a kind).
    pub fn is_channel(&self) -> bool {
        !self.kind.is_empty()
    }

    /// May be published to: it has a kind and publishing is not switched off.
    pub fn publishes(&self) -> bool {
        self.is_channel() && self.allow("publish") != Allow::Off
    }

    /// What Seemo may do here with one role: the choice made, else the default.
    pub fn allow(&self, role: &str) -> Allow {
        self.allow.get(role).copied().unwrap_or_else(|| Allow::default_for(role))
    }

    /// Learning from it is on (the account is a source).
    pub fn learns(&self) -> bool {
        self.allow("learn") != Allow::Off
    }

    /// A record from before permissions and places, read into the new shape:
    /// "asks first" off is publishing on its own, the Source switch is the
    /// learn permission, a destination is the account's one place. Saving
    /// writes the new shape; nothing is lost.
    fn normalize(&mut self) {
        if self.asks_first.take() == Some(false) {
            self.allow.entry("publish".into()).or_insert(Allow::Auto);
        }
        match self.learns.take() {
            Some(true) => {
                self.allow.entry("learn".into()).or_insert(Allow::Auto);
            }
            Some(false) => {
                self.allow.entry("learn".into()).or_insert(Allow::Off);
            }
            None => {}
        }
        if !self.destination.is_empty() && self.places.is_empty() {
            let fields = std::mem::take(&mut self.destination);
            // An old email channel's `to` names the place; anything else, its first text.
            let label = fields.get("to").and_then(|v| v.as_str().map(str::to_string)).or_else(|| fields.values().find_map(|v| v.as_str().map(str::to_string))).unwrap_or_else(|| self.label.clone());
            self.places.push(Place { id: "main".into(), kind: self.kind.clone(), label, fields, default: true });
        }
        self.destination.clear();
    }

    /// The place a call means: the one named (by id or label), else the
    /// default, else the only one. None when the account has none, or the
    /// name is unknown.
    pub fn place(&self, named: Option<&str>) -> Option<&Place> {
        match named.map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => self.places.iter().find(|p| p.id == n || p.label.eq_ignore_ascii_case(n)),
            None => self.places.iter().find(|p| p.default).or_else(|| (self.places.len() == 1).then(|| &self.places[0])),
        }
    }

    /// The place of one kind a call means: the one named when it is of that
    /// kind, else that kind's default, else its only one.
    pub fn place_of_kind(&self, kind: &str, named: Option<&str>) -> Option<&Place> {
        let of_kind: Vec<&Place> = self.places.iter().filter(|p| p.kind == kind).collect();
        match named.map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => of_kind.into_iter().find(|p| p.id == n || p.label.eq_ignore_ascii_case(n)),
            None => of_kind.iter().copied().find(|p| p.default).or_else(|| (of_kind.len() == 1).then(|| of_kind[0])),
        }
    }

    /// Add or replace a place; the first one of its kind is that kind's default.
    pub fn put_place(&mut self, mut p: Place) {
        if p.id.is_empty() {
            p.id = crate::assets::slugify(&p.label);
        }
        if !self.places.iter().any(|o| o.kind == p.kind && o.id != p.id) {
            p.default = true;
        }
        let kind = p.kind.clone();
        if let Some(i) = self.places.iter().position(|o| o.id == p.id) {
            p.default = p.default || self.places[i].default;
            self.places[i] = p;
        } else {
            self.places.push(p);
        }
        // One default per kind: the last one made default wins.
        if self.places.iter().filter(|p| p.kind == kind && p.default).count() > 1 {
            let keep = self.places.iter().rposition(|p| p.kind == kind && p.default).unwrap_or(0);
            for (i, o) in self.places.iter_mut().enumerate() {
                if o.kind == kind {
                    o.default = i == keep;
                }
            }
        }
    }

    pub fn remove_place(&mut self, id: &str) -> Option<Place> {
        let i = self.places.iter().position(|p| p.id == id)?;
        let gone = self.places.remove(i);
        if gone.default {
            if let Some(first) = self.places.iter_mut().find(|p| p.kind == gone.kind) {
                first.default = true;
            }
        }
        Some(gone)
    }

    /// The service it is an account of: `service`, else the ability it was made with.
    pub fn service(&self) -> &str {
        if self.service.is_empty() { &self.ability } else { &self.service }
    }

    /// Every connection of a project, by label.
    pub fn list(store: &Store, slug: &str) -> Result<Vec<Connection>> {
        let dir = store.home().connections_dir(slug);
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let f = e.path().join("connection.yaml");
                if let Ok(Some(mut c)) = store.read_yaml_opt::<Connection>(&f) {
                    c.normalize();
                    out.push(c);
                }
            }
        }
        out.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    /// The project's accounts that can be published to.
    pub fn channels(store: &Store, slug: &str) -> Result<Vec<Connection>> {
        Ok(Self::list(store, slug)?.into_iter().filter(Connection::publishes).collect())
    }

    /// One service's connections (`Manifest::service`), the default first.
    pub fn of(store: &Store, slug: &str, service: &str) -> Vec<Connection> {
        let mut v: Vec<Connection> = Self::list(store, slug).unwrap_or_default().into_iter().filter(|c| c.service() == service).collect();
        v.sort_by_key(|c| !c.default);
        v
    }

    pub fn load(store: &Store, slug: &str, id: &str) -> Result<Connection> {
        if !valid_id(id) {
            bail!("no connection '{id}'");
        }
        match store.read_yaml_opt::<Connection>(&store.home().connections_dir(slug).join(id).join("connection.yaml"))? {
            Some(mut c) => {
                c.normalize();
                Ok(c)
            }
            None => bail!("no connection '{id}'"),
        }
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        if !valid_id(&self.id) {
            bail!("a connection id is letters, digits and dashes");
        }
        store.write_yaml(&store.home().connections_dir(slug).join(&self.id).join("connection.yaml"), self)
    }

    /// Remove it, keys included. The next one of the same service becomes the default.
    pub fn delete(store: &Store, slug: &str, id: &str) -> Result<Connection> {
        let c = Self::load(store, slug, id)?;
        std::fs::remove_dir_all(store.home().connections_dir(slug).join(id))?;
        if c.default {
            if let Some(mut next) = Self::of(store, slug, c.service()).into_iter().next() {
                next.default = true;
                next.save(store, slug)?;
            }
        }
        Ok(c)
    }

    /// Make this one the service's default, and no other.
    pub fn make_default(store: &Store, slug: &str, id: &str) -> Result<()> {
        let c = Self::load(store, slug, id)?;
        for mut o in Self::of(store, slug, c.service()) {
            let want = o.id == c.id;
            if o.default != want {
                o.default = want;
                o.save(store, slug)?;
            }
        }
        Ok(())
    }

    /// The default of a service, else its first.
    pub fn default_of(store: &Store, slug: &str, service: &str) -> Option<Connection> {
        Self::of(store, slug, service).into_iter().next()
    }

    /// A service's connection by id or label (any case).
    pub fn find(store: &Store, slug: &str, service: &str, name: &str) -> Option<Connection> {
        let n = name.trim();
        Self::of(store, slug, service).into_iter().find(|c| c.id == n || c.label.eq_ignore_ascii_case(n))
    }

    pub fn env_file(&self, home: &Home, slug: &str) -> PathBuf {
        env_file(home, slug, &self.id)
    }

    /// This connection's keys, by name. Values never leave the daemon.
    pub fn keys(&self, home: &Home, slug: &str) -> BTreeMap<String, String> {
        secrets::read_env_file(&self.env_file(home, slug))
    }

    /// Keys the ability needs that this connection does not have.
    pub fn missing(&self, home: &Home, slug: &str, m: &Manifest) -> Vec<String> {
        let keys = self.keys(home, slug);
        crate::extensions::missing_keys(m, |k| keys.contains_key(k))
    }

    /// Where a publish goes by default, in words: the default place's label, else nothing.
    pub fn destination_text(&self) -> String {
        self.place(None).map(|p| p.label.clone()).unwrap_or_default()
    }
}

/// The abilities that may publish to a channel, out of the (ability, kind)
/// pairs a project has on: those of its service that declare its kind and publish.
pub fn publishers<'a>(c: &Connection, on: &'a [(Manifest, ChannelKind)]) -> Vec<&'a (Manifest, ChannelKind)> {
    on.iter().filter(|(m, k)| m.service() == c.service() && k.kind == c.kind && k.publishes()).collect()
}

/// The one that publishes: the channel's `publisher`, else the ability it was
/// made with, else the first that can. None when no ability on publishes there.
pub fn pick_publisher<'a>(c: &Connection, on: &'a [(Manifest, ChannelKind)]) -> Option<&'a (Manifest, ChannelKind)> {
    let all = publishers(c, on);
    let by = |id: &str| all.iter().copied().find(|(m, _)| !id.is_empty() && m.id == id);
    by(&c.publisher).or_else(|| by(&c.ability)).or_else(|| all.first().copied())
}

/// One action through an account: what an ability did, on what, when.
/// `connections/<id>/activity.jsonl`, one line each, never committed.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Activity {
    pub at: DateTime<Utc>,
    pub ability: String,
    pub role: String,
    pub tool: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Connection {
    /// A folder of the account's own that outlives a call: where a hosted
    /// program keeps its state (a browser profile), shared by every ability
    /// of the service and removed with the account. Never committed.
    pub fn data_dir(&self, home: &Home, slug: &str) -> PathBuf {
        home.connections_dir(slug).join(&self.id).join("data")
    }

    fn activity_file(&self, home: &Home, slug: &str) -> PathBuf {
        home.connections_dir(slug).join(&self.id).join("activity.jsonl")
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

pub fn env_file(home: &Home, slug: &str, id: &str) -> PathBuf {
    home.connections_dir(slug).join(id).join(".env")
}

/// Every key name a setup of this ability holds: what it asks for, what its
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

/// A lookup for templates (`${NAME}`) in a call through one connection: the
/// ability's own key names come from the connection only (never the
/// project's or the machine's `.env`); anything else (the core keys) as
/// before.
pub fn env_for(home: &Home, slug: &str, conn: Option<&Connection>, m: &Manifest) -> impl Fn(&str) -> Option<String> + Clone {
    let own = key_names(m);
    let keys = conn.map(|c| c.keys(home, slug)).unwrap_or_default();
    let rest = secrets::env_fn(home, Some(slug));
    move |k: &str| if own.iter().any(|n| n == k) { keys.get(k).cloned() } else { rest(k) }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') && !id.starts_with('-')
}

/// A first account made from keys the project already had: a channel of the
/// ability's first kind when that kind needs no destination (a blog), so it
/// can be published to at once; a plain account otherwise.
fn first_account(store: &Store, slug: &str, m: &Manifest) -> Connection {
    let kind = m.channels.first().filter(|k| k.place.values().all(|spec| spec["optional"].as_bool().unwrap_or(false))).map(|k| k.kind.clone()).unwrap_or_default();
    let label = m.channels.first().filter(|_| !kind.is_empty()).map(|k| k.label.clone()).unwrap_or_else(|| m.name.clone());
    Connection { id: new_id(store, slug, &label), label, ability: m.id.clone(), service: m.service.id.clone(), default: true, created_by: "kiqr".into(), kind, allow: m.allow.clone(), ..Default::default() }
}

/// A unique id from a label: `email-to-jane`, `email-to-jane-2`.
pub fn new_id(store: &Store, slug: &str, label: &str) -> String {
    let base = crate::assets::slugify(label);
    let base = if base.is_empty() { "connection".to_string() } else { base };
    let dir = store.home().connections_dir(slug);
    let mut id = base.clone();
    let mut n = 2;
    while dir.join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// What the one-time move to connections did.
#[derive(Debug, Default)]
pub struct Migrated {
    pub channels: usize,
    pub connections: usize,
    pub keys_moved: usize,
}

/// Once per home: channels and keys from before connections become
/// connections. Each `channels/<id>` keeps its id under `connections/`, with
/// a copy of its ability's keys; an ability a project had keys for (its own
/// or the machine's) and no channel gets one connection named after it.
/// The ability's keys then leave the project's `.env`; the machine's `.env`
/// is left alone (nothing is lost; it is just no longer read for abilities).
pub fn migrate(store: &Store, manifests: &[Manifest]) -> Result<Option<Migrated>> {
    let home = store.home();
    let marker = home.projects_dir().join(".connections");
    if marker.exists() {
        return Ok(None);
    }
    let mut done = Migrated::default();
    let projects = crate::project::Project::list(store).unwrap_or_default();
    for p in &projects {
        let slug = p.slug.as_str();
        let project_env = home.project_env_file(slug);
        let copy_keys = |c: &Connection, m: &Manifest| -> Result<usize> {
            let mut n = 0;
            for name in key_names(m) {
                if let Some(v) = secrets::lookup(home, Some(slug), &name) {
                    secrets::write_env(&c.env_file(home, slug), &name, &v)?;
                    n += 1;
                }
            }
            Ok(n)
        };
        // 1. Channels keep their ids.
        let old = home.channels_dir(slug);
        if let Ok(rd) = std::fs::read_dir(&old) {
            for e in rd.flatten() {
                let Ok(Some(mut c)) = store.read_yaml_opt::<Connection>(&e.path().join("channel.yaml")) else { continue };
                if c.kind.is_empty() || !valid_id(&c.id) {
                    continue;
                }
                c.default = !Connection::of(store, slug, c.service()).iter().any(|o| o.default);
                c.save(store, slug)?;
                if let Some(m) = manifests.iter().find(|m| m.id == c.ability) {
                    copy_keys(&c, m)?;
                }
                done.channels += 1;
            }
            std::fs::remove_dir_all(&old)?;
        }
        // 2. Abilities with keys and nothing yet.
        for m in manifests.iter().filter(|m| !m.auth.is_empty()) {
            if !Connection::of(store, slug, m.service()).is_empty() {
                continue;
            }
            if !crate::extensions::missing_keys(m, |k| secrets::has(home, Some(slug), k)).is_empty() {
                continue;
            }
            let c = first_account(store, slug, m);
            c.save(store, slug)?;
            copy_keys(&c, m)?;
            done.connections += 1;
        }
        // 3. The project's `.env` keeps only what is not an ability's.
        let own = secrets::read_env_file(&project_env);
        for m in manifests {
            for name in key_names(m) {
                if own.contains_key(&name) {
                    secrets::write_env(&project_env, &name, "")?;
                    done.keys_moved += 1;
                }
            }
        }
    }
    std::fs::create_dir_all(home.projects_dir())?;
    std::fs::write(&marker, "channels and keys moved to connections/\n")?;
    Ok(Some(done))
}

/// An ability changed shape (`moved` in its manifest, or a new `service`):
/// every account made with it moves along. Keys are renamed in its `.env`, a
/// key whose value is now a place field becomes a place of the ability's
/// kind, and the account joins the manifest's service. Returns the accounts
/// changed, as `slug/id`.
pub fn move_old(store: &Store, m: &Manifest) -> Result<Vec<String>> {
    let home = store.home();
    let mut out = vec![];
    for p in crate::project::Project::list(store).unwrap_or_default() {
        let slug = p.slug.as_str();
        for mut c in Connection::list(store, slug)?.into_iter().filter(|c| c.ability == m.id) {
            let mut changed = false;
            let env_path = c.env_file(home, slug);
            let keys = secrets::read_env_file(&env_path);
            for (old, new) in &m.moved.keys {
                if let Some(v) = keys.get(old) {
                    if !keys.contains_key(new) {
                        secrets::write_env(&env_path, new, v)?;
                    }
                    secrets::write_env(&env_path, old, "")?;
                    changed = true;
                }
            }
            for (old, field) in &m.moved.places {
                if let Some(v) = keys.get(old).filter(|v| !v.trim().is_empty()) {
                    if c.place_of_kind(&m.id, None).is_none() {
                        c.put_place(Place { id: String::new(), kind: m.id.clone(), label: v.trim().to_string(), fields: [(field.clone(), Value::String(v.trim().to_string()))].into_iter().collect(), default: true });
                    }
                    secrets::write_env(&env_path, old, "")?;
                    changed = true;
                }
            }
            if c.service() != m.service() {
                // Joining a service with accounts already: not the default there.
                if Connection::of(store, slug, m.service()).iter().any(|o| o.default) {
                    c.default = false;
                }
                c.service = m.service().to_string();
                changed = true;
            }
            if changed {
                c.save(store, slug)?;
                out.push(format!("{slug}/{}", c.id));
            }
        }
    }
    Ok(out)
}

/// After an install: a project whose own `.env` still holds every key the
/// ability needs (from before connections, or typed by hand) gets them as its
/// first account, and they leave the `.env`. Returns the projects that did.
pub fn adopt(store: &Store, m: &Manifest) -> Result<Vec<String>> {
    let home = store.home();
    let mut out = vec![];
    if m.auth.is_empty() {
        return Ok(out);
    }
    for p in crate::project::Project::list(store).unwrap_or_default() {
        let slug = p.slug.as_str();
        let env_path = home.project_env_file(slug);
        let own = secrets::read_env_file(&env_path);
        if !Connection::of(store, slug, m.service()).is_empty() || !crate::extensions::missing_keys(m, |k| own.contains_key(k)).is_empty() {
            continue;
        }
        let c = first_account(store, slug, m);
        c.save(store, slug)?;
        for name in key_names(m) {
            if let Some(v) = own.get(&name) {
                secrets::write_env(&c.env_file(home, slug), &name, v)?;
                secrets::write_env(&env_path, &name, "")?;
            }
        }
        out.push(slug.to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(yaml: &str) -> Manifest {
        crate::extensions::parse_manifest(yaml).unwrap()
    }

    #[test]
    fn connections_hold_their_own_keys_and_migrate() {
        let dir = std::env::temp_dir().join(format!("kiqr-connections-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(Home::at(&dir)).unwrap();
        let home = store.home().clone();
        let p = crate::project::Project { slug: "acme".into(), name: "Acme".into(), url: "https://acme.test".into(), created_at: Utc::now(), crawl: Default::default(), last_crawl: None, abilities_off: vec![], suggestions_dismissed: vec![] };
        p.save(&store).unwrap();
        let wp = manifest("id: blog\nname: Blog\ntransport: http\nauth:\n  - { env: BLOG_SITE }\n  - { env: BLOG_KEY }\nhosts: [\"${BLOG_SITE}\"]\nchannels:\n  - { kind: blog, label: Blog, publish: post }\ntools:\n  - name: post\n    request: { method: POST, url: \"${BLOG_SITE}/p\" }\n");
        let img = manifest("id: pics\nname: Pictures\ntransport: http\nauth:\n  - { env: PICS_KEY }\nhosts: [api.pics.test]\nprovides:\n  image:\n    request: { method: POST, url: \"https://api.pics.test/x\" }\n");
        // Before: one channel, the blog keys in the project, the picture key on the machine.
        let old = home.channels_dir("acme").join("news");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("channel.yaml"), "id: news\nlabel: News\nextension: blog\nkind: blog\nasks_first: true\npublications: 3\n").unwrap();
        secrets::write_env(&home.project_env_file("acme"), "BLOG_SITE", "https://one.test").unwrap();
        secrets::write_env(&home.project_env_file("acme"), "BLOG_KEY", "k1").unwrap();
        secrets::write_env(&home.project_env_file("acme"), "OTHER", "stays").unwrap();
        std::env::set_var("PICS_KEY", "machine-key");
        let r = migrate(&store, &[wp.clone(), img.clone()]).unwrap().unwrap();
        assert_eq!((r.channels, r.connections, r.keys_moved), (1, 1, 2));
        assert!(migrate(&store, &[wp.clone()]).unwrap().is_none(), "once per home");
        let news = Connection::load(&store, "acme", "news").unwrap();
        assert!(news.is_channel() && news.default && news.publications == 3 && news.ability == "blog");
        assert_eq!(news.keys(&home, "acme").get("BLOG_KEY").map(String::as_str), Some("k1"));
        let pics = Connection::default_of(&store, "acme", "pics").unwrap();
        assert_eq!(pics.keys(&home, "acme").get("PICS_KEY").map(String::as_str), Some("machine-key"));
        let project = secrets::read_env_file(&home.project_env_file("acme"));
        assert!(!project.contains_key("BLOG_KEY") && project.get("OTHER").map(String::as_str) == Some("stays"));
        assert!(!home.channels_dir("acme").exists());

        // A second site: its own keys; the ability's names never fall back to the machine.
        let two = Connection { id: new_id(&store, "acme", "News"), label: "Second".into(), ability: "blog".into(), kind: "blog".into(), ..Default::default() };
        assert_eq!(two.id, "news-2");
        two.save(&store, "acme").unwrap();
        secrets::write_env(&two.env_file(&home, "acme"), "BLOG_SITE", "https://two.test").unwrap();
        assert_eq!(two.missing(&home, "acme", &wp), vec!["BLOG_KEY".to_string()]);
        std::env::set_var("BLOG_KEY", "machine");
        let env = env_for(&home, "acme", Some(&two), &wp);
        assert_eq!(env("BLOG_SITE").as_deref(), Some("https://two.test"));
        assert_eq!(env("BLOG_KEY"), None);
        assert_eq!(env("OTHER").as_deref(), Some("stays"));
        std::env::remove_var("BLOG_KEY");
        std::env::remove_var("PICS_KEY");
        assert_eq!(Connection::find(&store, "acme", "blog", "second").map(|c| c.id), Some("news-2".into()));
        assert_eq!(Connection::channels(&store, "acme").unwrap().len(), 2);

        // Installed later: keys left in the project's .env become its first account.
        let late = manifest("id: late\nname: Late\ntransport: http\nauth:\n  - { env: LATE_KEY }\nhosts: [api.late.test]\ntools:\n  - name: go\n    request: { method: GET, url: \"https://api.late.test/x\" }\n");
        secrets::write_env(&home.project_env_file("acme"), "LATE_KEY", "l1").unwrap();
        assert_eq!(adopt(&store, &late).unwrap(), vec!["acme".to_string()]);
        assert_eq!(Connection::default_of(&store, "acme", "late").unwrap().keys(&home, "acme").get("LATE_KEY").map(String::as_str), Some("l1"));
        assert!(!secrets::read_env_file(&home.project_env_file("acme")).contains_key("LATE_KEY"));
        assert!(adopt(&store, &late).unwrap().is_empty());

        // Defaults move along.
        Connection::make_default(&store, "acme", "news-2").unwrap();
        assert_eq!(Connection::default_of(&store, "acme", "blog").unwrap().id, "news-2");
        Connection::delete(&store, "acme", "news-2").unwrap();
        assert!(Connection::load(&store, "acme", "news").unwrap().default);

        // Two abilities of one service share its accounts: one sign-in for both.
        let reader = manifest("id: social-read\nname: Read posts\nservice: social\ntransport: http\nauth:\n  - { env: SOCIAL_TOKEN }\nhosts: [api.social.test]\ntools:\n  - name: read_posts\n    request: { method: GET, url: \"https://api.social.test/feed\" }\n");
        let replier = manifest("id: social-reply\nname: Reply\nservice: social\ntransport: http\nauth:\n  - { env: SOCIAL_TOKEN }\nhosts: [api.social.test]\ntools:\n  - name: reply\n    request: { method: POST, url: \"https://api.social.test/reply\" }\n");
        assert_eq!((reader.service(), wp.service()), ("social", "blog"));
        let me = Connection { id: new_id(&store, "acme", "Me"), label: "Me".into(), ability: reader.id.clone(), service: reader.service.id.clone(), default: true, ..Default::default() };
        me.save(&store, "acme").unwrap();
        secrets::write_env(&me.env_file(&home, "acme"), "SOCIAL_TOKEN", "t1").unwrap();
        let found = Connection::default_of(&store, "acme", replier.service()).unwrap();
        assert_eq!(found.id, "me");
        assert!(found.missing(&home, "acme", &replier).is_empty());
        assert_eq!(env_for(&home, "acme", Some(&found), &replier)("SOCIAL_TOKEN").as_deref(), Some("t1"));
        assert_eq!(Connection::find(&store, "acme", "social", "ME").map(|c| c.id), Some("me".into()));
        assert!(Connection::of(&store, "acme", "social-read").is_empty(), "an ability with a service is found by the service, not its id");
        let work = Connection { id: new_id(&store, "acme", "Work"), label: "Work".into(), ability: replier.id.clone(), service: replier.service.id.clone(), ..Default::default() };
        work.save(&store, "acme").unwrap();
        Connection::make_default(&store, "acme", "work").unwrap();
        assert_eq!(Connection::default_of(&store, "acme", "social").unwrap().id, "work");
        assert!(!Connection::load(&store, "acme", "me").unwrap().default);
        assert!(Connection::load(&store, "acme", "news").unwrap().default, "defaults are per service");
        Connection::delete(&store, "acme", "work").unwrap();
        assert!(Connection::load(&store, "acme", "me").unwrap().default);
        // Several abilities on one channel: exactly one publishes, the user's pick first.
        let poster = |id: &str| manifest(&format!("id: {id}\nname: {id}\nservice: social\ntransport: http\nhosts: [api.social.test]\nchannels:\n  - {{ kind: page, label: Page, publish: post }}\ntools:\n  - name: post\n    request: {{ method: POST, url: \"https://api.social.test/p\" }}\n"));
        let watcher = manifest("id: social-watch\nname: Watch\nservice: social\ntransport: http\nhosts: [api.social.test]\nchannels:\n  - { kind: page, label: Page, does: [read, reply] }\ntools:\n  - name: read_page\n    request: { method: GET, url: \"https://api.social.test/r\" }\n");
        let other = manifest("id: elsewhere\nname: Elsewhere\ntransport: http\nhosts: [api.else.test]\nchannels:\n  - { kind: page, label: Page, publish: post }\ntools:\n  - name: post\n    request: { method: POST, url: \"https://api.else.test/p\" }\n");
        assert_eq!(watcher.channels[0].roles(), vec!["read", "reply"]);
        assert!(!watcher.channels[0].publishes() && poster("x").channels[0].publishes());
        let on: Vec<(Manifest, ChannelKind)> = [watcher.clone(), poster("social-b"), other, poster("social-a")].into_iter().flat_map(|m| m.channels.clone().into_iter().map(move |k| (m.clone(), k))).collect();
        let mut page = Connection { id: "page".into(), label: "Page".into(), ability: "social-a".into(), service: "social".into(), kind: "page".into(), ..Default::default() };
        let ids = |v: Vec<&(Manifest, ChannelKind)>| v.into_iter().map(|(m, _)| m.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(publishers(&page, &on)), vec!["social-b", "social-a"], "readers and other services never publish here");
        assert_eq!(pick_publisher(&page, &on).unwrap().0.id, "social-a", "the one it was made with");
        page.publisher = "social-b".into();
        assert_eq!(pick_publisher(&page, &on).unwrap().0.id, "social-b", "the user's pick");
        page.publisher = "gone".into();
        page.ability = "social-watch".into();
        assert_eq!(pick_publisher(&page, &on).unwrap().0.id, "social-b", "else the first that can");
        assert!(pick_publisher(&page, &on[..1]).is_none());
        assert!(crate::extensions::parse_manifest("id: bad\nname: Bad\ntransport: http\nhosts: [a.test]\nchannels:\n  - { kind: page, label: Page, does: [shout] }\n").is_err());
        assert!(crate::extensions::parse_manifest("id: bad\nname: Bad\ntransport: http\nhosts: [a.test]\nchannels:\n  - { kind: page, label: Page }\n").is_err(), "publishing needs a publish tool");

        // The account's log: what was done on what, and how much today.
        let act = |role: &str, target: &str, at: DateTime<Utc>| Activity { at, ability: "social-reply".into(), role: role.into(), tool: "reply".into(), target: target.into(), url: None };
        assert!(me.activity(&home, "acme").is_empty() && me.actions_today(&home, "acme") == 0);
        me.record(&home, "acme", &act("reply", "post-1", Utc::now())).unwrap();
        me.record(&home, "acme", &act("reply", "post-2", Utc::now() - chrono::Duration::days(2))).unwrap();
        me.record(&home, "acme", &act("read", "", Utc::now())).unwrap();
        assert_eq!(me.activity(&home, "acme").len(), 3);
        assert_eq!(me.acted_on(&home, "acme", "reply", "post-2").map(|a| a.target), Some("post-2".into()));
        assert!(me.acted_on(&home, "acme", "manage", "post-1").is_none(), "another role on the same target is another action");
        assert_eq!(me.actions_today(&home, "acme"), 1, "reading and older days do not count");
        assert!(me.data_dir(&home, "acme").ends_with("connections/me/data"));

        // Older records with no service are the ability's own.
        assert_eq!(Connection::load(&store, "acme", "news").unwrap().service(), "blog");

        // Permissions: acting asks by default, the rest is on; a role switched off stops publishing.
        let mut acct = Connection { id: "x".into(), label: "X".into(), kind: "page".into(), ..Default::default() };
        assert_eq!((acct.allow("publish"), acct.allow("reply"), acct.allow("read"), acct.allow("learn")), (Allow::Ask, Allow::Ask, Allow::Auto, Allow::Auto));
        assert!(acct.publishes() && acct.learns());
        acct.allow.insert("publish".into(), Allow::Off);
        acct.allow.insert("learn".into(), Allow::Off);
        assert!(!acct.publishes() && !acct.learns() && acct.is_channel());
        assert_eq!((Allow::parse("on"), Allow::parse("ASK"), Allow::parse("nope")), (Some(Allow::Auto), Some(Allow::Ask), None));
        assert!(Allow::acts("manage") && !Allow::acts("measure"));

        // Places: the first is the default, one default at a time, names resolve, removing moves the default.
        acct.put_place(Place { label: "r/rust".into(), kind: "page".into(), fields: [("subreddit".to_string(), serde_json::json!("rust"))].into_iter().collect(), ..Default::default() });
        acct.put_place(Place { label: "r/selfhosted".into(), kind: "page".into(), ..Default::default() });
        assert_eq!(acct.places[0].id, "r-rust");
        assert!(acct.places[0].default && !acct.places[1].default);
        assert_eq!(acct.place(None).map(|p| p.label.as_str()), Some("r/rust"));
        assert_eq!(acct.place(Some("R/SELFHOSTED")).map(|p| p.id.as_str()), Some("r-selfhosted"));
        assert!(acct.place(Some("nowhere")).is_none());
        acct.put_place(Place { id: "r-selfhosted".into(), label: "r/selfhosted".into(), kind: "page".into(), default: true, ..Default::default() });
        assert!(acct.places[1].default && !acct.places[0].default);
        assert_eq!(acct.destination_text(), "r/selfhosted");
        acct.remove_place("r-selfhosted");
        assert!(acct.places[0].default && acct.places.len() == 1);
        let list = Place { id: "list-news".into(), kind: Place::LIST.into(), label: "News".into(), fields: [("list".to_string(), serde_json::json!("news"))].into_iter().collect(), default: false };
        assert_eq!(list.list_id(), Some("news"));

        // A record from before permissions and places reads into the new shape and is written that way.
        let old = home.connections_dir("acme").join("legacy");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("connection.yaml"), "id: legacy\nlabel: Legacy\nability: blog\nkind: email\ndestination:\n  to: jane@example.com\n  subject: News\nasks_first: false\nlearns: true\npublications: 2\n").unwrap();
        let legacy = Connection::load(&store, "acme", "legacy").unwrap();
        assert_eq!((legacy.allow("publish"), legacy.allow("learn"), legacy.allow("reply")), (Allow::Auto, Allow::Auto, Allow::Ask));
        assert_eq!(legacy.places.len(), 1);
        assert_eq!((legacy.places[0].label.as_str(), legacy.places[0].fields["to"].as_str(), legacy.places[0].default), ("jane@example.com", Some("jane@example.com"), true));
        assert_eq!(legacy.publications, 2);
        legacy.save(&store, "acme").unwrap();
        let text = std::fs::read_to_string(old.join("connection.yaml")).unwrap();
        assert!(!text.contains("asks_first") && !text.contains("destination:") && !text.contains("learns") && text.contains("allow:") && text.contains("places:"), "{text}");
        let again = Connection::load(&store, "acme", "legacy").unwrap();
        assert_eq!(again.allow("publish"), Allow::Auto);
        // An ability that changed shape: its accounts move along (keys renamed, a key into a place, a new service).
        let ga_old = manifest("id: ga\nname: Analytics\ntransport: http\nauth:\n  - { env: GA_PROPERTY }\n  - { env: GA_TOKEN, optional: true }\nhosts: [api.g.test]\ntools:\n  - name: read\n    request: { method: GET, url: \"https://api.g.test/${GA_PROPERTY}\" }\n");
        let ga = Connection { id: "ga".into(), label: "GA".into(), ability: "ga".into(), default: true, ..Default::default() };
        ga.save(&store, "acme").unwrap();
        secrets::write_env(&ga.env_file(&home, "acme"), "GA_PROPERTY", "123").unwrap();
        secrets::write_env(&ga.env_file(&home, "acme"), "GA_TOKEN", "t").unwrap();
        let _ = ga_old;
        let ga_new = manifest("id: ga\nname: Analytics\nservice: { id: google, name: Google, group: numbers }\ntransport: http\nauth:\n  - { env: G_TOKEN, optional: true }\nhosts: [api.g.test]\nplaces:\n  property: { label: Property }\nmoved:\n  keys: { GA_TOKEN: G_TOKEN }\n  places: { GA_PROPERTY: property }\ntools:\n  - name: read\n    request: { method: GET, url: \"https://api.g.test/{{property}}\" }\n");
        assert_eq!(move_old(&store, &ga_new).unwrap(), vec!["acme/ga".to_string()]);
        let moved = Connection::load(&store, "acme", "ga").unwrap();
        assert_eq!(moved.service(), "google");
        let keys = moved.keys(&home, "acme");
        assert_eq!((keys.get("G_TOKEN").map(String::as_str), keys.get("GA_TOKEN"), keys.get("GA_PROPERTY")), (Some("t"), None, None));
        let place = moved.place_of_kind("ga", None).unwrap();
        assert_eq!((place.label.as_str(), place.fields["property"].as_str(), place.default), ("123", Some("123"), true));
        assert!(move_old(&store, &ga_new).unwrap().is_empty(), "moving is idempotent");
        // Defaults are per kind: a place of another kind on the same account is a default too.
        let mut two = moved.clone();
        two.put_place(Place { label: "acme.com".into(), kind: "gsc".into(), ..Default::default() });
        assert!(two.place_of_kind("gsc", None).unwrap().default && two.place_of_kind("ga", None).unwrap().default);
        assert!(crate::extensions::parse_manifest("id: bad\nname: Bad\ntransport: http\nhosts: [a.test]\nmoved:\n  places: { OLD: nowhere }\n").is_err());
        // One that never said: learning stays on by default, publishing asks.
        std::fs::write(old.join("connection.yaml"), "id: legacy\nlabel: Legacy\nability: blog\nkind: blog\n").unwrap();
        let plain = Connection::load(&store, "acme", "legacy").unwrap();
        assert_eq!((plain.allow("publish"), plain.allow("learn")), (Allow::Ask, Allow::Auto));
        assert!(plain.places.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
