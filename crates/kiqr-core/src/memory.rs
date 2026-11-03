//! What Seemo remembers about how this business wants things done:
//! `projects/<slug>/memory.yaml`. The owner's rules and preferences (said in
//! a chat or written on Seemo → Memory) and what Seemo learned from earlier
//! work (lessons, facts). Every chat and task run gets them in its prompt;
//! the owner's under their own heading, so a lesson never reads as an order.
//! A lesson from a task run waits (`pending`) until the owner keeps it: a run
//! reads pages and threads, and nothing they say becomes standing guidance
//! without a person's yes.

use crate::store::Store;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// At most this many; past it the oldest of Seemo's lessons goes first.
pub const MAX: usize = 80;
/// One sentence.
pub const MAX_TEXT: usize = 300;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// How things must or must not be done ("help, never promote").
    #[default]
    Rule,
    /// How the owner likes things ("short posts, no hashtags").
    Preference,
    /// Something true about the business the brief does not say.
    Fact,
    /// What an outcome taught ("r/webscraping removes posts with links").
    Lesson,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Rule => "rule",
            Kind::Preference => "preference",
            Kind::Fact => "fact",
            Kind::Lesson => "lesson",
        }
    }
}

/// Where a memory applies, by channel ownership: promoting is right on the
/// business's own profiles and wrong in someone else's community.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    #[default]
    Everywhere,
    /// The business's own channels: its profiles, pages, blog, lists.
    Own,
    /// Other people's places: replies in threads, communities, groups.
    Others,
}

impl Scope {
    fn note(self) -> &'static str {
        match self {
            Scope::Everywhere => "",
            Scope::Own => ", on the business's own channels",
            Scope::Others => ", in other people's threads, communities and groups",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Memory {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "is_everywhere")]
    pub scope: Scope,
    /// `user` (the owner's words, or edited by them) or `seemo` (learned).
    pub by: String,
    /// `chat:<id>` or `run:<task>/<run>`, where it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// A task run's lesson the owner has not kept yet: not in any prompt.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pending: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn is_everywhere(s: &Scope) -> bool {
    *s == Scope::Everywhere
}

