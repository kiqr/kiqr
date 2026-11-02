//! Channels: everywhere the business's people are. A channel is **owned**
//! (the company's X profile, its Facebook page, its blog, a mailing list) or
//! **third-party** (a subreddit, a forum category, a Slack channel). It stands
//! alone: it knows its site (`domain`), how a spot there is named (a kind's
//! target fields, `subreddit: selfhosted`) and what Seemo may do there
//! (`read`, `publish`, `reply`, `learn`). To publish it needs a **publisher**:
//! who Seemo posts as. Nothing else holds permissions.
//!
//! `projects/<slug>/channels/<id>/channel.yaml`, committed. A mailing list is
//! a channel of kind `list` and keeps its people beside it (`contacts.csv`,
//! `sends/`), never committed (`crate::lists`).

use crate::extensions::Kind;
use crate::Store;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The kind of a mailing list.
pub const LIST: &str = "list";

/// What Seemo may do in a channel for an act that changes something there.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Never.
    Off,
    /// Only after the user's yes.
    #[default]
    Ask,
    /// On its own; the card in the chat is the record.
    Auto,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" | "no" => Some(Mode::Off),
            "ask" | "approval" => Some(Mode::Ask),
            "auto" | "on" | "yes" => Some(Mode::Auto),
            _ => None,
        }
    }
    pub fn word(self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::Ask => "ask",
            Mode::Auto => "auto",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Ownership {
    /// The business's own: a profile, a page, a blog, a list.
    #[default]
    Owned,
    /// Somewhere it takes part: a subreddit, a forum, a group.
    ThirdParty,
}

impl Ownership {
    pub fn parse(s: &str) -> Option<Ownership> {
        match s.trim().to_ascii_lowercase().replace(['-', ' '], "_").as_str() {
            "owned" | "own" | "page" | "list" => Some(Ownership::Owned),
            "third_party" | "community" | "external" => Some(Ownership::ThirdParty),
            _ => None,
        }
    }
    /// Where a kind's spots usually belong: a community is someone else's.
    pub fn of_kind(k: &Kind) -> Ownership {
        if k.section() == "community" { Ownership::ThirdParty } else { Ownership::Owned }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Channel {
    pub id: String,
    /// As people say it: `@acme on X`, `r/selfhosted`, `Newsletter`.
    pub label: String,
    pub ownership: Ownership,
    /// The site it is on (`x.com`, `reddit.com`); its favicon is the channel's icon. Empty for a list.
    pub domain: String,
    /// A kind a manifest declares, or `list`. Empty: no ability here knows the site yet; one that
    /// declares a kind on the domain picks it up.
    pub kind: String,
    /// The kind's target fields (`subreddit: selfhosted`); empty when the profile itself is the channel.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Who Seemo publishes as. Empty: nobody yet, so nothing is published here.
    pub publisher: String,
    /// The ability that publishes here, when several could.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub ability: String,
    /// Seemo may read here (posts, replies, mentions).
    pub read: bool,
    pub publish: Mode,
    /// Replying and engaging (likes, follows, moderation).
    pub reply: Mode,
    /// Owned channels only: what is there teaches the profile (a repository's README and releases).
    pub learn: bool,
    /// `user`, `seemo`, `crawl`, `publish` (made with a yes to a new spot).
    pub added_by: String,
    /// Waits for the user's Review: nothing goes there until it is added.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub suggested: bool,
    /// Why Seemo suggested it, in a line.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub reason: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "is_zero")]
    pub publications: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_published: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_learned: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "is_zero")]
    pub learned: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            ownership: Ownership::Owned,
            domain: String::new(),
            kind: String::new(),
            fields: BTreeMap::new(),
            url: String::new(),
            description: String::new(),
            publisher: String::new(),
            ability: String::new(),
            read: true,
            publish: Mode::Ask,
            reply: Mode::Ask,
            learn: false,
            added_by: "user".into(),
            suggested: false,
            reason: String::new(),
            created_at: Utc::now(),
            publications: 0,
            last_published: None,
            last_learned: None,
            learned: 0,
        }
    }
}

/// An id: lowercase letters, digits and dashes.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') && !id.starts_with('-')
}

/// Fields compared without case.
pub fn same_fields(a: &BTreeMap<String, Value>, b: &BTreeMap<String, Value>) -> bool {
    let norm = |m: &BTreeMap<String, Value>| m.iter().map(|(k, v)| (k.clone(), v.as_str().map(str::to_lowercase).unwrap_or_else(|| v.to_string()))).collect::<BTreeMap<_, _>>();
    norm(a) == norm(b)
}

