//! Seemo conversations. One folder per chat under `projects/<slug>/chats/<id>/`:
//! `chat.yaml` (title, when, which model session) and `messages.jsonl`, one
//! JSON message per line. Both are committed after every turn; this transcript
//! is the record. The model's own session is a cache that may expire.

use crate::Store;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chat {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// The model provider's session, resumed turn after turn. `None` until the
    /// first turn, and cleared when a resume fails.
    #[serde(default)]
    pub session_id: Option<String>,
    /// `provider/model` the chat runs on.
    #[serde(default)]
    pub model: String,
    /// The last message's first line, for the chat list.
    #[serde(default)]
    pub last_message: String,
    #[serde(default)]
    pub messages: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The person.
    User,
    /// Seemo's reply.
    Seemo,
    /// An embedded card: an approval, a task.
    Card,
    /// Something the daemon told Seemo, shown small: a run that finished.
    Note,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub at: DateTime<Utc>,
    pub role: Role,
    #[serde(default)]
    pub text: String,
    /// The tools Seemo used while writing this message.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<Card>,
    /// `followup` when the daemon started the turn after a task run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub tool: String,
    pub label: String,
    /// `None` while running.
    #[serde(default)]
    pub ok: Option<bool>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// An image the step produced, as a URL the browser can load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// An asset the step created, so the chat can show it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<StepAsset>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepAsset {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
}

/// A card in the transcript. `kind` is `approval` (a task or config change
/// waiting for a yes) or `task` (a task this chat created, shown live).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Card {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<String>,
    /// For approvals: `task` or `config`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_kind: Option<String>,
    /// `pending`, `approved`, `declined`, `expired`.
    #[serde(default)]
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// What was proposed: the task fields, or the config changes.
    #[serde(default)]
    pub payload: serde_json::Value,
    /// What happened when it was applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

pub fn new_id() -> String {
    let u = uuid::Uuid::new_v4().simple().to_string();
    format!("{}-{}", Utc::now().format("%Y%m%d-%H%M%S"), &u[..6])
}

pub fn message_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..10].to_string()
}

/// A chat title from its first message: the first line, cut at a word.
pub fn title_from(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    let line = line.trim_start_matches(['#', '*', '-', ' ']);
    if line.chars().count() <= 60 {
        return if line.is_empty() { "New chat".into() } else { line.to_string() };
    }
    let cut: String = line.chars().take(60).collect();
    let cut = match cut.rfind(' ') {
        Some(i) if i > 30 => &cut[..i],
        _ => &cut,
    };
    format!("{}…", cut.trim_end_matches([',', '.', ';', ':']))
}

impl Chat {
    pub fn create(store: &Store, slug: &str, title: &str, model: &str) -> Result<Chat> {
        let now = Utc::now();
        let chat = Chat {
            id: new_id(),
            title: if title.trim().is_empty() { "New chat".into() } else { title.trim().to_string() },
            created_at: now,
            updated_at: now,
            session_id: None,
            model: model.to_string(),
            last_message: String::new(),
            messages: 0,
        };
        chat.save(store, slug)?;
        std::fs::write(store.home().chat_dir(slug, &chat.id).join("messages.jsonl"), "")?;
        Ok(chat)
    }

    pub fn load(store: &Store, slug: &str, id: &str) -> Result<Chat> {
        if !valid_id(id) {
            return Err(anyhow!("no chat '{id}'"));
        }
        store.read_yaml_opt(&store.home().chat_dir(slug, id).join("chat.yaml"))?.ok_or_else(|| anyhow!("no chat '{id}'"))
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        store.write_yaml(&store.home().chat_dir(slug, &self.id).join("chat.yaml"), self)
    }

    /// Newest activity first.
    pub fn list(store: &Store, slug: &str) -> Result<Vec<Chat>> {
        let dir = store.home().chats_dir(slug);
        let mut out = Vec::new();
        if dir.is_dir() {
            for e in std::fs::read_dir(dir)? {
                let p = e?.path().join("chat.yaml");
                if p.exists() {
                    match store.read_yaml::<Chat>(&p) {
                        Ok(c) => out.push(c),
                        Err(e) => tracing::warn!(path = %p.display(), error = %e, "skipping unreadable chat"),
                    }
                }
            }
        }
        out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(out)
    }

