//! Lists: the people a project emails. A list is a project's own
//! (`projects/<slug>/lists/<id>/`): `list.yaml` (its name, committed) and
//! `contacts.csv` (the people, never committed: personal data stays on this
//! machine and out of history). A mail account sends to a list through a
//! place of kind `list`; the daemon sends one message per contact.

use crate::paths::Home;
use crate::Store;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct List {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

impl Default for List {
    fn default() -> Self {
        Self { id: String::new(), label: String::new(), description: String::new(), created_by: "user".into(), created_at: Utc::now() }
    }
}

/// One person on a list. `consent` says how they agreed, in the user's words
/// ("signed up on the site", "customer since 2024"); `unsubscribed` set means
/// they left and are never written to again.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Contact {
    pub email: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub consent: String,
    #[serde(default)]
    pub added: Option<DateTime<Utc>>,
    #[serde(default)]
    pub unsubscribed: Option<DateTime<Utc>>,
}

impl Contact {
    pub fn active(&self) -> bool {
        self.unsubscribed.is_none()
    }
}

/// What the UI and Seemo see of a list: the record plus the counts, never the addresses.
#[derive(Clone, Debug, Serialize)]
pub struct ListView {
    #[serde(flatten)]
    pub list: List,
    pub contacts: usize,
    pub unsubscribed: usize,
}

pub fn dir(home: &Home, slug: &str, id: &str) -> PathBuf {
    home.lists_dir(slug).join(id)
}

fn contacts_file(home: &Home, slug: &str, id: &str) -> PathBuf {
    dir(home, slug, id).join("contacts.csv")
}

impl List {
    pub fn all(store: &Store, slug: &str) -> Result<Vec<List>> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(store.home().lists_dir(slug)) {
            for e in rd.flatten() {
                if let Ok(Some(l)) = store.read_yaml_opt::<List>(&e.path().join("list.yaml")) {
                    out.push(l);
                }
            }
        }
        out.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
        Ok(out)
    }

    pub fn load(store: &Store, slug: &str, id: &str) -> Result<List> {
        if !crate::connections::valid_id(id) {
            bail!("no list '{id}'");
        }
        match store.read_yaml_opt::<List>(&dir(store.home(), slug, id).join("list.yaml"))? {
            Some(l) => Ok(l),
            None => bail!("no list '{id}'"),
        }
    }

    /// By id or name, any case.
    pub fn find(store: &Store, slug: &str, name: &str) -> Option<List> {
        let n = name.trim();
        Self::all(store, slug).unwrap_or_default().into_iter().find(|l| l.id == n || l.label.eq_ignore_ascii_case(n))
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        if !crate::connections::valid_id(&self.id) {
            bail!("a list id is letters, digits and dashes");
        }
        store.write_yaml(&dir(store.home(), slug, &self.id).join("list.yaml"), self)
    }

    /// Remove the list and everyone on it.
    pub fn delete(store: &Store, slug: &str, id: &str) -> Result<List> {
        let l = Self::load(store, slug, id)?;
        std::fs::remove_dir_all(dir(store.home(), slug, id))?;
        Ok(l)
    }

    /// A unique id from a label: `newsletter`, `newsletter-2`.
    pub fn new_id(store: &Store, slug: &str, label: &str) -> String {
        let base = crate::assets::slugify(label);
        let base = if base.is_empty() { "list".to_string() } else { base };
        let root = store.home().lists_dir(slug);
        let mut id = base.clone();
        let mut n = 2;
        while root.join(&id).exists() {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }

    pub fn view(&self, home: &Home, slug: &str) -> ListView {
        let all = contacts(home, slug, &self.id);
        let unsubscribed = all.iter().filter(|c| !c.active()).count();
        ListView { list: self.clone(), contacts: all.len() - unsubscribed, unsubscribed }
    }
}

/// Everyone on the list, in the order they were added.
pub fn contacts(home: &Home, slug: &str, id: &str) -> Vec<Contact> {
    let text = std::fs::read_to_string(contacts_file(home, slug, id)).unwrap_or_default();
    parse_csv(&text)
}