/// A link without scheme, `www.` or a trailing slash, for comparing.
fn bare_url(u: &str) -> String {
    let u = u.trim().to_ascii_lowercase();
    let u = u.split_once("://").map(|(_, r)| r.to_string()).unwrap_or(u);
    u.trim_start_matches("www.").trim_end_matches('/').to_string()
}

/// The host of a link or a kind's URL template (`https://reddit.com/r/{{subreddit}}` → `reddit.com`).
pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.contains("{{") {
        return String::new();
    }
    crate::vault::normalise_host(host).unwrap_or_default()
}

impl Channel {
    pub fn is_list(&self) -> bool {
        self.kind == LIST
    }

    pub fn owned(&self) -> bool {
        self.ownership == Ownership::Owned
    }

    /// What Seemo may do here for a tool role: `publish`; `reply` and `manage` (engaging);
    /// `read` and `measure` (on or off); `learn`.
    pub fn mode(&self, role: &str) -> Mode {
        let on = |b: bool| if b { Mode::Auto } else { Mode::Off };
        match role {
            "publish" => self.publish,
            "reply" | "manage" => self.reply,
            "learn" => on(self.learn && self.owned()),
            _ => on(self.read),
        }
    }

    /// The same place as another: the same kind and fields; where the profile itself is the
    /// channel (no fields), the same link, or else the same publisher.
    pub fn is_same(&self, o: &Channel) -> bool {
        if self.is_list() || o.is_list() || self.kind != o.kind || !same_fields(&self.fields, &o.fields) {
            return false;
        }
        if !self.fields.is_empty() {
            return true;
        }
        match (self.url.is_empty(), o.url.is_empty()) {
            (false, false) => bare_url(&self.url) == bare_url(&o.url),
            _ => !self.publisher.is_empty() && self.publisher == o.publisher,
        }
    }

    pub fn all(store: &Store, slug: &str) -> Result<Vec<Channel>> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(store.home().channels_dir(slug)) {
            for e in rd.flatten() {
                if let Ok(Some(x)) = store.read_yaml_opt::<Channel>(&e.path().join("channel.yaml")) {
                    out.push(x);
                }
            }
        }
        out.sort_by(|a, b| a.ownership.cmp(&b.ownership).then(a.is_list().cmp(&b.is_list())).then(a.label.to_lowercase().cmp(&b.label.to_lowercase())));
        Ok(out)
    }

    pub fn load(store: &Store, slug: &str, id: &str) -> Result<Channel> {
        if !valid_id(id) {
            bail!("there is no channel '{id}'");
        }
        match store.read_yaml_opt::<Channel>(&store.home().channels_dir(slug).join(id).join("channel.yaml"))? {
            Some(x) => Ok(x),
            None => bail!("there is no channel '{id}'"),
        }
    }

    /// By id or label, any case.
    pub fn find(store: &Store, slug: &str, name: &str) -> Option<Channel> {
        let n = name.trim().trim_start_matches('@');
        Self::all(store, slug).unwrap_or_default().into_iter().find(|x| x.id == n || x.label.trim_start_matches('@').eq_ignore_ascii_case(n))
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        if !valid_id(&self.id) {
            bail!("a channel id is letters, digits and dashes");
        }
        if self.label.trim().is_empty() {
            bail!("a channel needs a name");
        }
        if !self.is_list() && self.domain.is_empty() && self.kind.is_empty() {
            bail!("a channel needs the site it is on");
        }
        if let Some(o) = Self::all(store, slug)?.into_iter().find(|o| o.id != self.id && o.is_same(self)) {
            bail!("this is already the channel {}", o.label);
        }
        store.write_yaml(&store.home().channels_dir(slug).join(&self.id).join("channel.yaml"), self)
    }

    /// Remove it; a list goes with everyone on it.
    pub fn delete(store: &Store, slug: &str, id: &str) -> Result<Channel> {
        let x = Self::load(store, slug, id)?;
        std::fs::remove_dir_all(store.home().channels_dir(slug).join(id))?;
        Ok(x)
    }

    /// A unique id from a label: `r-selfhosted`, `r-selfhosted-2`.
    pub fn new_id(store: &Store, slug: &str, label: &str) -> String {
        let base = crate::assets::slugify(label);
        let base = if base.is_empty() { "channel".to_string() } else { base };
        let root = store.home().channels_dir(slug);
        let mut id = base.clone();
        let mut n = 2;
        while root.join(&id).exists() {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }

    /// A new channel for a spot of a kind: name, link and site from the kind, fields checked.
    /// `who` names the profile when the profile itself is the spot.
    pub fn for_spot(kind: &Kind, who: &str, fields: BTreeMap<String, Value>, added_by: &str) -> Result<Channel> {
        let fields = if kind.account_is_target { BTreeMap::new() } else { kind.check_target(&fields)? };
        let url = kind.url_of(&fields);
        let domain = [host_of(&url), host_of(&kind.url)].into_iter().find(|h| !h.is_empty()).unwrap_or_default();
        Ok(Channel { label: spot_label(kind, who, &fields), ownership: Ownership::of_kind(kind), domain, kind: kind.kind.clone(), url, fields, added_by: added_by.to_string(), ..Default::default() })
    }
}