impl Memory {
    pub fn is_users(&self) -> bool {
        self.by == "user"
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Memories {
    pub items: Vec<Memory>,
    /// The owner switched learning off: no reflection after chats and runs
    /// (`remember` and the page still work).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub learning_off: bool,
}

fn path(store: &Store, slug: &str) -> std::path::PathBuf {
    store.home().project_dir(slug).join("memory.yaml")
}

pub fn load(store: &Store, slug: &str) -> Result<Memories> {
    Ok(store.read_yaml_opt(&path(store, slug))?.unwrap_or_default())
}

pub fn save(store: &Store, slug: &str, m: &Memories) -> Result<()> {
    store.write_yaml(&path(store, slug), m)
}

pub fn new_id() -> String {
    let u = uuid::Uuid::new_v4().simple().to_string();
    format!("m_{}", &u[..10])
}

/// The text as kept: one line, trimmed. Refused when empty, too long, or
/// when it looks like it holds a secret.
pub fn clean(text: &str) -> Result<String> {
    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        bail!("say what to remember");
    }
    if t.chars().count() > MAX_TEXT {
        bail!("keep it to one sentence ({MAX_TEXT} characters at most)");
    }
    if looks_secret(&t) {
        bail!("that looks like it holds a password or a key; those go in the Vault, never in memory");
    }
    Ok(t)
}

/// A password, a key or a token: a long run of letters and digits with no
/// spaces, a known key prefix, or `password:` / `key=` with a value.
pub fn looks_secret(t: &str) -> bool {
    let lower = t.to_lowercase();
    for word in t.split_whitespace() {
        let w = word.trim_matches(|c: char| !c.is_alphanumeric());
        if w.len() >= 24 && w.chars().any(|c| c.is_ascii_digit()) && w.chars().any(|c| c.is_ascii_alphabetic()) && !w.contains('.') && !w.contains('/') {
            return true;
        }
        if ["sk-", "sk_", "ghp_", "xox", "pk_live", "rk_live", "akia"].iter().any(|p| w.to_lowercase().starts_with(p)) && w.len() >= 12 {
            return true;
        }
    }
    ["password:", "password=", "password is", "passwd", "api key:", "api_key=", "key=", "token:", "token=", "secret:", "secret="].iter().any(|p| lower.contains(p))
}

/// For finding the same thing said again: lowercase letters and digits only.
fn norm(t: &str) -> String {
    t.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

impl Memories {
    pub fn get(&self, id: &str) -> Option<&Memory> {
        self.items.iter().find(|m| m.id == id)
    }

    /// The same text already there.
    pub fn same(&self, text: &str) -> Option<&Memory> {
        let n = norm(text);
        self.items.iter().find(|m| norm(&m.text) == n)
    }

    /// Adds it, unless the same is there already (then that one comes back).
    /// When full, the oldest of Seemo's lessons, pending first, makes room;
    /// the owner's are never dropped, and with only theirs left it is refused.
    pub fn add(&mut self, text: &str, kind: Kind, scope: Scope, by: &str, from: Option<String>, pending: bool) -> Result<(Memory, bool)> {
        let text = clean(text)?;
        if let Some(m) = self.same(&text) {
            return Ok((m.clone(), false));
        }
        while self.items.len() >= MAX {
            let drop = self.items.iter().enumerate().filter(|(_, m)| !m.is_users() && m.pending).min_by_key(|(_, m)| m.created_at).or_else(|| self.items.iter().enumerate().filter(|(_, m)| !m.is_users()).min_by_key(|(_, m)| (m.kind != Kind::Lesson, m.created_at)));
            match drop.map(|(i, _)| i) {
                Some(i) => {
                    self.items.remove(i);
                }
                None => bail!("memory is full ({MAX}); remove one on Seemo → Memory first"),
            }
        }
        let now = Utc::now();
        let m = Memory { id: new_id(), text, kind, scope, by: by.to_string(), from, pending, created_at: now, updated_at: now };
        self.items.push(m.clone());
        Ok((m, true))
    }

    /// The owner's change: it is theirs from now on, and kept.
    pub fn update(&mut self, id: &str, text: Option<&str>, kind: Option<Kind>, scope: Option<Scope>) -> Result<Memory> {
        let text = text.map(clean).transpose()?;
        let m = self.items.iter_mut().find(|m| m.id == id).ok_or_else(|| anyhow::anyhow!("no such memory"))?;
        if let Some(t) = text {
            m.text = t;
        }
        if let Some(k) = kind {
            m.kind = k;
        }
        if let Some(sc) = scope {
            m.scope = sc;
        }
        m.by = "user".into();
        m.pending = false;
        m.updated_at = Utc::now();
        Ok(m.clone())
    }

    pub fn keep(&mut self, id: &str) -> Result<Memory> {
        let m = self.items.iter_mut().find(|m| m.id == id).ok_or_else(|| anyhow::anyhow!("no such memory"))?;
        m.pending = false;
        m.updated_at = Utc::now();
        Ok(m.clone())
    }

    pub fn remove(&mut self, id: &str) -> Option<Memory> {
        let i = self.items.iter().position(|m| m.id == id)?;
        Some(self.items.remove(i))
    }

    /// For the prompt: (the owner's, what Seemo learned), each line with
    /// its kind; pending ones left out. Sorted rule, preference, fact, lesson.
    pub fn lines(&self) -> (Vec<String>, Vec<String>) {
        let mut items: Vec<&Memory> = self.items.iter().filter(|m| !m.pending).collect();
        items.sort_by_key(|m| (m.kind, m.created_at));
        let line = |m: &&Memory| format!("({}{}) {}", m.kind.label(), m.scope.note(), m.text);
        (items.iter().filter(|m| m.is_users()).map(line).collect(), items.iter().filter(|m| !m.is_users()).map(line).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_thing_said_again_is_kept_once() {
        let mut m = Memories::default();
        let (a, new) = m.add("Help people, never promote.", Kind::Rule, Scope::Everywhere, "user", None, false).unwrap();
        assert!(new);
        let (b, new) = m.add("help people   never promote", Kind::Rule, Scope::Everywhere, "user", None, false).unwrap();
        assert!(!new);
        assert_eq!(a.id, b.id);
        assert_eq!(m.items.len(), 1);
    }

    #[test]
    fn secrets_and_essays_are_refused() {
        assert!(clean("my password is hunter2").is_err());
        assert!(clean("use sk-ant-api03-abcdefghijkl for the API").is_err());
        assert!(clean("the token aB3dE5fG7hI9jK1lM3nO5pQ7rS9 works").is_err());
        assert!(clean(&"word ".repeat(80)).is_err());
        assert!(clean("  ").is_err());
        assert!(clean("Link to unblockingapi.com/pricing only when someone asks about price").is_ok());
        assert!(clean("Write X posts under 260 characters").is_ok());
    }

    #[test]
    fn when_full_seemos_oldest_lesson_goes_and_the_owners_never_do() {
        let mut m = Memories::default();
        m.add("lesson zero", Kind::Lesson, Scope::Everywhere, "seemo", None, false).unwrap();
        for i in 1..MAX {
            m.add(&format!("rule number {i}"), Kind::Rule, Scope::Everywhere, "user", None, false).unwrap();
        }
        m.add("a new rule", Kind::Rule, Scope::Everywhere, "user", None, false).unwrap();
        assert_eq!(m.items.len(), MAX);
        assert!(m.items.iter().all(|x| x.is_users()));
        assert!(m.add("one more", Kind::Rule, Scope::Everywhere, "user", None, false).is_err());
    }

    #[test]
    fn pending_lessons_stay_out_of_the_prompt_and_the_owners_come_apart() {
        let mut m = Memories::default();
        m.add("a lesson from a thread", Kind::Lesson, Scope::Everywhere, "seemo", Some("run:t/r".into()), true).unwrap();
        m.add("a lesson from a chat", Kind::Lesson, Scope::Everywhere, "seemo", Some("chat:c".into()), false).unwrap();
        let (r, _) = m.add("help, never promote", Kind::Rule, Scope::Everywhere, "user", None, false).unwrap();
        let (owner, learned) = m.lines();
        assert_eq!(owner, vec!["(rule) help, never promote".to_string()]);
        assert_eq!(learned, vec!["(lesson) a lesson from a chat".to_string()]);
        // Kept, it counts; edited by the owner, it is theirs.
        let id = m.items[0].id.clone();
        m.keep(&id).unwrap();
        assert_eq!(m.lines().1.len(), 2);
        m.update(&id, Some("a lesson, reworded"), None, None).unwrap();
        assert!(m.get(&id).unwrap().is_users());
        assert!(m.get(&r.id).unwrap().is_users());
    }

    #[test]
    fn a_rule_says_where_it_applies() {
        let mut m = Memories::default();
        m.add("Never promote; just help", Kind::Rule, Scope::Others, "user", None, false).unwrap();
        m.add("Every post ends with a link to the site", Kind::Rule, Scope::Own, "user", None, false).unwrap();
        let (owner, _) = m.lines();
        assert_eq!(owner[0], "(rule, in other people's threads, communities and groups) Never promote; just help");
        assert_eq!(owner[1], "(rule, on the business's own channels) Every post ends with a link to the site");
    }
}