/// The people a send goes to.
pub fn recipients(home: &Home, slug: &str, id: &str) -> Vec<Contact> {
    contacts(home, slug, id).into_iter().filter(Contact::active).collect()
}

fn write_contacts(home: &Home, slug: &str, id: &str, all: &[Contact]) -> Result<()> {
    let f = contacts_file(home, slug, id);
    if let Some(d) = f.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut out = String::from("email,name,consent,added,unsubscribed\n");
    for c in all {
        out.push_str(&format!("{},{},{},{},{}\n", quote(&c.email), quote(&c.name), quote(&c.consent), c.added.map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)).unwrap_or_default(), c.unsubscribed.map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)).unwrap_or_default()));
    }
    crate::secrets::write_private(&f, &out)
}

/// Add people. An address already on the list keeps its row (and its
/// unsubscribe); the name and consent fill in when they were empty. Returns
/// how many were new.
pub fn add(home: &Home, slug: &str, id: &str, new: Vec<Contact>) -> Result<usize> {
    let mut all = contacts(home, slug, id);
    let mut added = 0;
    for mut c in new {
        c.email = c.email.trim().to_ascii_lowercase();
        if !valid_email(&c.email) {
            continue;
        }
        match all.iter_mut().find(|o| o.email == c.email) {
            Some(o) => {
                if o.name.is_empty() {
                    o.name = c.name.trim().to_string();
                }
                if o.consent.is_empty() {
                    o.consent = c.consent.trim().to_string();
                }
            }
            None => {
                c.name = c.name.trim().to_string();
                c.consent = c.consent.trim().to_string();
                c.added = c.added.or_else(|| Some(Utc::now()));
                all.push(c);
                added += 1;
            }
        }
    }
    write_contacts(home, slug, id, &all)?;
    Ok(added)
}

/// Someone left: they stay on file, marked, and are never written to again.
pub fn unsubscribe(home: &Home, slug: &str, id: &str, email: &str) -> Result<bool> {
    let mut all = contacts(home, slug, id);
    let e = email.trim().to_ascii_lowercase();
    let Some(c) = all.iter_mut().find(|c| c.email == e) else { return Ok(false) };
    if c.unsubscribed.is_none() {
        c.unsubscribed = Some(Utc::now());
    }
    write_contacts(home, slug, id, &all)?;
    Ok(true)
}

/// Take someone off the list entirely (their row goes).
pub fn remove(home: &Home, slug: &str, id: &str, email: &str) -> Result<bool> {
    let mut all = contacts(home, slug, id);
    let e = email.trim().to_ascii_lowercase();
    let before = all.len();
    all.retain(|c| c.email != e);
    write_contacts(home, slug, id, &all)?;
    Ok(all.len() != before)
}

/// People from a CSV the user brought (an export from a shop or a mail
/// tool): the `email` column (or the first column holding addresses) and a
/// `name` column when there is one. `consent` is what the user says about
/// the whole file.
pub fn from_csv(text: &str, consent: &str) -> Vec<Contact> {
    let rows = split_csv(text);
    let Some(first) = rows.first() else { return vec![] };
    let header: Vec<String> = first.iter().map(|h| h.trim().to_ascii_lowercase()).collect();
    let has_header = !header.iter().any(|h| valid_email(h));
    let col = |names: &[&str]| header.iter().position(|h| names.iter().any(|n| h == n || h.contains(n)));
    let email_col = if has_header { col(&["email", "e-mail", "mail"]) } else { None };
    let name_col = if has_header { col(&["name", "first", "full"]).filter(|c| Some(*c) != email_col) } else { None };
    let mut out = Vec::new();
    for row in rows.iter().skip(if has_header { 1 } else { 0 }) {
        let email = match email_col {
            Some(c) => row.get(c).cloned().unwrap_or_default(),
            None => row.iter().find(|v| valid_email(v.trim())).cloned().unwrap_or_default(),
        };
        let email = email.trim().to_ascii_lowercase();
        if !valid_email(&email) {
            continue;
        }
        let name = name_col.and_then(|c| row.get(c)).cloned().unwrap_or_default();
        out.push(Contact { email, name: name.trim().to_string(), consent: consent.trim().to_string(), added: None, unsubscribed: None });
    }
    out
}