/// How a spot reads: the profile's own name when it is the spot, `r/selfhosted`
/// style when the link says so, else the fields' values.
pub fn spot_label(kind: &Kind, who: &str, fields: &BTreeMap<String, Value>) -> String {
    if kind.account_is_target || fields.is_empty() {
        return if who.is_empty() { kind.display_name().to_string() } else { who.to_string() };
    }
    let vals: Vec<&str> = fields.values().filter_map(Value::as_str).collect();
    // `https://reddit.com/r/{{subreddit}}`: the path piece before the one field reads as its prefix.
    if let (1, Some((_, path))) = (vals.len(), kind.url.split_once("://")) {
        let segs: Vec<&str> = path.split('/').collect();
        if let Some(i) = segs.iter().position(|s| s.starts_with("{{")) {
            if i >= 2 && segs[i - 1].len() <= 2 {
                return format!("{}/{}", segs[i - 1], vals[0]);
            }
        }
    }
    vals.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Home;

    fn reddit() -> Kind {
        let m = crate::extensions::parse_manifest("id: rb\nname: R\ntransport: http\nhosts: [r.test]\ntools:\n  - { name: post, label: Posting, description: Post., role: publish, request: { method: POST, url: \"https://r.test/{{subreddit}}\" } }\nkinds:\n  - kind: reddit\n    label: Subreddit\n    section: community\n    publish: post\n    url: \"https://reddit.com/r/{{subreddit}}\"\n    target: { subreddit: { label: Subreddit, pattern: \"^[A-Za-z0-9_]{2,21}$\" } }\n    target_from: { post_url: \"reddit.com/r/{subreddit}/\" }\n").unwrap();
        m.kinds[0].clone()
    }

    #[test]
    fn channels_are_named_checked_and_unique() {
        let k = reddit();
        let f = |v: &str| -> BTreeMap<String, Value> { [("subreddit".to_string(), Value::String(v.into()))].into_iter().collect() };
        assert_eq!(k.target_of_url("post_url", "https://old.reddit.com/r/selfhosted/comments/abc/x/"), Some(f("selfhosted")));
        assert!(k.check_target(&f("a/../b")).is_err() && k.check_target(&BTreeMap::new()).unwrap_err().to_string().contains("needs subreddit"));
        let c = Channel::for_spot(&k, "", f("selfhosted"), "user").unwrap();
        assert_eq!((c.label.as_str(), c.ownership, c.url.as_str(), c.domain.as_str()), ("r/selfhosted", Ownership::ThirdParty, "https://reddit.com/r/selfhosted", "reddit.com"));
        assert_eq!((c.mode("publish"), c.mode("manage"), c.mode("read"), c.mode("learn")), (Mode::Ask, Mode::Ask, Mode::Auto, Mode::Off));

        let dir = std::env::temp_dir().join(format!("kiqr-chan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(Home::at(&dir)).unwrap();
        let mut c = c;
        c.id = Channel::new_id(&store, "acme", &c.label);
        c.save(&store, "acme").unwrap();
        let twin = Channel { id: "twin".into(), fields: f("SelfHosted"), publisher: "someone-else".into(), ..c.clone() };
        assert!(twin.save(&store, "acme").unwrap_err().to_string().contains("already"), "the place, whoever posts there");
        // Two profiles on one site are two channels: by link, else by who posts there.
        let x = |url: &str, p: &str| Channel { id: crate::assets::slugify(&format!("{url}{p}")), label: "X".into(), domain: "x.com".into(), kind: "x".into(), url: url.into(), publisher: p.into(), ..Default::default() };
        x("https://x.com/acme", "").save(&store, "acme").unwrap();
        assert!(x("x.com/acme/", "a").save(&store, "acme").is_err());
        x("https://x.com/acme_dev", "").save(&store, "acme").unwrap();
        assert_eq!(Channel::find(&store, "acme", "R/SELFHOSTED").map(|x| x.id), Some("r-selfhosted".into()));
        assert_eq!(host_of("https://www.Reddit.com/r/{{subreddit}}"), "reddit.com");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
