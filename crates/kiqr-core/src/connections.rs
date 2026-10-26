//! Connections: one named account of a service inside a project, with its
//! own keys. Every installed ability of that service (its manifest's
//! `service`, else its id) uses the project's accounts of it: one sign-in for
//! all of them. A project can have as many as it wants (two WordPress
//! sites, two analytics properties). A connection whose ability publishes is
//! a channel: it also has a kind, a destination, "asks first" and a count of
//! what went out. Installing an ability only puts its code on the machine;
//! everything that says *which* account lives here.
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
    /// A channel's kind (`email`, `wordpress`…); empty for a connection that does not publish.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub kind: String,
    /// Destination fields as the kind declares them: `to`, `subject`…
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub destination: BTreeMap<String, Value>,
    /// The ability that publishes here, when the user picked one of several
    /// (`pick_publisher`). Empty: the one it was made with, else the first on.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub publisher: String,
    /// A publish Seemo proposes shows a card and waits for a yes. Off, Seemo
    /// publishes here on its own and the card only records what went out.
    pub asks_first: bool,
    pub publications: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_published: Option<DateTime<Utc>>,
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
            destination: BTreeMap::new(),
            publisher: String::new(),
            asks_first: true,
            publications: 0,
            last_published: None,
        }
    }
}

impl Connection {
    pub fn is_channel(&self) -> bool {
        !self.kind.is_empty()
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
                if let Ok(Some(c)) = store.read_yaml_opt::<Connection>(&f) {
                    out.push(c);
                }
            }
        }
        out.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    /// The project's channels: connections that publish.
    pub fn channels(store: &Store, slug: &str) -> Result<Vec<Connection>> {
        Ok(Self::list(store, slug)?.into_iter().filter(Connection::is_channel).collect())
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
            Some(c) => Ok(c),
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

    /// `jane@example.com` for email, else the first destination value.
    pub fn destination_text(&self) -> String {
        let pick = |k: &str| self.destination.get(k).and_then(|v| v.as_str()).map(str::to_string);
        pick("to").or_else(|| pick("site")).or_else(|| pick("url")).or_else(|| pick("account")).or_else(|| self.destination.values().find_map(|v| v.as_str().map(str::to_string))).unwrap_or_default()
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

pub fn env_file(home: &Home, slug: &str, id: &str) -> PathBuf {
    home.connections_dir(slug).join(id).join(".env")
}

/// Every key name a setup of this ability holds: what it asks for, what its
/// sign-in writes, what its program or commands are given.
pub fn key_names(m: &Manifest) -> Vec<String> {
    let mut v: Vec<String> = m.auth.iter().map(|a| a.env.clone()).collect();
    if let Some(c) = &m.connect {
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
    let kind = m.channels.first().filter(|k| k.destination.values().all(|spec| spec["optional"].as_bool().unwrap_or(false))).map(|k| k.kind.clone()).unwrap_or_default();
    let label = m.channels.first().filter(|_| !kind.is_empty()).map(|k| k.label.clone()).unwrap_or_else(|| m.name.clone());
    Connection { id: new_id(store, slug, &label), label, ability: m.id.clone(), service: m.service.clone(), default: true, created_by: "kiqr".into(), kind, ..Default::default() }
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
        let me = Connection { id: new_id(&store, "acme", "Me"), label: "Me".into(), ability: reader.id.clone(), service: reader.service.clone(), default: true, ..Default::default() };
        me.save(&store, "acme").unwrap();
        secrets::write_env(&me.env_file(&home, "acme"), "SOCIAL_TOKEN", "t1").unwrap();
        let found = Connection::default_of(&store, "acme", replier.service()).unwrap();
        assert_eq!(found.id, "me");
        assert!(found.missing(&home, "acme", &replier).is_empty());
        assert_eq!(env_for(&home, "acme", Some(&found), &replier)("SOCIAL_TOKEN").as_deref(), Some("t1"));
        assert_eq!(Connection::find(&store, "acme", "social", "ME").map(|c| c.id), Some("me".into()));
        assert!(Connection::of(&store, "acme", "social-read").is_empty(), "an ability with a service is found by the service, not its id");
        let work = Connection { id: new_id(&store, "acme", "Work"), label: "Work".into(), ability: replier.id.clone(), service: replier.service.clone(), ..Default::default() };
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

        // Older records with no service are the ability's own.
        assert_eq!(Connection::load(&store, "acme", "news").unwrap().service(), "blog");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