    pub fn delete(store: &Store, slug: &str, id: &str) -> Result<()> {
        let _ = Chat::load(store, slug, id)?;
        std::fs::remove_dir_all(store.home().chat_dir(slug, id))?;
        Ok(())
    }

    pub fn messages(&self, store: &Store, slug: &str) -> Result<Vec<Message>> {
        messages(store, slug, &self.id)
    }

    /// Append one message and update the chat's summary fields.
    pub fn append(&mut self, store: &Store, slug: &str, m: &Message) -> Result<()> {
        let path = store.home().chat_dir(slug, &self.id).join("messages.jsonl");
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
        writeln!(f, "{}", serde_json::to_string(m)?)?;
        self.messages += 1;
        self.touch(m);
        self.save(store, slug)
    }

    /// Replace a message by id (a card whose status moved on). Rewrites the file.
    pub fn replace(&mut self, store: &Store, slug: &str, m: &Message) -> Result<()> {
        let mut all = self.messages(store, slug)?;
        match all.iter_mut().find(|x| x.id == m.id) {
            Some(slot) => *slot = m.clone(),
            None => all.push(m.clone()),
        }
        let text: String = all.iter().filter_map(|x| serde_json::to_string(x).ok()).map(|l| l + "\n").collect();
        store.write_text(&store.home().chat_dir(slug, &self.id).join("messages.jsonl"), &text)?;
        self.messages = all.len();
        self.touch(m);
        self.save(store, slug)
    }

    fn touch(&mut self, m: &Message) {
        self.updated_at = Utc::now();
        if m.role == Role::User || m.role == Role::Seemo {
            if let Some(line) = m.text.lines().find(|l| !l.trim().is_empty()) {
                self.last_message = crate::profile::truncate_chars(line.trim(), 120);
            }
        }
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn messages(store: &Store, slug: &str, id: &str) -> Result<Vec<Message>> {
    let path = store.home().chat_dir(slug, id).join("messages.jsonl");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    Ok(text.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect())
}

/// The conversation so far as plain text, newest last, within `max_chars`,
/// for starting a fresh model session when the old one is gone.
pub fn transcript(messages: &[Message], max_chars: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    for m in messages {
        let who = match m.role {
            Role::User => "User",
            Role::Seemo => "Seemo",
            Role::Note => "Note",
            Role::Card => continue,
        };
        if m.text.trim().is_empty() {
            continue;
        }
        lines.push(format!("{who}: {}", m.text.trim()));
    }
    let mut out = String::new();
    for l in lines.iter().rev() {
        if out.len() + l.len() + 2 > max_chars {
            break;
        }
        out = if out.is_empty() { l.clone() } else { format!("{l}\n\n{out}") };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_short_and_whole_words() {
        assert_eq!(title_from("Hello"), "Hello");
        assert_eq!(title_from("  \n# Write me three posts about teeth whitening"), "Write me three posts about teeth whitening");
        let long = title_from("Please write three social media posts for the library as draft post assets, following the instructions for social media posts, and tell me when they are in");
        assert!(long.ends_with('…'));
        assert!(long.chars().count() <= 61);
        assert!(!long.contains("assets,"));
        assert_eq!(title_from("   "), "New chat");
    }

    #[test]
    fn transcript_keeps_the_newest() {
        let m = |role, text: &str| Message { id: message_id(), at: Utc::now(), role, text: text.into(), steps: vec![], card: None, kind: None, error: None };
        let msgs = vec![m(Role::User, "one"), m(Role::Seemo, "two"), m(Role::Card, ""), m(Role::User, "three")];
        let t = transcript(&msgs, 1000);
        assert_eq!(t, "User: one\n\nSeemo: two\n\nUser: three");
        assert_eq!(transcript(&msgs, 14), "User: three");
    }
}
