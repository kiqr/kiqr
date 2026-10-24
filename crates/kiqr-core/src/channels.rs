//! Channels: where a project's content goes out. A channel names a connection
//! that can publish (its manifest lists channel kinds), a kind, and a
//! destination (an address, a subreddit, a blog). Publishing always asks the
//! user first; the channel only remembers where.
//!
//! `projects/<slug>/channels/<id>/channel.yaml`, committed.

use crate::Store;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Channel {
    pub id: String,
    pub label: String,
    /// The connection (extension id) that publishes.
    pub extension: String,
    /// The channel kind the connection declares: `email`, `wordpress`…
    pub kind: String,
    /// Destination fields as the kind declares them: `to`, `subject`, `site`…
    pub destination: BTreeMap<String, Value>,
    /// `user`, or `chat:<id>`.
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    /// Every publish asks the user first. Always true today; kept so it can be loosened per channel later.
    pub asks_first: bool,
    pub publications: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_published: Option<DateTime<Utc>>,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            extension: String::new(),
            kind: String::new(),
            destination: BTreeMap::new(),
            created_by: "user".into(),
            created_at: Utc::now(),
            asks_first: true,
            publications: 0,
            last_published: None,
        }
    }
}

impl Channel {
    pub fn list(store: &Store, slug: &str) -> Result<Vec<Channel>> {
        let dir = store.home().channels_dir(slug);
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let f = e.path().join("channel.yaml");
                if let Ok(Some(c)) = store.read_yaml_opt::<Channel>(&f) {
                    out.push(c);
                }
            }
        }
        out.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
        Ok(out)
    }

    pub fn load(store: &Store, slug: &str, id: &str) -> Result<Channel> {
        if !valid_id(id) {
            bail!("no channel '{id}'");
        }
        match store.read_yaml_opt::<Channel>(&store.home().channels_dir(slug).join(id).join("channel.yaml"))? {
            Some(c) => Ok(c),
            None => bail!("no channel '{id}'"),
        }
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        if !valid_id(&self.id) {
            bail!("a channel id is letters, digits and dashes");
        }
        store.write_yaml(&store.home().channels_dir(slug).join(&self.id).join("channel.yaml"), self)
    }

    pub fn delete(store: &Store, slug: &str, id: &str) -> Result<()> {
        let dir = store.home().channels_dir(slug).join(id);
        if !valid_id(id) || !dir.join("channel.yaml").exists() {
            bail!("no channel '{id}'");
        }
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }

    /// `jane@example.com` for email, else the first destination value.
    pub fn destination_text(&self) -> String {
        let pick = |k: &str| self.destination.get(k).and_then(|v| v.as_str()).map(str::to_string);
        pick("to").or_else(|| pick("site")).or_else(|| pick("url")).or_else(|| pick("account")).or_else(|| self.destination.values().find_map(|v| v.as_str().map(str::to_string))).unwrap_or_default()
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') && !id.starts_with('-')
}

/// A unique id from a label: `email-to-jane`, `email-to-jane-2`.
pub fn new_id(store: &Store, slug: &str, label: &str) -> String {
    let base = crate::assets::slugify(label);
    let base = if base.is_empty() { "channel".to_string() } else { base };
    let dir = store.home().channels_dir(slug);
    let mut id = base.clone();
    let mut n = 2;
    while dir.join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Markdown to simple HTML for email bodies and blog posts.
pub fn markdown_to_html(md: &str) -> String {
    let mut opts = pulldown_cmark::Options::empty();
    opts.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    opts.insert(pulldown_cmark::Options::ENABLE_TABLES);
    let parser = pulldown_cmark::Parser::new_ext(md, opts);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_renders_and_ids_are_safe() {
        let h = markdown_to_html("# Hi\n\nOne *two* [three](https://x.y).\n\n- a\n- b\n");
        assert!(h.contains("<h1>Hi</h1>") && h.contains("<em>two</em>") && h.contains("<li>a</li>"));
        assert!(valid_id("email-to-jane") && !valid_id("Email") && !valid_id("-x"));
        let c = Channel { destination: [("to".to_string(), serde_json::json!("jane@example.com"))].into_iter().collect(), ..Default::default() };
        assert_eq!(c.destination_text(), "jane@example.com");
    }
}
