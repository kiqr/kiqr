//! The Audience: everywhere the business's people are, each reached through
//! one of the project's accounts. Three sections: **pages** (the business's
//! own: an X profile, a company page, a blog), **communities** (where it takes
//! part: a subreddit, a group) and **lists** (people it emails, `crate::lists`).
//! A post is approved for an entry; Seemo acting on its own (`allow: auto` on
//! the account) happens only in entries, anywhere else asks first.
//!
//! An entry names its kind (one a manifest declares, or `list`), the account
//! that reaches it, and the kind's target fields (`subreddit: selfhosted`).
//! `(account, kind, fields)` is unique. Stored as
//! `projects/<slug>/audience/<id>/entry.yaml`, committed; a list keeps its
//! people beside it (`contacts.csv`, `sends/`), never committed.

use crate::extensions::Kind;
use crate::Store;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The kind of an email list.
pub const LIST: &str = "list";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Entry {
    pub id: String,
    /// As people say it: `r/selfhosted`, `Acme on Facebook`, `Newsletter`.
    pub label: String,
    /// `page`, `community` or `list`.
    pub section: String,
    /// A kind a manifest declares, or `list`.
    pub kind: String,
    /// The account that reaches it (a list: the account it is sent from). Empty: none yet.
    pub account: String,
    /// The kind's target fields (`subreddit: selfhosted`); empty for a list or an account that is the spot.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// `user`, `seemo`, `account` (made with it), `discover` (found through it), `crawl`.
    pub added_by: String,
    /// Waits for the user's Review: nothing goes there on its own until it is added.
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
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl Default for Entry {
    fn default() -> Self {
        Self { id: String::new(), label: String::new(), section: "page".into(), kind: String::new(), account: String::new(), fields: BTreeMap::new(), url: String::new(), description: String::new(), added_by: "user".into(), suggested: false, reason: String::new(), created_at: Utc::now(), publications: 0, last_published: None }
    }
}

impl Entry {
    pub fn is_list(&self) -> bool {
        self.kind == LIST
    }

    /// The same spot: same account, kind and fields (text compared without case).
    pub fn is(&self, account: &str, kind: &str, fields: &BTreeMap<String, Value>) -> bool {
        let norm = |m: &BTreeMap<String, Value>| m.iter().map(|(k, v)| (k.clone(), v.as_str().map(str::to_lowercase).unwrap_or_else(|| v.to_string()))).collect::<BTreeMap<_, _>>();
        self.account == account && self.kind == kind && norm(&self.fields) == norm(fields)
    }

    pub fn all(store: &Store, slug: &str) -> Result<Vec<Entry>> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(store.home().audience_dir(slug)) {
            for e in rd.flatten() {
                if let Ok(Some(x)) = store.read_yaml_opt::<Entry>(&e.path().join("entry.yaml")) {
                    out.push(x);
                }
            }
        }
        let order = |s: &str| crate::extensions::SECTIONS.iter().position(|x| *x == s).unwrap_or(9);
        out.sort_by(|a, b| order(&a.section).cmp(&order(&b.section)).then(a.label.to_lowercase().cmp(&b.label.to_lowercase())));
        Ok(out)
    }

    pub fn load(store: &Store, slug: &str, id: &str) -> Result<Entry> {
        if !crate::accounts::valid_id(id) {
            bail!("nothing in the Audience is called '{id}'");
        }
        match store.read_yaml_opt::<Entry>(&store.home().audience_dir(slug).join(id).join("entry.yaml"))? {
            Some(x) => Ok(x),
            None => bail!("nothing in the Audience is called '{id}'"),
        }
    }

    /// By id or label, any case.
    pub fn find(store: &Store, slug: &str, name: &str) -> Option<Entry> {
        let n = name.trim().trim_start_matches('@');
        Self::all(store, slug).unwrap_or_default().into_iter().find(|x| x.id == n || x.label.trim_start_matches('@').eq_ignore_ascii_case(n))
    }

    /// The entry for a spot, when the Audience has it (suggestions do not count).
    pub fn matching(store: &Store, slug: &str, account: &str, kind: &str, fields: &BTreeMap<String, Value>) -> Option<Entry> {
        Self::all(store, slug).unwrap_or_default().into_iter().find(|x| !x.suggested && x.is(account, kind, fields))
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        if !crate::accounts::valid_id(&self.id) {
            bail!("an Audience id is letters, digits and dashes");
        }
        if !crate::extensions::SECTIONS.contains(&self.section.as_str()) {
            bail!("'{}' is not a section of the Audience", self.section);
        }
        if self.label.trim().is_empty() || self.kind.is_empty() {
            bail!("an Audience entry needs a name and a kind");
        }
        if let Some(o) = Self::all(store, slug)?.into_iter().find(|o| o.id != self.id && !self.is_list() && o.is(&self.account, &self.kind, &self.fields)) {
            bail!("the Audience already has this as {}", o.label);
        }
        store.write_yaml(&store.home().audience_dir(slug).join(&self.id).join("entry.yaml"), self)
    }

    /// Remove it; a list goes with everyone on it.
    pub fn delete(store: &Store, slug: &str, id: &str) -> Result<Entry> {
        let x = Self::load(store, slug, id)?;
        std::fs::remove_dir_all(store.home().audience_dir(slug).join(id))?;
        Ok(x)
    }

    /// A unique id from a label: `r-selfhosted`, `r-selfhosted-2`.
    pub fn new_id(store: &Store, slug: &str, label: &str) -> String {
        let base = crate::assets::slugify(label);
        let base = if base.is_empty() { "entry".to_string() } else { base };
        let root = store.home().audience_dir(slug);
        let mut id = base.clone();
        let mut n = 2;
        while root.join(&id).exists() {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }

    /// A new entry for a spot of a kind: label and link from the kind, fields checked.
    pub fn for_spot(kind: &Kind, account: &str, account_label: &str, fields: BTreeMap<String, Value>, added_by: &str) -> Result<Entry> {
        let fields = if kind.account_is_target { BTreeMap::new() } else { kind.check_target(&fields)? };
        Ok(Entry { label: spot_label(kind, account_label, &fields), section: kind.section().to_string(), kind: kind.kind.clone(), account: account.to_string(), url: kind.url_of(&fields), fields, added_by: added_by.to_string(), ..Default::default() })
    }
}

/// How a spot reads: the account's own name when the account is the spot,
/// `r/selfhosted` style when the link says so, else the fields' values.
pub fn spot_label(kind: &Kind, account_label: &str, fields: &BTreeMap<String, Value>) -> String {
    if kind.account_is_target || fields.is_empty() {
        return account_label.to_string();
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
    fn spots_are_named_checked_and_unique() {
        let k = reddit();
        let f = |v: &str| -> BTreeMap<String, Value> { [("subreddit".to_string(), Value::String(v.into()))].into_iter().collect() };
        assert_eq!(k.target_of_url("post_url", "https://old.reddit.com/r/selfhosted/comments/abc/x/"), Some(f("selfhosted")));
        assert_eq!(k.target_of_url("post_url", "https://example.com/r/x/"), None);
        assert!(k.check_target(&f("a/../b")).is_err() && k.check_target(&BTreeMap::new()).unwrap_err().to_string().contains("needs subreddit"));
        let e = Entry::for_spot(&k, "u-acme", "u/acme", f("selfhosted"), "user").unwrap();
        assert_eq!((e.label.as_str(), e.section.as_str(), e.url.as_str()), ("r/selfhosted", "community", "https://reddit.com/r/selfhosted"));

        let dir = std::env::temp_dir().join(format!("kiqr-aud-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(Home::at(&dir)).unwrap();
        let mut e = e;
        e.id = Entry::new_id(&store, "acme", &e.label);
        e.save(&store, "acme").unwrap();
        assert!(Entry::matching(&store, "acme", "u-acme", "reddit", &f("SelfHosted")).is_some(), "case does not matter");
        assert!(Entry::matching(&store, "acme", "u-other", "reddit", &f("selfhosted")).is_none(), "another account is another spot");
        let twin = Entry { id: "twin".into(), ..e.clone() };
        assert!(twin.save(&store, "acme").unwrap_err().to_string().contains("already"));
        let mut s = Entry::for_spot(&k, "u-acme", "u/acme", f("homelab"), "seemo").unwrap();
        s.id = "r-homelab".into();
        s.suggested = true;
        s.save(&store, "acme").unwrap();
        assert!(Entry::matching(&store, "acme", "u-acme", "reddit", &f("homelab")).is_none(), "a suggestion is not in the Audience yet");
        assert_eq!(Entry::find(&store, "acme", "R/SELFHOSTED").map(|x| x.id), Some("r-selfhosted".into()));
    }
}