pub fn valid_email(s: &str) -> bool {
    let s = s.trim();
    let Some((user, host)) = s.rsplit_once('@') else { return false };
    !user.is_empty() && host.contains('.') && !host.starts_with('.') && !host.ends_with('.') && !s.chars().any(|c| c.is_whitespace() || c == ',' || c == ';')
}

fn parse_csv(text: &str) -> Vec<Contact> {
    let rows = split_csv(text);
    let mut it = rows.into_iter();
    let Some(header) = it.next() else { return vec![] };
    let idx = |n: &str| header.iter().position(|h| h.trim() == n);
    let (ie, iname, ic, ia, iu) = (idx("email"), idx("name"), idx("consent"), idx("added"), idx("unsubscribed"));
    let get = |row: &[String], i: Option<usize>| i.and_then(|i| row.get(i)).map(|s| s.trim().to_string()).unwrap_or_default();
    let when = |s: String| DateTime::parse_from_rfc3339(&s).ok().map(|t| t.with_timezone(&Utc));
    it.filter_map(|row| {
        let email = get(&row, ie).to_ascii_lowercase();
        valid_email(&email).then(|| Contact { email, name: get(&row, iname), consent: get(&row, ic), added: when(get(&row, ia)), unsubscribed: when(get(&row, iu)) })
    })
    .collect()
}

/// RFC 4180 rows: quoted fields, doubled quotes, any line ending.
fn split_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            }
            ('"', false) if field.is_empty() => quoted = true,
            (',' | ';' | '\t', false) => row.push(std::mem::take(&mut field)),
            ('\r', false) => {}
            ('\n', false) => {
                row.push(std::mem::take(&mut field));
                if row.iter().any(|f| !f.trim().is_empty()) {
                    rows.push(std::mem::take(&mut row));
                } else {
                    row.clear();
                }
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        if row.iter().any(|f| !f.trim().is_empty()) {
            rows.push(row);
        }
    }
    rows
}

fn quote(s: &str) -> String {
    if s.contains([',', '"', '\n', ';']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_hold_people_outside_git() {
        let dir = std::env::temp_dir().join(format!("kiqr-lists-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open(Home::at(&dir)).unwrap();
        let home = store.home().clone();
        let l = List { id: List::new_id(&store, "acme", "Newsletter"), label: "Newsletter".into(), ..Default::default() };
        assert_eq!(l.id, "newsletter");
        l.save(&store, "acme").unwrap();
        let people = from_csv("Email,Full name\njane@example.com,Jane Doe\n\"bob@example.com\",\"Bob, Jr.\"\nnot-an-address,X\n", "signed up on the site");
        assert_eq!(people.len(), 2);
        assert_eq!(add(&home, "acme", "newsletter", people.clone()).unwrap(), 2);
        assert_eq!(add(&home, "acme", "newsletter", people).unwrap(), 0, "the same address twice is one person");
        assert_eq!(contacts(&home, "acme", "newsletter")[1].name, "Bob, Jr.");
        assert!(unsubscribe(&home, "acme", "newsletter", "JANE@example.com").unwrap());
        assert_eq!(recipients(&home, "acme", "newsletter").len(), 1);
        let v = l.view(&home, "acme");
        assert_eq!((v.contacts, v.unsubscribed), (1, 1));
        assert_eq!(from_csv("a@b.co\nc@d.org\n", "").len(), 2, "no header: addresses alone");
        assert!(List::find(&store, "acme", "NEWSLETTER").is_some());
        assert!(remove(&home, "acme", "newsletter", "bob@example.com").unwrap());
        List::delete(&store, "acme", "newsletter").unwrap();
        assert!(List::all(&store, "acme").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
