//! Abilities on disk: one folder, `abilities/<id>/` (the machine's, or a
//! project's own), holding `SKILL.md` (instructions: how Seemo does
//! something, in words the user can edit), `extension.yaml` (a connection:
//! what provides an ability contract, a few tools of its own, or a channel
//! kind), or both (e.g. "Publish to WordPress"). The user sees one thing, an
//! ability; the code calls the manifest side an extension.
//!
//! The manifest says what the connection needs (keys, by variable name),
//! which hosts it may call, which abilities it provides and how (its
//! transport: an HTTP call, a local command, an MCP server, or built-in Rust).
//! This module parses and validates; the daemon executes.

use crate::paths::Home;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    /// Declarative HTTP: the daemon makes the request described in `request`.
    Http,
    /// A local program the daemon runs with the call's arguments.
    Command,
    /// An MCP server the daemon hosts and proxies.
    Mcp,
    /// Mail over SMTP, with the server settings the manifest's `smtp` block names.
    Smtp,
    /// Rust inside KIQR: the core renderer (`kiqr-render`).
    Builtin,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthKey {
    /// Environment variable name; the value lives in `.env`.
    pub env: String,
    pub label: String,
    /// Where to get it, or what it is, shown under the field.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub help: String,
    /// Shown greyed inside the empty field: what a value looks like.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub placeholder: String,
    /// Optional: a key the connection works without.
    pub optional: bool,
    /// The values it can take when it is a choice (`starttls`, `tls`, `none`):
    /// the UI shows a list, and anything else is refused.
    #[serde(alias = "enum", skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    /// Values read from the service once the account is signed in (the
    /// properties it can see): the UI offers them as a list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<Choices>,
    /// A token a sign-in keeps that may also be typed (a personal access
    /// token instead of signing in): shown as a field like any key.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub typed: bool,
}

/// Where a key's choices come from: a request through the account, the list
/// in its answer, and in each item the value and what to call it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Choices {
    pub request: HttpRequest,
    /// A pointer to the items; `*` steps into every element of an array.
    pub list: String,
    /// In each item: the value saved, and the label shown (the value when empty).
    pub value: String,
    pub label: String,
    /// Taken off the front of the value (`properties/` of `properties/123`).
    pub strip: String,
    /// Read up to this many pages (at most 20), the request naming `{{page}}`
    /// (1, 2, …); it stops at the first page with no items.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages: Option<u32>,
}

impl Choices {
    pub fn pages(&self) -> u32 {
        self.pages.unwrap_or(1).clamp(1, 20)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    /// JSON body; strings may hold `{{arg}}` and `${ENV}` placeholders.
    pub json: Option<Value>,
    /// A URL-encoded form body: field → template.
    pub form: BTreeMap<String, String>,
    /// A multipart body: parts with a value or a file.
    pub multipart: Vec<Part>,
    /// One file's bytes as the whole body (`{{file}}`, `{{files.0}}`); its
    /// type becomes the Content-Type unless a header says otherwise.
    pub body_file: Option<String>,
    pub query: BTreeMap<String, String>,
    /// HTTP basic auth: `["${USER}", "${PASSWORD}"]`, filled at call time.
    pub basic_auth: Option<Vec<String>>,
    pub timeout_s: Option<u64>,
    /// Poll this URL (taken from the first response by JSON pointer) until
    /// `poll_until` (pointer) equals `poll_value`; for slow generators.
    pub poll_url: Option<String>,
    pub poll_until: Option<String>,
    pub poll_value: Option<String>,
    pub poll_every_s: Option<u64>,
}

/// A mail server, for `transport: smtp`. Each value is a template, usually
/// `${VAR}` from `.env`; a connection for one mail host can write its host
/// and port as they are.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SmtpSpec {
    pub host: String,
    /// Empty: 465 with `tls`, else 587.
    pub port: String,
    pub user: String,
    /// Empty: a relay that needs no password (on this machine or a private network).
    pub password: String,
    /// What recipients see; the user name when empty.
    pub from: String,
    /// `starttls` (default), `tls`, or `none` for a relay on this machine only.
    pub security: String,
}

/// One request of a tool that takes several: upload a file, then create the
/// post with what the upload answered. What a step `save`s is there for the
/// next ones as `{{name.var}}` (a list, `{{name.0.var}}`, after `for_each`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Step {
    pub name: String,
    pub request: HttpRequest,
    /// On a hosted program (`transport: mcp`): the server tool this step calls, with
    /// `args` as a template over the arguments and what earlier steps saved.
    pub call: String,
    pub args: Option<Value>,
    /// var → pointer into this step's answer. Empty: the whole answer.
    pub save: BTreeMap<String, String>,
    /// Run only when this fills to something (not empty, not `[]`, not false).
    pub when: Option<String>,
    /// `files`: once per file of the asset; or an earlier step's name: once
    /// per entry of its list. Each run has `{{item}}` (the entry), `{{file}}`
    /// (the file at the same position) and `{{index}}`.
    pub for_each: Option<String>,
}

/// At most this many steps, and this many runs of one `for_each`.
pub const MAX_STEPS: usize = 10;
pub const MAX_EACH: usize = 20;

/// One part of a multipart body: `value` (a template) or `file` (a file
/// reference such as `{{file}}`), with an optional file name and type.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Part {
    pub name: String,
    pub value: Option<String>,
    pub file: Option<String>,
    pub filename: Option<String>,
    pub mime: Option<String>,
}

impl HttpRequest {
    /// How many body kinds it sets; more than one is refused.
    pub fn body_kinds(&self) -> usize {
        [self.json.is_some(), !self.form.is_empty(), !self.multipart.is_empty(), self.body_file.is_some()].iter().filter(|b| **b).count()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandSpec {
    pub command: String,
    pub args: Vec<String>,
    /// `json`: the arguments as one JSON object on stdin. Empty: nothing.
    pub stdin: String,
    /// Environment variable names passed through from `.env`.
    pub env: Vec<String>,
    pub cwd: Option<String>,
    pub timeout_s: Option<u64>,
}

/// How a result is read: JSON pointers into the response.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ResultMap {
    /// A URL to download as the file.
    pub file_url: Option<String>,
    /// Base64 content of the file.
    pub file_b64: Option<String>,
    /// Text worth telling the model (a caption, a revised prompt).
    pub text: Option<String>,
    /// The file's media type when the response does not say.
    pub mime: Option<String>,
}

/// One way of doing something: an ability's implementation or an extra tool.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Provide {
    pub cost: Option<String>,
    pub request: Option<HttpRequest>,
    /// Several requests in order, instead of one `request` (http only).
    pub steps: Vec<Step>,
    pub command: Option<CommandSpec>,
    /// MCP: the server's tool to call, with an argument template.
    pub tool: Option<String>,
    pub args: Option<Value>,
    pub result: ResultMap,
    /// For extra tools: the result is a file to put on the asset in `asset`.
    pub attach: Option<bool>,
    /// Replace an argument's value before templating: `orientation: {landscape: 1536x1024, "*": 1024x1024}`.
    pub map: BTreeMap<String, BTreeMap<String, Value>>,
}

impl Provide {
    /// The arguments with `map` applied.
    pub fn mapped_args(&self, args: &Value) -> Value {
        let mut a = args.clone();
        if let Some(obj) = a.as_object_mut() {
            for (arg, table) in &self.map {
                let current = obj.get(arg).and_then(|v| v.as_str().map(str::to_string).or_else(|| Some(v.to_string()))).unwrap_or_default();
                if let Some(v) = table.get(&current).or_else(|| table.get("*")) {
                    obj.insert(arg.clone(), v.clone());
                }
            }
        }
        a
    }
}

/// A tool outside any ability contract, offered under the connection's own group.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtraTool {
    pub name: String,
    pub label: String,
    pub description: String,
    pub input: Value,
    /// What a call does on its account (`ROLES`): anything but `read` is an
    /// action, logged per account and counted against `daily_cap`. Empty:
    /// `read`, or `publish` for a channel kind's publish tool.
    pub role: String,
    /// The thing acted on, as a template (`"{{post_url}}"`): the same action
    /// on the same target through one account is refused the second time.
    pub target: String,
    #[serde(flatten)]
    pub provide: Provide,
}

/// Somewhere content goes out on a service: a social network, a blog, an
/// inbox. Everything KIQR knows about one (its name, the text limit, the image
/// size, how a post looks there, what names a spot on it) comes from the
/// ability that declares it, never from KIQR's code. Where exactly a post goes
/// is an Audience entry of the kind (`crate::audience`), never the account.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Kind {
    pub kind: String,
    /// What one spot of the kind is called ("Subreddit", "Page", "Blog").
    pub label: String,
    /// Where an Audience entry of the kind is listed: `page` (the business's
    /// own: a profile, a page, a blog), `community` (somewhere it takes part:
    /// a subreddit, a group) or `list` (people it emails). Default `page`.
    pub section: String,
    /// The fields that name a spot (a subreddit): name → {label, help,
    /// placeholder, optional, enum, pattern}. A publish there carries them as
    /// arguments; an Audience entry holds them. Empty: the account is the spot.
    pub target: BTreeMap<String, Value>,
    /// How a URL argument of a call names the spot: arg → a pattern with the
    /// target's fields in braces (`post_url: "reddit.com/r/{subreddit}/"`), so a
    /// reply to a post is known to be in r/selfhosted.
    pub target_from: BTreeMap<String, String>,
    /// The account itself is the one spot (an X profile): connecting it makes
    /// its Audience entry.
    pub account_is_target: bool,
    /// A link to a spot, a template over the target's fields.
    pub url: String,
    /// The tool, or the ability, that performs a publish. Empty: nothing publishes as this kind.
    pub publish: String,
    /// The network's name as people write it ("LinkedIn"); the label when empty.
    pub name: String,
    /// A lucide icon name.
    pub icon: String,
    /// Characters a post may have there.
    pub text_limit: Option<u32>,
    /// The image size a post there gets.
    pub image: Option<ImageSize>,
    /// How a post is previewed: `feed` (a social feed card), else plain.
    pub preview: String,
    /// How a post there is written, in a line or two ("600 to 1,300
    /// characters, short paragraphs, at most three hashtags"); the posts job
    /// passes it on with the network.
    pub writing: String,
}

/// What a tool may do on an account.
pub const ROLES: &[&str] = &["publish", "read", "reply", "manage"];
/// Where an Audience entry is listed.
pub const SECTIONS: &[&str] = &["page", "community", "list"];

impl ExtraTool {
    /// Does more than read: Seemo's call asks the user first.
    pub fn acts(&self) -> bool {
        !self.role.is_empty() && self.role != "read"
    }
}

impl Kind {
    pub fn publishes(&self) -> bool {
        !self.publish.is_empty()
    }

    /// `section`, else `page`.
    pub fn section(&self) -> &str {
        if self.section.is_empty() { "page" } else { &self.section }
    }

    pub fn display_name(&self) -> &str {
        if !self.name.is_empty() {
            &self.name
        } else if !self.label.is_empty() {
            &self.label
        } else {
            &self.kind
        }
    }
    /// A social network (as opposed to an inbox or a blog): somewhere posts go.
    pub fn is_network(&self) -> bool {
        self.text_limit.is_some() || self.preview == "feed"
    }

    /// The target fields a spot must have.
    pub fn required(&self) -> Vec<&str> {
        self.target.iter().filter(|(_, s)| !s["optional"].as_bool().unwrap_or(false)).map(|(n, _)| n.as_str()).collect()
    }

    /// The target fields a URL names, through `target_from` (`{field}` in the pattern matches up to the next `/`, `?` or `#`).
    pub fn target_of_url(&self, arg: &str, url: &str) -> Option<BTreeMap<String, Value>> {
        let pat = self.target_from.get(arg)?;
        let mut out = BTreeMap::new();
        // The pattern's literal start is found anywhere in the URL (scheme and www. vary).
        let first = pat.find('{').unwrap_or(pat.len());
        let at = url.to_ascii_lowercase().find(&pat[..first].to_ascii_lowercase())?;
        let mut rest = &url[at + first..];
        let mut p = &pat[first..];
        while let Some(open) = p.find('{') {
            let lit = &p[..open];
            if !rest.to_ascii_lowercase().starts_with(&lit.to_ascii_lowercase()) {
                return None;
            }
            rest = &rest[lit.len()..];
            let close = p[open..].find('}')? + open;
            let name = &p[open + 1..close];
            let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
            if end == 0 {
                return None;
            }
            out.insert(name.to_string(), Value::String(rest[..end].to_string()));
            rest = &rest[end..];
            p = &p[close + 1..];
        }
        (!out.is_empty()).then_some(out)
    }

    /// Check a spot's fields against `target`: required ones there, `enum` and
    /// `pattern` honoured, nothing it does not declare. The values land in URLs.
    pub fn check_target(&self, fields: &BTreeMap<String, Value>) -> Result<BTreeMap<String, Value>> {
        let mut out = BTreeMap::new();
        for (name, v) in fields {
            let Some(spec) = self.target.get(name) else { bail!("{} has no field '{name}'", self.display_name()) };
            let text = match v {
                Value::String(s) => s.trim().to_string(),
                Value::Null => continue,
                other => other.to_string(),
            };
            if text.is_empty() {
                continue;
            }
            if let Some(opts) = spec["enum"].as_array() {
                if !opts.iter().any(|o| o.as_str() == Some(text.as_str())) {
                    bail!("'{text}' is not one of the {} for {name}", opts.iter().filter_map(|o| o.as_str()).collect::<Vec<_>>().join(", "));
                }
            }
            if let Some(p) = spec["pattern"].as_str() {
                let re = regex::Regex::new(p).map_err(|e| anyhow::anyhow!("{}: the pattern of {name} does not compile: {e}", self.kind))?;
                if !re.is_match(&text) {
                    bail!("'{text}' is not a valid {}", spec["label"].as_str().unwrap_or(name).to_lowercase());
                }
            } else if text.contains(['/', '?', '#', '\\']) || text.chars().any(char::is_control) {
                bail!("'{text}' is not a valid {}", spec["label"].as_str().unwrap_or(name).to_lowercase());
            }
            out.insert(name.clone(), Value::String(text));
        }
        let missing: Vec<&str> = self.required().into_iter().filter(|n| !out.contains_key(*n)).collect();
        if !missing.is_empty() {
            bail!("{} needs {}", self.display_name(), missing.join(", "));
        }
        Ok(out)
    }

    /// A spot's link: `url` filled with its fields.
    pub fn url_of(&self, fields: &BTreeMap<String, Value>) -> String {
        let mut u = self.url.clone();
        for (k, v) in fields {
            u = u.replace(&format!("{{{{{k}}}}}"), v.as_str().unwrap_or(""));
        }
        if u.contains("{{") { String::new() } else { u }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

/// When KIQR suggests an ability to a new project, from the welcome questions
/// and the crawl: the goals it serves (`customers`, `repeat`, `known`,
/// `launch`) and text in a site's HTML that shows the business already uses
/// the service (a script URL, a generator tag). Matched case-insensitively.
/// When `html` is given it has to match: a goal or a named network alone
/// doesn't suggest an ability that needs the site to run on something.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Fits {
    pub goals: Vec<String>,
    pub html: Vec<String>,
    /// Worth having for any website (a search console): suggested to every project, saying so.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub every_site: bool,
}

impl Fits {
    pub fn is_empty(&self) -> bool {
        self.goals.is_empty() && self.html.is_empty() && !self.every_site
    }
}

/// `learn:`: the documents an account is read for, each the answer of one of
/// the ability's own tools (`args` filled like a call's), or, with `each`, of
/// one tool per entry of that answer (every Markdown file in `docs/`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LearnSpec {
    /// What the switch says: "Learn the product from the repository".
    pub label: String,
    pub documents: Vec<LearnDoc>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LearnDoc {
    pub tool: String,
    pub args: Value,
    /// Templates over the account's keys (never a secret) and, under `each`, `{{value}}`:
    /// the document's title, and the link its facts cite as evidence.
    pub title: String,
    pub url: String,
    pub each: Option<LearnEach>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LearnEach {
    /// Pointer to the list in the first tool's answer; empty: the answer is the list.
    pub list: String,
    /// Pointer into an entry: the value handed on (`/path`).
    pub value: String,
    /// Entries whose fields equal these (`type: file`).
    #[serde(rename = "where")]
    pub where_: BTreeMap<String, String>,
    /// Values ending in one of these (`.md`), case-insensitive; empty: all.
    pub only: Vec<String>,
    /// The tool called per entry, with the value as `arg`.
    pub tool: String,
    pub arg: String,
    /// At most this many entries (default 20, at most 50).
    pub max: Option<usize>,
}

impl LearnEach {
    pub fn limit(&self) -> usize {
        self.max.unwrap_or(20).clamp(1, 50)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub summary: String,
    pub transport: Transport,
    /// The service whose accounts this ability uses. Abilities that name the
    /// same service share its accounts (one sign-in, one browser profile, one
    /// daily cap) and agree on its key names. Empty: the ability's own id, so
    /// its accounts are its own. A string, or a block with how the service
    /// shows in the grid of things to connect (`Service`).
    pub service: Service,
    /// What an account of this ability allows Seemo when it is made, by role
    /// (`accounts::PERMISSIONS`); the user changes it on the account. A role
    /// not named has its default: acting asks, the rest is on.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub allow: BTreeMap<String, crate::accounts::Allow>,
    /// What the project picks once for this ability, through one of its
    /// accounts (an analytics property, a Search Console site): name → {label,
    /// help, placeholder, optional, enum, choices}. Kept in the project
    /// (`project.yaml`, `ability_settings`), never on the account; the values
    /// fill the ability's calls (`{{property}}`) unless a call names its own. A
    /// field with `choices` is picked from a list read through the account.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub settings: BTreeMap<String, Value>,
    /// One call at a time per account, across every ability of the service
    /// that says so: a browser session that two callers must not drive at once.
    pub exclusive: bool,
    /// Actions (tool calls with a role other than `read`) per account per
    /// day, across every ability of the service; the lowest declared wins.
    pub daily_cap: Option<u32>,
    pub auth: Vec<AuthKey>,
    /// How the keys can be got with one button instead of typed in (`docs/abilities.md`,
    /// "Signing in"): one block, or a list of ways (the first with a button is the button).
    #[serde(deserialize_with = "crate::connect::one_or_many", skip_serializing_if = "Vec::is_empty")]
    pub connect: Vec<crate::connect::Connect>,
    pub hosts: Vec<String>,
    /// The mail server, for `transport: smtp`.
    pub smtp: Option<SmtpSpec>,
    /// ability id → how this connection provides it.
    pub provides: BTreeMap<String, Provide>,
    pub tools: Vec<ExtraTool>,
    /// Where content goes out through this ability (`Kind`).
    pub kinds: Vec<Kind>,
    /// Metrics sources this connection reads (`traffic`, `search`, `conversions`, `ranks`, `mentions`),
    /// each with the requests and the mapping that read it (`crate::sources`).
    #[serde(deserialize_with = "crate::sources::deserialize")]
    pub sources: BTreeMap<String, crate::sources::SourceSpec>,
    /// What an account of this ability can teach the project (a repository's
    /// README and docs): documents read through its own tools and extracted
    /// into the business profile like the site's pages. An account with
    /// learning on is a source; it may be a channel too.
    pub learn: Option<LearnSpec>,
    /// When to suggest it to a new project.
    #[serde(skip_serializing_if = "Fits::is_empty")]
    pub fits: Fits,
    // MCP transport.
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<String>,
    pub cwd: Option<String>,
    /// MCP: which of the server's tools to offer raw (`all`, or names). Empty: none.
    pub expose: Vec<String>,
    /// MCP: run on another installed ability's program instead of a command of
    /// its own (the browser): that ability's `command`, `args`, `env_set` and
    /// `requires`, one server per account shared by every ability that names it.
    pub program: String,
    /// Environment set for the program, each value a template over the
    /// account's keys, `${KIQR_ACCOUNT_DIR}` and `${KIQR_PROJECT_DIR}`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub env_set: BTreeMap<String, String>,
    /// Tools of the program called once each time it starts, before anything
    /// else (opening the browser), each a step with `call` and `args`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warmup: Vec<Step>,
    /// Programs KIQR installs for this ability (pinned, checked), honoured only
    /// for official abilities.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Requirement>,
    /// Arguments of exposed or own tools that are URLs: each is checked against `hosts`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub url_args: Vec<String>,
    /// How an account of this ability is signed in and kept signed in on a hosted program.
    pub session: Option<Session>,
    /// How a person can see and drive the hosted program when a sign-in needs a hand.
    pub live_view: Option<LiveView>,
    /// A link for the Abilities page.
    pub homepage: String,
    /// Marketplace shelves: images, video, music, email, publishing, browser, tools.
    pub categories: Vec<String>,
    /// A lucide icon name for the card; the ability's icon when empty.
    pub icon: String,
    /// A 64×64 mark: a file name in the folder (`avatar.svg`) or a URL. Optional; the icon is the fallback.
    pub avatar: String,
}

impl Manifest {
    /// The service whose accounts this ability uses: `service`, else its own id.
    pub fn service(&self) -> &str {
        if self.service.id.is_empty() { &self.id } else { &self.service.id }
    }
    /// The service as the grid of things to connect shows it: the block's
    /// name, icon and group, else the ability's own.
    pub fn service_card(&self) -> Service {
        Service {
            id: self.service().to_string(),
            name: if self.service.name.is_empty() { self.name.clone() } else { self.service.name.clone() },
            icon: if self.service.icon.is_empty() { self.icon.clone() } else { self.service.icon.clone() },
            group: self.service.group.clone(),
            profile_hosts: self.service.profile_hosts.clone(),
        }
    }
}

/// A program KIQR installs for an ability: pinned to a version, fetched from
/// a URL per platform and checked against its sha256 before it is unpacked
/// into `~/.kiqr/bin`, or installed as a `uv` tool, or named as a system
/// package the user installs. Honoured only for official abilities.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Requirement {
    /// The program's name on PATH once installed.
    pub program: String,
    pub version: String,
    /// A command that prints something containing `version` when the right one is there (default: `<program> --version`).
    pub check: Vec<String>,
    pub install: Vec<Download>,
    /// `uv tool install <this>` (a pinned spec), through the `uv` KIQR installed.
    pub uv_tool: String,
    /// A system package by package manager (`apt`, `brew`, `winget`): shown to the user as a command, never run.
    pub system: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Download {
    /// `linux`, `macos`, `windows`.
    pub os: String,
    /// `x86_64`, `aarch64`.
    pub arch: String,
    pub url: String,
    pub sha256: String,
    /// `zip`, `tar.gz`, or empty for a bare binary.
    pub archive: String,
    /// The binary's path inside the archive (or its name); the program's name when empty.
    /// With `unpack`, the path of the program inside the unpacked archive (`xvfb-21.1/bin/Xvfb`).
    pub bin: String,
    /// Unpack the whole archive (a program with its libraries and data) instead of taking one binary out.
    pub unpack: bool,
}

/// Signing an account in on a hosted program and knowing whether it is:
/// `check` steps say (the last answer's text matched by `signed_in_when`, or
/// when neither marker is set, the model reading the page), the sign-in
/// starts at `login_url` (the model reads each page's controls, in whatever
/// language, and names the user, password and code fields; the runner types
/// into those and nothing else, the password only into a password field) or
/// follows `login` steps that type the credentials (`{{credential.username}}`,
/// `{{credential.password}}`, `{{credential.otp}}`: only here, never in a
/// tool Seemo calls), `blocked_when` texts in an answer mean a person is
/// needed (the model reading the page says so too), and `every` is how often
/// the check runs.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub check: Vec<Step>,
    /// A piece of text (or a selector the program echoes) in the check's last answer that means signed in.
    pub signed_in_when: String,
    /// Or the other way round: texts that mean signed out (a password field on the start page); signed in when none shows.
    pub signed_out_when: Vec<String>,
    /// Where signing in starts. The model reads the pages from here and the runner fills them: no selectors to keep up to date.
    pub login_url: String,
    /// Or fixed steps, for a site whose form never changes (a test site).
    pub login: Vec<Step>,
    pub blocked_when: Vec<String>,
    /// `6h`, `30m`, `1d`; empty: 6 hours.
    pub every: String,
}

impl Session {
    /// Whether the runner can sign in on its own (else a person does, once, in the live view).
    pub fn signs_in(&self) -> bool {
        !self.login_url.trim().is_empty() || !self.login.is_empty()
    }

    pub fn every(&self) -> std::time::Duration {
        let t = self.every.trim();
        let (n, unit) = t.split_at(t.len().saturating_sub(1));
        match (n.parse::<u64>(), unit) {
            (Ok(n), "m") => std::time::Duration::from_secs(n * 60),
            (Ok(n), "h") => std::time::Duration::from_secs(n * 3600),
            (Ok(n), "d") => std::time::Duration::from_secs(n * 86400),
            _ => std::time::Duration::from_secs(6 * 3600),
        }
    }
}

/// The program's tools a person's live view is built from, each a step on
/// the program (`call` and `args`): `frame` answers with an image of the
/// page; `click` gets `{{x}}` and `{{y}}` (a fraction of the frame's width and
/// height, 0 to 1, and `{{px}}`, `{{py}}` in pixels of the frame), `key` gets
/// `{{key}}`, `type` gets `{{text}}`, `navigate` gets `{{url}}`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LiveView {
    pub frame: Step,
    /// Optional: answers the window's geometry as JSON (`ow`, `oh`, `iw`, `ih`: the window's and the
    /// page's width and height in CSS pixels), so a click on a frame of the whole window lands on the page.
    pub metrics: Step,
    pub click: Step,
    pub key: Step,
    #[serde(rename = "type")]
    pub type_: Step,
    pub navigate: Step,
}

/// The list a field is picked from, when it has one: an `auth` key's
/// `choices`, or a setting's.
pub fn choices_of(m: &Manifest, name: &str) -> Option<Choices> {
    if let Some(c) = m.auth.iter().find(|a| a.env == name).and_then(|a| a.choices.clone()) {
        return Some(c);
    }
    let spec = m.settings.get(name)?;
    serde_json::from_value(spec.get("choices")?.clone()).ok()
}

/// A service in the grid of things a project can connect: its id (what
/// accounts are keyed by), its name and icon as shown, the group it sits in
/// (`SERVICE_GROUPS`) and the hosts its profiles live on. In a manifest,
/// `service:` is the id alone or this block.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Service {
    pub id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub icon: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub group: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub profile_hosts: Vec<String>,
}

/// Where a service sits in the grid of things to connect.
pub const SERVICE_GROUPS: &[(&str, &str)] = &[("social", "Social"), ("website", "Website and blog"), ("email", "Email"), ("numbers", "Numbers"), ("passwords", "Passwords"), ("sites", "Sites behind a login"), ("tools", "Tools")];

impl<'de> Deserialize<'de> for Service {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Id(String),
            Block {
                #[serde(default)]
                id: String,
                #[serde(default)]
                name: String,
                #[serde(default)]
                icon: String,
                #[serde(default)]
                group: String,
                #[serde(default)]
                profile_hosts: Vec<String>,
            },
        }
        Ok(match Raw::deserialize(d)? {
            Raw::Id(id) => Service { id, ..Default::default() },
            Raw::Block { id, name, icon, group, profile_hosts } => Service { id, name, icon, group, profile_hosts },
        })
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: "0.1.0".into(),
            summary: String::new(),
            transport: Transport::Http,
            service: Service::default(),
            allow: BTreeMap::new(),
            settings: BTreeMap::new(),
            exclusive: false,
            daily_cap: None,
            auth: vec![],
            connect: vec![],
            hosts: vec![],
            smtp: None,
            provides: Default::default(),
            tools: vec![],
            kinds: vec![],
            sources: BTreeMap::new(),
            learn: None,
            fits: Fits::default(),
            program: String::new(),
            env_set: BTreeMap::new(),
            warmup: Vec::new(),
            requires: Vec::new(),
            url_args: Vec::new(),
            session: None,
            live_view: None,
            command: String::new(),
            args: vec![],
            env: vec![],
            cwd: None,
            expose: vec![],
            homepage: String::new(),
            categories: vec![],
            icon: String::new(),
            avatar: String::new(),
        }
    }
}

/// SKILL.md frontmatter: what the ability is called, when Seemo should use
/// it, and (for sharing) how the marketplace shows it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Instructions {
    /// The action, as the user says it: "Write product update posts".
    pub name: String,
    /// One line: when Seemo should read and follow it.
    pub description: String,
    /// A longer summary for the card; the description when empty.
    pub summary: String,
    pub categories: Vec<String>,
    pub icon: String,
    /// Vendor, as in the marketplace (a GitHub user or org); empty for the user's own.
    pub author: String,
    /// Ability contracts it needs (`image`, `publish`), shown as "needs a connection" when none is ready.
    pub abilities: Vec<String>,
    /// The kinds it is about (`reddit`): informational, shown with it.
    #[serde(alias = "channels")]
    pub kinds: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "scope", content = "project")]
pub enum Scope {
    Global,
    Project(String),
}

/// One folder under `abilities/`.
#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    /// Folder name.
    pub name: String,
    pub dir: PathBuf,
    pub scope: Scope,
    pub builtin: bool,
    pub instructions: Option<Instructions>,
    pub manifest: Option<Manifest>,
    /// Why the manifest could not be used, if so.
    pub error: Option<String>,
    /// Installed but switched off (`.off` in the folder): Seemo does not see it.
    pub paused: bool,
    /// Came from a git repository (`.source` holds the URL, or `.git` is there); `update` fetches it again.
    pub git: bool,
    /// The repository it was installed from, when known.
    pub source: Option<String>,
}

impl Entry {
    /// The ability id: the manifest's, else the folder name.
    pub fn id(&self) -> &str {
        self.manifest.as_ref().map(|m| m.id.as_str()).unwrap_or(&self.name)
    }
    /// Whether this folder is one of the core abilities (never removed).
    pub fn is_core(&self) -> bool {
        CORE_FOLDERS.contains(&self.name.as_str()) || CORE_IDS.contains(&self.id())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub entries: Vec<Entry>,
}

/// Folders KIQR ships: name → (SKILL.md, extension.yaml). The core ones are
/// installed on every start. Everything else, including abilities whose code
/// is in KIQR (email over SMTP, the analytics readers), lives in the
/// marketplace and is installed from there. A user's copy of a SKILL.md wins.
pub const BUILTIN_EXTENSIONS: &[(&str, Option<&str>, Option<&str>)] = &[
    ("make-images", Some(include_str!("../../../abilities/make-images/SKILL.md")), Some(include_str!("../../../abilities/make-images/extension.yaml"))),
    ("browser", Some(include_str!("../../../abilities/browser/SKILL.md")), Some(include_str!("../../../abilities/browser/extension.yaml"))),
];

/// On without installing: the built-in renderer (folder `make-images`, id
/// `kiqr-render`) and the real browser (folder and id `browser`, the service
/// "A site behind a login"; the programs it runs install when its first
/// account is connected, not on start). Web reading is a built-in tool, not a folder.
pub const CORE_FOLDERS: &[&str] = &["make-images", "browser"];
pub const CORE_IDS: &[&str] = &["kiqr-render", "browser"];

/// What KIQR can do out of the box that is code rather than a folder, named
/// as the user sees it in the marketplace. Always installed, never removed,
/// switched off through `config.core_off`; one of them (UnblockingAPI) takes
/// a key, on the machine or per project. `instructions` names the shipped
/// folders that are its instructions (the user edits those, not the code).
pub struct CoreAbility {
    pub id: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub categories: &'static [&'static str],
    pub icon: &'static str,
    /// (env, label, help)
    pub auth: &'static [(&'static str, &'static str, &'static str)],
    pub cost: &'static str,
    pub homepage: &'static str,
    /// Shipped instruction folders, in reading order: (folder, label).
    pub instructions: &'static [(&'static str, &'static str)],
    /// Listed in Seemo's prompt as something to read before acting (KIQR's
    /// own pipeline prompts are not: the crawler runs them).
    pub in_prompt: bool,
}

pub const CORE_ABILITIES: &[CoreAbility] = &[
    CoreAbility { id: "write", name: "Write articles, posts and emails", summary: "Seemo writes in the brand's tone of voice from the business brief: articles, social posts, replies, emails, and answers in chat. Everything lands in the library as a draft.", categories: &["writing"], icon: "pen-tool", auth: &[], cost: "model usage", homepage: "", instructions: &[], in_prompt: false },
    CoreAbility { id: "business-profile", name: "Crawl a website and build its business profile", summary: "Fetch the site's own pages, extract what they say, and write the profile, the brand and the business brief Seemo works from.", categories: &["web", "writing"], icon: "brain", auth: &[], cost: "model usage", homepage: "", instructions: &[("crawl-site", "Reading the site"), ("extract-facts", "Extracting facts"), ("build-profile", "Building the profile"), ("business-brief", "Writing the brief")], in_prompt: false },
    CoreAbility { id: "read-web", name: "Read the web", summary: "Seemo reads one page at a time through KIQR's own fetcher: a competitor, an article you link, a source to quote.", categories: &["web"], icon: "globe", auth: &[], cost: "free", homepage: "", instructions: &[], in_prompt: false },
    CoreAbility { id: "unblockingapi", name: "Read blocked pages with UnblockingAPI", summary: "A real browser and residential proxies for the pages KIQR could not read: crawls and Seemo's web reading. One credit per success; failures are free.", categories: &["web"], icon: "shield-check", auth: &[("UNBLOCKINGAPI_KEY", "UnblockingAPI key", "https://unblockingapi.com")], cost: "about $0.001 per page read this way", homepage: "https://unblockingapi.com", instructions: &[], in_prompt: false },
    CoreAbility { id: "social-posts", name: "Write social media posts", summary: "Ready-to-publish posts in the brand's tone of voice, each with its image, from the business brief.", categories: &["writing", "publishing"], icon: "message-square-text", auth: &[], cost: "model usage", homepage: "", instructions: &[("social-posts", "Writing the posts")], in_prompt: true },
    CoreAbility { id: "marketing", name: "Plan the marketing", summary: "Seemo owns a marketing plan for the business: a few objectives for the period, each with a KPI and a target, and campaigns under them that group what it makes. It proposes the plan from the brief; you approve it on a card.", categories: &["strategy"], icon: "target", auth: &[], cost: "model usage", homepage: "", instructions: &[("marketing-plan", "Building the plan")], in_prompt: true },
    CoreAbility { id: "tasks", name: "Run tasks on a schedule", summary: "Seemo works later or repeatedly: every Monday a post, each morning a check, a batch for the library. Every task is approved by you first and reports back in its chat.", categories: &["writing"], icon: "list-checks", auth: &[], cost: "model usage", homepage: "", instructions: &[], in_prompt: true },
];

/// Seemo's own tool guide: a shipped instruction folder that belongs to no ability.
pub const GUIDE_FOLDER: &str = "seemo-agent";

/// Folders that are part of something else (a core ability's instructions,
/// Seemo's guide) and never listed as abilities of their own.
pub fn attached_folders() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = CORE_ABILITIES.iter().flat_map(|c| c.instructions.iter().map(|(f, _)| *f)).collect();
    v.push(GUIDE_FOLDER);
    v
}

/// The core ability a shipped instruction folder belongs to, if any.
pub fn core_of_folder(folder: &str) -> Option<&'static CoreAbility> {
    CORE_ABILITIES.iter().find(|c| c.instructions.iter().any(|(f, _)| *f == folder))
}

pub fn core_ability(id: &str) -> Option<&'static CoreAbility> {
    CORE_ABILITIES.iter().find(|c| c.id == id)
}

/// Whether an id is core: code (`CORE_ABILITIES`) or a core folder.
pub fn is_core_id(id: &str) -> bool {
    core_ability(id).is_some() || CORE_IDS.contains(&id) || CORE_FOLDERS.contains(&id)
}

pub fn builtin_names() -> Vec<&'static str> {
    BUILTIN_EXTENSIONS.iter().map(|(n, _, _)| *n).collect()
}

/// A shipped folder by its manifest id: (folder, SKILL.md, extension.yaml, manifest).
pub fn shipped(id: &str) -> Option<(&'static str, Option<&'static str>, &'static str, Manifest)> {
    BUILTIN_EXTENSIONS.iter().find_map(|(folder, skill, manifest)| {
        let text = (*manifest)?;
        let m = parse_manifest(text).ok()?;
        (m.id == id).then_some((*folder, *skill, text, m))
    })
}

/// Every shipped manifest, parsed: the offline marketplace.
pub fn shipped_all() -> Vec<(&'static str, Manifest)> {
    BUILTIN_EXTENSIONS.iter().filter_map(|(folder, _, manifest)| (*manifest).and_then(|t| parse_manifest(t).ok()).map(|m| (*folder, m))).collect()
}

/// Folders an older KIQR installed on its own, which the marketplace now
/// leaves to the user: removed when they were never set up (only the shipped
/// manifest inside, no key present), kept and treated as installed otherwise.
/// Returns the folder names removed.
pub fn retire_auto_installed(store: &crate::Store, has_key: impl Fn(&str) -> bool) -> Result<Vec<String>> {
    let mut removed = vec![];
    // Once per home: after that, what is in `skills/` is the user's choice.
    let marker = store.home().abilities_dir().join(".marketplace");
    if marker.exists() {
        return Ok(removed);
    }
    for (folder, _, manifest) in BUILTIN_EXTENSIONS {
        if CORE_FOLDERS.contains(folder) {
            continue;
        }
        let Some(text) = manifest else { continue };
        let dir = store.home().abilities_dir().join(folder);
        if !dir.is_dir() {
            continue;
        }
        let only_shipped = std::fs::read_dir(&dir).map(|rd| rd.flatten().all(|e| matches!(e.file_name().to_str(), Some("extension.yaml" | ".builtin" | "SKILL.previous.md")))).unwrap_or(false);
        // A manifest that does not parse is kept: never remove on a doubt.
        let set_up = parse_manifest(text).map(|m| !m.auth.is_empty() && missing_keys(&m, &has_key).is_empty()).unwrap_or(true);
        if only_shipped && !set_up {
            std::fs::remove_dir_all(&dir)?;
            removed.push(folder.to_string());
        }
    }
    std::fs::create_dir_all(store.home().abilities_dir())?;
    std::fs::write(&marker, "the marketplace decides what is installed from here on\n")?;
    Ok(removed)
}

impl Registry {
    /// Every folder in the machine's `abilities/` and in each project's.
    /// `core_off` (from `config.yaml`) is the switch for core folders, so
    /// `Entry.paused` answers for every entry alike.
    pub fn load(home: &Home, core_off: &[String]) -> Registry {
        let mut entries = Vec::new();
        entries.extend(scan(&home.abilities_dir(), Scope::Global));
        if let Ok(rd) = std::fs::read_dir(home.projects_dir()) {
            for p in rd.flatten() {
                if !p.path().is_dir() {
                    continue;
                }
                let slug = p.file_name().to_string_lossy().to_string();
                entries.extend(scan(&home.project_abilities_dir(&slug), Scope::Project(slug)));
            }
        }
        for e in entries.iter_mut() {
            if e.is_core() && core_off.iter().any(|id| id == e.id() || id == &e.name) {
                e.paused = true;
            }
        }
        Registry { entries }
    }

    /// The folder an ability id names, as this project sees it (its own copy first).
    pub fn entry(&self, slug: Option<&str>, id: &str) -> Option<&Entry> {
        let project = slug.and_then(|s| self.entries.iter().find(|e| e.scope == Scope::Project(s.to_string()) && e.id() == id));
        project.or_else(|| self.entries.iter().find(|e| e.scope == Scope::Global && e.id() == id))
    }

    /// The entries a project sees: its own, then the global ones it does not shadow.
    pub fn visible(&self, slug: &str) -> Vec<&Entry> {
        let mut out: Vec<&Entry> = self.entries.iter().filter(|e| e.scope == Scope::Project(slug.to_string())).collect();
        for e in self.entries.iter().filter(|e| e.scope == Scope::Global) {
            if !out.iter().any(|o| o.name == e.name) {
                out.push(e);
            }
        }
        out
    }

    /// The installed abilities (entries with a usable manifest, not paused) a project sees.
    pub fn connections(&self, slug: &str) -> Vec<(&Entry, &Manifest)> {
        self.visible(slug).into_iter().filter(|e| !e.paused).filter_map(|e| e.manifest.as_ref().filter(|_| e.error.is_none()).map(|m| (e, m))).collect()
    }

    /// The working manifests a project sees for a service (`Manifest::service`),
    /// paused ones included: every ability that uses the service's accounts.
    pub fn of_service(&self, slug: &str, service: &str) -> Vec<&Manifest> {
        self.visible(slug).into_iter().filter(|e| e.error.is_none()).filter_map(|e| e.manifest.as_ref()).filter(|m| m.service() == service).collect()
    }

    /// Every installed folder with this ability id, paused or not, any scope.
    pub fn installed(&self, id: &str) -> Vec<&Entry> {
        self.entries.iter().filter(|e| e.id() == id).collect()
    }

    pub fn connection(&self, slug: &str, id: &str) -> Option<(&Entry, &Manifest)> {
        self.connections(slug).into_iter().find(|(_, m)| m.id == id)
    }

    /// Which connection serves an ability for this project: the configured
    /// one if it is ready, else a ready core one (the renderer), else the first ready one.
    pub fn provider<'a>(&'a self, slug: &str, ability: &str, configured: Option<&str>, ready: impl Fn(&Manifest) -> bool) -> Option<(&'a Entry, &'a Manifest)> {
        let candidates: Vec<(&Entry, &Manifest)> = self.connections(slug).into_iter().filter(|(_, m)| m.provides.contains_key(ability) && ready(m)).collect();
        if let Some(c) = configured {
            if let Some(found) = candidates.iter().find(|(_, m)| m.id == c) {
                return Some(*found);
            }
        }
        if let Some(found) = candidates.iter().find(|(e, _)| e.is_core()) {
            return Some(*found);
        }
        candidates.first().copied()
    }
}

fn scan(dir: &Path, scope: Scope) -> Vec<Entry> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        let instructions = std::fs::read_to_string(path.join("SKILL.md")).ok().map(|t| instructions_meta(&name, &t));
        let (manifest, error) = match std::fs::read_to_string(path.join("extension.yaml")) {
            Ok(text) => match parse_manifest(&text) {
                Ok(m) => (Some(m), None),
                Err(err) => (None, Some(err.to_string())),
            },
            Err(_) => (None, None),
        };
        if instructions.is_none() && manifest.is_none() && error.is_none() {
            continue;
        }
        let builtin = builtin_names().contains(&name.as_str()) || crate::extract::builtin_instructions(&name).is_some();
        let paused = path.join(".off").exists();
        let source = std::fs::read_to_string(path.join(".source")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let git = source.is_some() || path.join(".git").is_dir();
        out.push(Entry { name, dir: path, scope: scope.clone(), builtin, instructions, manifest, error, paused, git, source });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Parse and validate a manifest.
pub fn parse_manifest(text: &str) -> Result<Manifest> {
    let m: Manifest = serde_yaml::from_str(text).context("extension.yaml does not parse")?;
    validate(&m)?;
    Ok(m)
}

pub fn validate(m: &Manifest) -> Result<()> {
    if !crate::tools::valid_ext_id(&m.id) {
        bail!("id must be lowercase letters, digits and dashes (got '{}')", m.id);
    }
    if m.name.trim().is_empty() {
        bail!("name is missing");
    }
    if !m.service.id.is_empty() && !crate::tools::valid_ext_id(&m.service.id) {
        bail!("service must be lowercase letters, digits and dashes (got '{}')", m.service.id);
    }
    if !m.service.group.is_empty() && !SERVICE_GROUPS.iter().any(|(g, _)| *g == m.service.group) {
        bail!("service group '{}' is not one of {}", m.service.group, SERVICE_GROUPS.iter().map(|(g, _)| *g).collect::<Vec<_>>().join(", "));
    }
    for h in &m.service.profile_hosts {
        if h.trim().is_empty() || h.contains('/') || h.contains(':') {
            bail!("service: profile_hosts holds '{h}', which is not a host name");
        }
    }
    for (name, spec) in &m.settings {
        if !valid_env_name(&name.to_ascii_uppercase()) || name.contains(char::is_uppercase) {
            bail!("settings: '{name}' is not a field name (lowercase letters, digits, _)");
        }
        if let Some(ch) = spec.get("choices") {
            let ch: Choices = serde_json::from_value(ch.clone()).map_err(|e| anyhow::anyhow!("settings: the choices of {name} do not parse: {e}"))?;
            if ch.request.url.trim().is_empty() || ch.list.trim().is_empty() || ch.value.trim().is_empty() {
                bail!("settings: the choices of {name} need a request, a list and a value");
            }
        }
    }
    for (role, a) in &m.allow {
        if !crate::accounts::PERMISSIONS.contains(&role.as_str()) {
            bail!("allow names '{role}', which is not one of {}", crate::accounts::PERMISSIONS.join(", "));
        }
        if !crate::accounts::Allow::acts(role) && *a == crate::accounts::Allow::Ask {
            bail!("allow: '{role}' never asks; it is on or off");
        }
    }
    for a in &m.auth {
        if !valid_env_name(&a.env) {
            bail!("auth env '{}' is not a variable name (A-Z, 0-9, _)", a.env);
        }
    }
    for h in &m.hosts {
        // A host name, or `${VAR}`: a site each project names (its value may be a URL).
        let is_var = h.starts_with("${") && h.ends_with('}') && valid_env_name(&h[2..h.len() - 1]);
        if !is_var && (h.trim().is_empty() || h.contains('/') || h.contains(':')) {
            bail!("hosts holds '{h}', which is not a host name");
        }
    }
    for c in &m.connect {
        crate::connect::validate(m, c)?;
    }
    for a in &m.auth {
        if let Some(ch) = &a.choices {
            if ch.request.url.trim().is_empty() || ch.list.trim().is_empty() || ch.value.trim().is_empty() {
                bail!("the choices of {} need a request, a list and a value", a.env);
            }
        }
    }
    let mut names: Vec<&str> = m.tools.iter().map(|t| t.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    if names.len() != m.tools.len() {
        bail!("two tools share a name");
    }
    for t in &m.tools {
        if !is_tool_name(&t.name) {
            bail!("tool name '{}' must be snake_case", t.name);
        }
        if !t.role.is_empty() && !ROLES.contains(&t.role.as_str()) {
            bail!("tool '{}' has role '{}', which is not one of {}", t.name, t.role, ROLES.join(", "));
        }
        if t.input != Value::Null && t.input["type"] != "object" {
            bail!("tool '{}' input must be a JSON schema object", t.name);
        }
        check_provide(m, &t.provide, &format!("tool '{}'", t.name))?;
    }
    for (ability, p) in &m.provides {
        if crate::abilities::find(ability).is_none() && !crate::abilities::INTERNAL.contains(&ability.as_str()) {
            bail!("'{ability}' is not an ability (image, video, music, email, credential)");
        }
        check_provide(m, p, &format!("ability '{ability}'"))?;
    }
    for (src, spec) in &m.sources {
        if !crate::metrics::SOURCES.iter().any(|(id, _)| id == src) {
            bail!("'{src}' is not a metrics source ({})", crate::metrics::SOURCES.iter().map(|(id, _)| *id).collect::<Vec<_>>().join(", "));
        }
        crate::sources::validate(src, spec)?;
        for (i, r) in spec.requests.iter().enumerate() {
            check_provide(m, &Provide { request: Some(r.request.clone()), ..Default::default() }, &format!("source '{src}' request {}", i + 1))?;
        }
    }
    for r in &m.requires {
        if r.program.trim().is_empty() {
            bail!("requires: every program needs a name");
        }
        if r.version.trim().is_empty() && (!r.install.is_empty() || !r.uv_tool.trim().is_empty()) {
            bail!("requires: {} needs a version to pin", r.program);
        }
        if r.install.is_empty() && r.uv_tool.trim().is_empty() && r.system.is_empty() {
            bail!("requires: {} says nothing about how to install it", r.program);
        }
        for d in &r.install {
            if !["linux", "macos", "windows"].contains(&d.os.as_str()) || !["x86_64", "aarch64"].contains(&d.arch.as_str()) {
                bail!("requires: {}: os is linux, macos or windows and arch x86_64 or aarch64", r.program);
            }
            if !d.url.starts_with("https://") || d.sha256.len() != 64 || !d.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                bail!("requires: {}: every download needs an https URL and its sha256", r.program);
            }
            if !["", "zip", "tar.gz"].contains(&d.archive.as_str()) {
                bail!("requires: {}: archive is zip, tar.gz or empty", r.program);
            }
        }
    }
    if !m.program.is_empty() && !crate::tools::valid_ext_id(&m.program) {
        bail!("program must be an ability id (got '{}')", m.program);
    }
    for (n, _) in &m.env_set {
        if !valid_env_name(n) {
            bail!("env_set: '{n}' is not a variable name");
        }
    }
    for st in &m.warmup {
        if st.call.trim().is_empty() {
            bail!("warmup: step '{}' needs the program tool it calls", st.name);
        }
    }
    if let Some(sess) = &m.session {
        if m.transport != Transport::Mcp {
            bail!("session needs transport: mcp");
        }
        for st in sess.check.iter().chain(&sess.login) {
            if st.call.trim().is_empty() {
                bail!("session: step '{}' needs the program tool it calls", st.name);
            }
        }
        if sess.check.is_empty() {
            bail!("session needs check steps");
        }
        // With no marker, the model reads the check's page; that needs the sign-in to be the model's too.
        if sess.signed_in_when.trim().is_empty() && sess.signed_out_when.is_empty() && sess.login_url.trim().is_empty() {
            bail!("session needs signed_in_when or signed_out_when, or a login_url (then the model reads the page)");
        }
        if !sess.login_url.trim().is_empty() && url::Url::parse(sess.login_url.trim()).is_err() && !sess.login_url.contains("${") {
            bail!("session: login_url '{}' is not a URL", sess.login_url.trim());
        }
    }
    if let Some(lv) = &m.live_view {
        if lv.frame.call.trim().is_empty() {
            bail!("live_view needs the program tool that answers with an image (frame.call)");
        }
    }
    match m.transport {
        Transport::Mcp if m.command.trim().is_empty() && m.program.trim().is_empty() => bail!("an mcp connection needs a command, or the program of another ability"),
        Transport::Smtp if m.smtp.as_ref().map(|s| s.host.trim().is_empty() || (s.user.trim().is_empty() && s.from.trim().is_empty())).unwrap_or(true) => bail!("an smtp connection needs an smtp block with host and user (or from)"),
        Transport::Http if m.hosts.is_empty() && (!m.provides.is_empty() || !m.tools.is_empty()) => bail!("an http connection must list the hosts it may call"),
        _ => {}
    }
    for n in m.env.iter().chain(m.tools.iter().flat_map(|t| t.provide.command.iter().flat_map(|c| c.env.iter()))) {
        if !valid_env_name(n) {
            bail!("env '{n}' is not a variable name");
        }
    }
    if let Some(l) = &m.learn {
        if l.documents.is_empty() {
            bail!("learn lists no documents");
        }
        for d in &l.documents {
            for t in std::iter::once(&d.tool).chain(d.each.as_ref().map(|e| &e.tool)) {
                if !m.tools.iter().any(|x| &x.name == t) {
                    bail!("learn reads with '{t}', which this connection does not define");
                }
            }
            if let Some(e) = &d.each {
                if e.arg.trim().is_empty() || e.value.trim().is_empty() {
                    bail!("learn's each needs a value and the arg it goes in");
                }
            }
            // Reading only: learning never acts on the account.
            for t in std::iter::once(&d.tool).chain(d.each.as_ref().map(|e| &e.tool)) {
                if m.tools.iter().any(|x| &x.name == t && !x.role.is_empty() && x.role != "read") {
                    bail!("learn may only read, and '{t}' acts");
                }
            }
        }
    }
    for c in &m.kinds {
        if c.kind.trim().is_empty() {
            bail!("a kind needs a name (`kind`)");
        }
        if !crate::tools::valid_ext_id(&c.kind) || c.kind == "list" {
            bail!("kind '{}' must be lowercase letters, digits and dashes (and not `list`)", c.kind);
        }
        if !c.section.is_empty() && !SECTIONS.contains(&c.section.as_str()) {
            bail!("kind '{}': section '{}' is not one of {}", c.kind, c.section, SECTIONS.join(", "));
        }
        if !c.publish.trim().is_empty() && !m.tools.iter().any(|t| t.name == c.publish) && !m.provides.contains_key(&c.publish) {
            bail!("kind '{}' publishes with '{}', which this ability does not define", c.kind, c.publish);
        }
        if c.account_is_target && !c.target.is_empty() {
            bail!("kind '{}': the account is the spot, so it has no target fields", c.kind);
        }
        for (name, spec) in &c.target {
            if !valid_env_name(&name.to_ascii_uppercase()) || name.contains(char::is_uppercase) {
                bail!("kind '{}': '{name}' is not a field name (lowercase letters, digits, _)", c.kind);
            }
            if let Some(p) = spec["pattern"].as_str() {
                regex::Regex::new(p).map_err(|e| anyhow::anyhow!("kind '{}': the pattern of {name} does not compile: {e}", c.kind))?;
            }
        }
        for (arg, pat) in &c.target_from {
            let named: Vec<&str> = pat.split('{').skip(1).filter_map(|x| x.split('}').next()).collect();
            if named.is_empty() || named.iter().any(|n| !c.target.contains_key(*n)) {
                bail!("kind '{}': target_from.{arg} must name the target's fields in braces", c.kind);
            }
        }
        if let Some(i) = c.image {
            if !(16..=8192).contains(&i.width) || !(16..=8192).contains(&i.height) {
                bail!("channel '{}': the image size is out of range", c.kind);
            }
        }
    }
    Ok(())
}

fn check_provide(m: &Manifest, p: &Provide, what: &str) -> Result<()> {
    match m.transport {
        Transport::Http => match (&p.request, p.steps.is_empty()) {
            (Some(_), false) => bail!("{what} has both a request and steps; use one"),
            (None, true) => bail!("{what} needs a request"),
            (Some(r), true) => check_request(m, r, what)?,
            (None, false) => {
                if p.steps.len() > MAX_STEPS {
                    bail!("{what} has {} steps; at most {MAX_STEPS}", p.steps.len());
                }
                let mut seen: Vec<&str> = vec![];
                for st in &p.steps {
                    let ok = !st.name.is_empty() && st.name.len() <= 40 && st.name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') && !st.name.starts_with(|c: char| c.is_ascii_digit());
                    if !ok || seen.contains(&st.name.as_str()) {
                        bail!("{what}: every step needs its own name, lowercase letters, digits and _ (got '{}')", st.name);
                    }
                    if matches!(st.name.as_str(), "file" | "files" | "index" | "item") {
                        bail!("{what}: '{}' is taken, name the step something else", st.name);
                    }
                    if let Some(f) = &st.for_each {
                        if f != "files" && !seen.contains(&f.as_str()) {
                            bail!("{what}: step '{}' repeats for each of `files` or of an earlier step, not '{f}'", st.name);
                        }
                    }
                    seen.push(&st.name);
                    check_request(m, &st.request, &format!("{what}, step '{}'", st.name))?;
                }
            }
        },
        Transport::Command => {
            if p.command.as_ref().map(|c| c.command.trim().is_empty()).unwrap_or(true) {
                bail!("{what} needs a command");
            }
        }
        Transport::Mcp if !p.steps.is_empty() => {
            if p.steps.len() > MAX_STEPS {
                bail!("{what} has {} steps; at most {MAX_STEPS}", p.steps.len());
            }
            for st in &p.steps {
                if st.call.trim().is_empty() {
                    bail!("{what}: step '{}' needs the program tool it calls", st.name);
                }
            }
        }
        Transport::Mcp => {
            if p.tool.as_ref().map(|t| t.trim().is_empty()).unwrap_or(true) {
                bail!("{what} needs the server's tool name");
            }
        }
        Transport::Smtp | Transport::Builtin => {}
    }
    Ok(())
}

fn check_request(m: &Manifest, r: &HttpRequest, what: &str) -> Result<()> {
    // A URL from an earlier step's answer is checked against hosts when it is called.
    if !r.url.trim().starts_with("{{") {
        // `${VAR}` in the URL stands for a site the project names; the host is checked at call time then.
        let url = url::Url::parse(&fill(&r.url, &Value::Null, |_| Some("https://placeholder.invalid".into()))).with_context(|| format!("{what}: the request url does not parse"))?;
        let per_project = m.hosts.iter().any(|h| h.starts_with("${"));
        if !per_project && !host_allowed(&url, &m.hosts) {
            bail!("{what} calls {}, which is not in hosts", url.host_str().unwrap_or(""));
        }
    }
    if !matches!(r.method.to_ascii_uppercase().as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE") {
        bail!("{what}: method '{}' is not one of GET, POST, PUT, PATCH, DELETE", r.method);
    }
    if r.body_kinds() > 1 {
        bail!("{what} has more than one body; use one of json, form, multipart and body_file");
    }
    for part in &r.multipart {
        if part.name.trim().is_empty() || (part.value.is_some() == part.file.is_some()) {
            bail!("{what}: a multipart part needs a name and either a value or a file");
        }
    }
    Ok(())
}

pub fn valid_env_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 64 && n.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') && !n.starts_with(|c: char| c.is_ascii_digit())
}

fn is_tool_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 64 && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') && !n.starts_with(|c: char| c.is_ascii_digit())
}

/// `host` or any of its subdomains.
/// The hosts an ability may call, with `${VAR}` entries filled from the
/// project's secrets; a value that is a URL counts by its host.
pub fn hosts_for(m: &Manifest, env: &impl Fn(&str) -> Option<String>) -> Vec<String> {
    m.hosts
        .iter()
        .filter_map(|h| {
            if h.starts_with("${") && h.ends_with('}') {
                let v = env(&h[2..h.len() - 1])?;
                let v = v.trim();
                return Some(url::Url::parse(v).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_else(|| v.trim_end_matches('/').to_string()));
            }
            Some(h.clone())
        })
        .collect()
}

pub fn host_allowed(url: &url::Url, hosts: &[String]) -> bool {
    let Some(h) = url.host_str() else { return false };
    let h = h.to_ascii_lowercase();
    hosts.iter().any(|a| {
        let a = a.trim().to_ascii_lowercase();
        h == a || h.ends_with(&format!(".{a}"))
    })
}

/// Where the `}}` that closes the `{{` at the start of `s` is, past any
/// `{{…}}` or `${…}` inside it.
fn closing(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut want: Vec<&[u8]> = vec![b"}}"];
    let mut i = 2;
    while i < b.len() {
        let rest = &b[i..];
        if rest.starts_with(b"{{") {
            want.push(b"}}");
            i += 2;
        } else if rest.starts_with(b"${") {
            want.push(b"}");
            i += 2;
        } else if want.last().is_some_and(|w| rest.starts_with(w)) {
            let w = want.pop()?;
            if want.is_empty() {
                return Some(i);
            }
            i += w.len();
        } else {
            i += 1;
        }
    }
    None
}

/// Fill `{{arg}}` / `{{arg|default}}` from the arguments and `${ENV}` from
/// `env`. A missing argument with no default becomes empty; a missing
/// variable stays as written so the caller can refuse.
pub fn fill(template: &str, args: &Value, env: impl Fn(&str) -> Option<String>) -> String {
    fill_with(template, args, &env, &mut Vec::new())
}

/// `fill`, refusing when the template names a variable (`${NAME}`, no default) that is not set:
/// the error is that name. Only the template is read for variables, never what an argument holds
/// (a file whose code says `${price}` goes through as written).
pub fn fill_checked(template: &str, args: &Value, env: impl Fn(&str) -> Option<String>) -> std::result::Result<String, String> {
    let mut missing = Vec::new();
    let out = fill_with(template, args, &env, &mut missing);
    match missing.into_iter().next() {
        Some(name) => Err(name),
        None => Ok(out),
    }
}

/// `fill_value` with `fill_checked`'s refusal.
pub fn fill_value_checked(v: &Value, args: &Value, env: &impl Fn(&str) -> Option<String>) -> std::result::Result<Value, String> {
    let mut missing = Vec::new();
    let out = fill_value_with(v, args, env, &mut missing);
    match missing.into_iter().next() {
        Some(name) => Err(name),
        None => Ok(out),
    }
}

fn fill_with(template: &str, args: &Value, env: &dyn Fn(&str) -> Option<String>, missing: &mut Vec<String>) -> String {
    let mut out = String::new();
    let mut rest = template;
    loop {
        let next_arg = rest.find("{{");
        let next_env = rest.find("${");
        let (at, is_arg) = match (next_arg, next_env) {
            (None, None) => {
                out.push_str(rest);
                return out;
            }
            (Some(a), None) => (a, true),
            (None, Some(e)) => (e, false),
            (Some(a), Some(e)) => {
                if a <= e {
                    (a, true)
                } else {
                    (e, false)
                }
            }
        };
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        if is_arg {
            let Some(end) = closing(rest) else {
                out.push_str(rest);
                return out;
            };
            let inner = &rest[2..end];
            let (key, default) = inner.split_once('|').map(|(k, d)| (k.trim(), Some(d.trim()))).unwrap_or((inner.trim(), None));
            let v = lookup(args, key).filter(|v| !v.is_null());
            match v.as_ref() {
                // `{{title|json}}`: the value as JSON (also valid YAML, for frontmatter); null when missing or empty.
                _ if default == Some("json") => match v {
                    Some(Value::String(s)) if s.is_empty() => out.push_str("null"),
                    Some(x) => out.push_str(&x.to_string()),
                    None => out.push_str("null"),
                },
                // `{{site|urlencode}}`: a value inside a URL path.
                Some(Value::String(s)) if default == Some("urlencode") => out.extend(url::form_urlencoded::byte_serialize(s.trim().as_bytes())),
                Some(Value::String(s)) if !s.is_empty() => out.push_str(s),
                // The default may hold placeholders of its own: `{{repo|${GITHUB_REPO}}}`, `{{title|{{tag}}}}`.
                Some(Value::String(_)) | None => out.push_str(&fill_with(default.unwrap_or(""), args, env, missing)),
                Some(other) => out.push_str(&other.to_string()),
            }
            rest = &rest[end + 2..];
        } else {
            let Some(end) = rest.find('}') else {
                out.push_str(rest);
                return out;
            };
            // `${NAME}`, `${NAME|urlencode}` (for a value inside a URL path), `${NAME|default}`.
            let inner = &rest[2..end];
            let (name, filter) = inner.split_once('|').map(|(n, f)| (n, Some(f))).unwrap_or((inner, None));
            match (valid_env_name(name).then(|| env(name)).flatten().filter(|v| !v.is_empty()), filter) {
                (Some(v), Some("urlencode")) => out.extend(url::form_urlencoded::byte_serialize(v.trim().as_bytes())),
                (Some(v), _) => out.push_str(&v),
                (None, Some(default)) if default != "urlencode" && valid_env_name(name) => out.push_str(default),
                _ => {
                    if valid_env_name(name) {
                        missing.push(name.to_string());
                    }
                    out.push_str(&rest[..=end]);
                }
            }
            rest = &rest[end + 1..];
        }
    }
}

/// Fill every string inside a JSON value. A string that is exactly one
/// `{{arg}}` takes the argument's own JSON value (numbers stay numbers).
pub fn fill_value(v: &Value, args: &Value, env: &impl Fn(&str) -> Option<String>) -> Value {
    fill_value_with(v, args, env, &mut Vec::new())
}

fn fill_value_with(v: &Value, args: &Value, env: &dyn Fn(&str) -> Option<String>, missing: &mut Vec<String>) -> Value {
    match v {
        Value::String(s) => {
            let t = s.trim();
            if t.starts_with("{{") && t.ends_with("}}") && t.matches("{{").count() == 1 && !t.ends_with("|json}}") {
                let inner = &t[2..t.len() - 2];
                let (key, default) = inner.split_once('|').map(|(k, d)| (k.trim(), Some(d.trim()))).unwrap_or((inner.trim(), None));
                return match lookup(args, key) {
                    Some(x) if !x.is_null() && !(x.is_string() && x.as_str() == Some("")) => x,
                    _ => default.map(|d| d.parse::<i64>().map(Value::from).or_else(|_| d.parse::<f64>().map(Value::from)).unwrap_or_else(|_| Value::String(d.to_string()))).unwrap_or(Value::Null),
                };
            }
            Value::String(fill_with(s, args, env, missing))
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| fill_value_with(x, args, env, missing)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), fill_value_with(x, args, env, missing))).filter(|(_, x)| !x.is_null()).collect()),
        other => other.clone(),
    }
}

/// An argument by name, or by a dotted path into one (`cover.0.id`,
/// `file.name`); `*` takes that field from every entry of a list
/// (`register.*.id` → `["1", "2"]`).
pub fn lookup(args: &Value, key: &str) -> Option<Value> {
    if let Some(v) = args.get(key) {
        return Some(v.clone());
    }
    fn walk(cur: &Value, parts: &[&str]) -> Option<Value> {
        let Some((first, rest)) = parts.split_first() else { return Some(cur.clone()) };
        match (cur, *first) {
            (Value::Array(a), "*") => Some(Value::Array(a.iter().filter_map(|x| walk(x, rest)).collect())),
            (Value::Object(o), k) => walk(o.get(k)?, rest),
            (Value::Array(a), k) => walk(a.get(k.parse::<usize>().ok()?)?, rest),
            _ => None,
        }
    }
    walk(args, &key.split('.').collect::<Vec<_>>())
}

/// Is a filled value something (for `when`): not null, false, empty or 0 items.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        Value::Number(_) => true,
    }
}

/// `/a/0/b` into a value; also accepts dotted `a.0.b`.
pub fn pointer<'a>(v: &'a Value, p: &str) -> Option<&'a Value> {
    let p = p.trim();
    if p.is_empty() {
        return Some(v);
    }
    let ptr = if p.starts_with('/') { p.to_string() } else { format!("/{}", p.replace('.', "/")) };
    v.pointer(&ptr)
}

/// Does the manifest's auth say the connection has what it needs? The keys
/// it requires, and, when it offers ways to sign in, one of them set up (else
/// the first's key is named).
pub fn missing_keys(m: &Manifest, has: impl Fn(&str) -> bool) -> Vec<String> {
    let mut out: Vec<String> = m.auth.iter().filter(|a| !a.optional && !has(&a.env)).map(|a| a.env.clone()).collect();
    if !m.connect.is_empty() && !m.connect.iter().any(|c| c.satisfied(&has)) {
        let proof = m.connect[0].proof().to_string();
        if !proof.is_empty() && !out.contains(&proof) {
            out.push(proof);
        }
    }
    out
}

/// Every value at a pointer whose segments may be `*` (each element of an
/// array): `/accountSummaries/*/propertySummaries/*`.
pub fn pointer_all<'a>(v: &'a Value, p: &str) -> Vec<&'a Value> {
    let mut at = vec![v];
    for seg in p.trim().trim_start_matches('/').split('/').filter(|s| !s.is_empty()) {
        at = at
            .into_iter()
            .flat_map(|x| -> Vec<&Value> {
                if seg == "*" {
                    x.as_array().map(|a| a.iter().collect()).unwrap_or_default()
                } else {
                    x.pointer(&format!("/{seg}")).into_iter().collect()
                }
            })
            .collect();
    }
    at
}

/// Frontmatter of a SKILL.md; the folder name stands in for a missing name.
pub fn instructions_meta(folder: &str, text: &str) -> Instructions {
    let mut meta = Instructions::default();
    if let Some(rest) = text.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            if let Ok(m) = serde_yaml::from_str::<Instructions>(&rest[..end]) {
                meta = m;
            }
        }
    }
    if meta.name.trim().is_empty() {
        meta.name = folder.to_string();
    }
    meta
}

/// Install the built-in folders. A SKILL.md is written once and then left to
/// the user; a built-in `extension.yaml` is KIQR's own and follows each
/// release (a changed copy is overwritten). Returns whether anything was written.
pub fn install_builtin(store: &crate::Store) -> Result<bool> {
    let mut wrote = false;
    for (name, skill, manifest) in BUILTIN_EXTENSIONS {
        let dir = store.home().abilities_dir().join(name);
        // Only the core folders install themselves; the rest are installed from the marketplace,
        // and once installed their manifest follows each release like a core one.
        if !CORE_FOLDERS.contains(name) && !dir.join("extension.yaml").exists() {
            continue;
        }
        if let Some(s) = skill {
            wrote |= install_builtin_text(store, &dir.join("SKILL.md"), s)?;
        }
        if let Some(m) = manifest {
            let p = dir.join("extension.yaml");
            if std::fs::read_to_string(&p).map(|t| t != *m).unwrap_or(true) {
                store.write_text(&p, m)?;
                wrote = true;
            }
        }
        // A core folder that an older KIQR installed from the marketplace keeps nothing of that origin.
        if CORE_FOLDERS.contains(name) && std::fs::remove_file(dir.join(".source")).is_ok() {
            wrote = true;
        }
    }
    Ok(wrote)
}

/// Put a built-in text file in place and keep it current across releases
/// without losing the user's edits. `.builtin` beside it remembers what was
/// installed last: a file that still matches it is replaced by the new
/// version; one the user changed is left alone. A file from before this
/// bookkeeping is replaced too, with the old text kept as `*.previous.*`
/// when it differed, so nothing is lost.
pub fn install_builtin_text(store: &crate::Store, path: &std::path::Path, body: &str) -> Result<bool> {
    let marker = path.with_file_name(".builtin");
    let current = std::fs::read_to_string(path).ok();
    let last = std::fs::read_to_string(&marker).ok();
    match (current, last) {
        (None, _) => {}
        (Some(c), _) if c == body => {
            if std::fs::read_to_string(&marker).ok().as_deref() != Some(body) {
                std::fs::write(&marker, body)?;
            }
            return Ok(false);
        }
        (Some(c), Some(l)) if c != l => {
            tracing::info!(path = %path.display(), "kept your edited version of built-in instructions; the new ones are not applied");
            return Ok(false);
        }
        (Some(c), None) => {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("txt");
            std::fs::write(path.with_file_name(format!("{stem}.previous.{ext}")), c)?;
        }
        (Some(_), Some(_)) => {}
    }
    store.write_text(path, body)?;
    std::fs::write(&marker, body)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    #[test]
    fn fill_checked_reads_the_template_not_the_arguments() {
        let env = |n: &str| (n == "SET").then(|| "v".to_string());
        let args = serde_json::json!({ "code": "const x = `${growth.monthly} and ${SET} and ${UNSET}`;", "changes": [{ "path": "a.tsx", "content": "<p>${price}</p>" }] });
        // What an argument holds goes through as written, variables included; only the template's own are filled or refused.
        let out = fill_checked("{{code}} ${SET}", &args, env).unwrap();
        assert_eq!(out, "const x = `${growth.monthly} and ${SET} and ${UNSET}`; v");
        assert_eq!(fill_checked("${UNSET}", &args, env), Err("UNSET".to_string()));
        assert_eq!(fill_checked("${UNSET|dflt} ${growth.monthly}", &args, env).unwrap(), "dflt ${growth.monthly}");
        let body = fill_value_checked(&serde_json::json!({ "tree": "{{changes}}", "ref": "${SET}" }), &args, &env).unwrap();
        assert_eq!(body["tree"][0]["content"], "<p>${price}</p>");
        assert_eq!(fill_value_checked(&serde_json::json!({ "ref": "${UNSET}" }), &args, &env), Err("UNSET".to_string()));
    }

    #[test]
    fn fill_defaults_hold_placeholders() {
        let env = |k: &str| (k == "REPO").then(|| "acme/app".to_string());
        let args = serde_json::json!({ "number": 7, "tag": "v1.2" });
        assert_eq!(fill("https://x.test/repos/{{repo|${REPO}}}/issues/{{number}}", &args, env), "https://x.test/repos/acme/app/issues/7");
        assert_eq!(fill("{{repo|${REPO}}}", &serde_json::json!({ "repo": "other/lib" }), env), "other/lib");
        assert_eq!(fill("{{title|{{tag}}}}", &args, env), "v1.2");
        assert_eq!(fill("{{title|{{tag}}}} done", &serde_json::json!({ "title": "Big" }), env), "Big done");
        assert_eq!(fill("{{a}}-{{b|x}}", &serde_json::json!({ "a": 1 }), env), "1-x");
        assert_eq!(fill("{{broken", &args, env), "{{broken");
    }

    use super::*;
    use serde_json::json;

    const STEPS: &str = r#"
id: blog
name: Publish to a blog
transport: http
hosts: [blog.test, uploads.test]
tools:
  - name: publish
    steps:
      - name: cover
        when: "{{files}}"
        for_each: files
        request: { method: POST, url: "https://blog.test/media", body_file: "{{file}}" }
        save: { id: /id, upload: /upload_url }
      - name: put
        when: "{{cover}}"
        request: { method: PUT, url: "{{cover.0.upload}}", body_file: "{{files.0}}" }
      - name: post
        request: { method: POST, url: "https://blog.test/posts", json: { title: "{{title}}", media: "{{cover.0.id}}" } }
        save: { link: /link }
    result: { text: "{{post.link}}" }
"#;

    #[test]
    fn steps_parse_and_validate() {
        let m = parse_manifest(STEPS).unwrap();
        assert_eq!(m.tools[0].provide.steps.len(), 3);
        // A URL from an earlier answer is checked when it is called, not here.
        assert!(parse_manifest(&STEPS.replace("https://blog.test/posts", "https://elsewhere.test/posts")).unwrap_err().to_string().contains("elsewhere.test"));
        assert!(parse_manifest(&STEPS.replace("name: put", "name: cover")).unwrap_err().to_string().contains("its own name"));
        assert!(parse_manifest(&STEPS.replace("name: put", "name: files")).unwrap_err().to_string().contains("taken"));
        assert!(parse_manifest(&STEPS.replace("for_each: files", "for_each: tags")).unwrap_err().to_string().contains("not 'tags'"));
        assert!(parse_manifest(&STEPS.replace("when: \"{{cover}}\"", "for_each: post")).unwrap_err().to_string().contains("not 'post'"));
        parse_manifest(&STEPS.replace("when: \"{{cover}}\"", "for_each: cover")).unwrap();
        let two = STEPS.replace(r#"body_file: "{{file}}" }"#, r#"body_file: "{{file}}", json: { a: 1 } }"#);
        assert!(parse_manifest(&two).unwrap_err().to_string().contains("more than one body"));
        let both = STEPS.replace("    steps:", "    request: { method: GET, url: \"https://blog.test/x\" }\n    steps:");
        assert!(parse_manifest(&both).unwrap_err().to_string().contains("both a request and steps"));
        let part = STEPS.replace(r#"body_file: "{{file}}" }"#, r#"multipart: [{ name: file }] }"#);
        assert!(parse_manifest(&part).unwrap_err().to_string().contains("either a value or a file"));
    }

    #[test]
    fn dotted_lookups_and_when() {
        let ctx = json!({ "title": "Hi", "cover": [{ "id": 42, "upload": "https://uploads.test/x" }], "files": [{ "name": "card.png" }], "empty": [] });
        assert_eq!(lookup(&ctx, "cover.0.id"), Some(json!(42)));
        assert_eq!(lookup(&ctx, "files.0.name"), Some(json!("card.png")));
        assert_eq!(lookup(&ctx, "cover.*.id"), Some(json!([42])));
        assert_eq!(fill_value(&json!({ "media_ids": "{{cover.*.id}}" }), &ctx, &|_| None), json!({ "media_ids": [42] }));
        assert_eq!(lookup(&ctx, "cover.1.id"), None);
        // A lone placeholder keeps its type: numbers and lists go into JSON bodies as they are.
        assert_eq!(fill_value(&json!({ "media": "{{cover.0.id}}", "all": "{{cover}}" }), &ctx, &|_| None), json!({ "media": 42, "all": [{ "id": 42, "upload": "https://uploads.test/x" }] }));
        assert_eq!(fill("{{cover.0.upload}}?n={{title}}", &ctx, |_| None), "https://uploads.test/x?n=Hi");
        assert!(truthy(&fill_value(&json!("{{files}}"), &ctx, &|_| None)));
        assert!(!truthy(&fill_value(&json!("{{empty}}"), &ctx, &|_| None)));
        assert!(!truthy(&fill_value(&json!("{{missing}}"), &ctx, &|_| None)));
    }

    #[test]
    fn env_defaults_and_urlencode() {
        let env = |k: &str| match k {
            "SITE" => Some("sc-domain:example.com".to_string()),
            "EMPTY" => Some(String::new()),
            _ => None,
        };
        assert_eq!(fill("${HOST|https://plausible.io}/api", &Value::Null, env), "https://plausible.io/api");
        assert_eq!(fill("${EMPTY|https://a.test}", &Value::Null, env), "https://a.test");
        assert_eq!(fill("/sites/${SITE|urlencode}/q", &Value::Null, env), "/sites/sc-domain%3Aexample.com/q");
        assert_eq!(fill("${NOPE|urlencode}", &Value::Null, env), "${NOPE|urlencode}");
        assert_eq!(fill("${NOPE}", &Value::Null, env), "${NOPE}");
    }

    #[test]
    fn builtin_text_follows_releases_but_keeps_edits() {
        let dir = std::env::temp_dir().join(format!("kiqr-builtin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = crate::Store::open(crate::paths::Home::at(&dir)).unwrap();
        let p = dir.join("abilities").join("x").join("SKILL.md");
        assert!(install_builtin_text(&store, &p, "v1").unwrap());
        assert!(!install_builtin_text(&store, &p, "v1").unwrap());
        assert!(install_builtin_text(&store, &p, "v2").unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "v2");
        std::fs::write(&p, "mine").unwrap();
        assert!(!install_builtin_text(&store, &p, "v3").unwrap());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "mine");
        // A copy from before the marker existed is replaced, and kept beside it.
        std::fs::remove_file(p.with_file_name(".builtin")).unwrap();
        assert!(install_builtin_text(&store, &p, "v4").unwrap());
        assert_eq!(std::fs::read_to_string(p.with_file_name("SKILL.previous.md")).unwrap(), "mine");
        let _ = std::fs::remove_dir_all(&dir);
    }

    const WP: &str = r#"
id: wordpress
name: WordPress
summary: Publish articles to a WordPress site.
transport: http
auth:
  - { env: WP_USER, label: User }
  - { env: WP_APP_PASSWORD, label: Application password, help: https://example.com/wp-admin/profile.php }
hosts: [myblog.com]
tools:
  - name: publish_article
    label: Publishing to WordPress
    description: Create a post.
    input: { type: object, properties: { title: { type: string }, html: { type: string } }, required: [title, html] }
    request:
      method: POST
      url: https://myblog.com/wp-json/wp/v2/posts
      headers: { Authorization: "Basic ${WP_BASIC}" }
      json: { title: "{{title}}", content: "{{html}}", status: "{{status|draft}}" }
    result: { text: /link }
kinds:
  - { kind: wordpress, label: Blog, publish: publish_article, account_is_target: true }
"#;

    #[test]
    fn learn_reads_with_its_own_reading_tools() {
        let read = WP.replace("kinds:", "  - { name: get_page, label: Reading a page, description: A page., request: { method: GET, url: \"https://myblog.com/{{path}}\" } }\nkinds:");
        let ok = format!("{read}learn:\n  label: Learn from the blog\n  documents:\n    - {{ tool: get_page, args: {{ path: about }}, title: About, url: \"https://myblog.com/about\" }}\n    - {{ tool: get_page, title: \"{{{{value}}}}\", url: \"https://myblog.com/{{{{value}}}}\", each: {{ list: /pages, value: /slug, only: [.md], tool: get_page, arg: path }} }}\n");
        let m = parse_manifest(&ok).unwrap_or_else(|e| panic!("{e:#}"));
        let l = m.learn.as_ref().unwrap();
        assert_eq!(l.documents.len(), 2);
        assert_eq!(l.documents[1].each.as_ref().unwrap().limit(), 20);
        assert!(parse_manifest(WP).unwrap().learn.is_none());
        // A tool it does not define, an each without its arg, and a tool that acts are refused.
        assert!(parse_manifest(&ok.replace("tool: get_page, args", "tool: nope, args")).unwrap_err().to_string().contains("does not define"));
        assert!(parse_manifest(&ok.replace(", arg: path", "")).unwrap_err().to_string().contains("arg"));
        let acting = format!("{WP}learn:\n  documents:\n    - {{ tool: publish_article, title: x, url: \"https://myblog.com\" }}\n").replace("    description: Create a post.", "    description: Create a post.\n    role: publish");
        assert!(parse_manifest(&acting).unwrap_err().to_string().contains("may only read"));
    }

    #[test]
    fn service_blocks_allow_and_kinds_parse() {
        // A service as a string, or as a block with how the connect grid shows it.
        let m = parse_manifest(&WP.replace("id: wordpress", "id: wordpress\nservice: { id: blogs, name: Blogs, icon: newspaper, group: website, profile_hosts: [blog.example] }\nallow: { publish: auto, learn: off }")).unwrap();
        assert_eq!((m.service(), m.service.name.as_str(), m.service.group.as_str()), ("blogs", "Blogs", "website"));
        assert_eq!(m.service_card().profile_hosts, vec!["blog.example".to_string()]);
        assert_eq!((m.allow["publish"], m.allow["learn"]), (crate::accounts::Allow::Auto, crate::accounts::Allow::Off));
        let plain = parse_manifest(&WP.replace("id: wordpress", "id: wordpress\nservice: blogs")).unwrap();
        assert_eq!((plain.service(), plain.service_card().name.as_str()), ("blogs", plain.name.as_str()), "no block: the ability's own name");
        assert!(parse_manifest(&WP.replace("id: wordpress", "id: wordpress\nservice: { id: blogs, group: nowhere }")).unwrap_err().to_string().contains("group"));
        assert!(parse_manifest(&WP.replace("id: wordpress", "id: wordpress\nallow: { shout: auto }")).unwrap_err().to_string().contains("allow"));
        assert!(parse_manifest(&WP.replace("id: wordpress", "id: wordpress\nallow: { learn: ask }")).unwrap_err().to_string().contains("never asks"));
        // A kind's target fields; the account being the spot has none, and a section is one of three.
        let sub = WP.replace("account_is_target: true", "section: community, target: { sub: { label: Sub, pattern: \"^[a-z]+$\" } }, target_from: { post_url: \"myblog.com/{sub}/\" }");
        let k = &parse_manifest(&sub).unwrap().kinds[0];
        assert_eq!((k.section(), k.required()), ("community", vec!["sub"]));
        assert!(parse_manifest(&sub.replace("{sub}/", "{nope}/")).unwrap_err().to_string().contains("target_from"));
        assert!(parse_manifest(&sub.replace("section: community", "section: forum")).unwrap_err().to_string().contains("section"));
        assert!(parse_manifest(&sub.replace("section: community", "account_is_target: true")).unwrap_err().to_string().contains("no target fields"));
        assert!(parse_manifest(&WP.replace("kind: wordpress", "kind: list")).is_err(), "list is the Audience's own kind");
    }

    #[test]
    fn manifests_parse_and_validate() {
        let m = parse_manifest(WP).unwrap();
        assert_eq!(m.tools.len(), 1);
        assert_eq!(m.kinds[0].publish, "publish_article");
        assert_eq!(missing_keys(&m, |k| k == "WP_USER"), vec!["WP_APP_PASSWORD".to_string()]);
        let bad = WP.replace("hosts: [myblog.com]", "hosts: [other.com]");
        assert!(parse_manifest(&bad).unwrap_err().to_string().contains("not in hosts"));
        let bad = WP.replace("id: wordpress", "id: Word Press");
        assert!(parse_manifest(&bad).is_err());
        let bad = WP.replace("publish: publish_article", "publish: nope");
        assert!(parse_manifest(&bad).unwrap_err().to_string().contains("does not define"));
        for (name, _, text) in BUILTIN_EXTENSIONS {
            let m = parse_manifest(text.unwrap()).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!m.id.is_empty(), "{name}");
        }
    }

    #[test]
    fn templates_fill_from_args_and_env() {
        let args = json!({ "title": "Hi", "n": 3, "empty": "" });
        let env = |k: &str| if k == "KEY" { Some("s3cret".to_string()) } else { None };
        assert_eq!(fill("t={{title}} n={{n}} d={{missing|x}} e={{empty|dflt}} k=${KEY} u=${NOPE}", &args, env), "t=Hi n=3 d=x e=dflt k=s3cret u=${NOPE}");
        let v = fill_value(&json!({ "a": "{{n}}", "b": "{{title}}!", "c": "{{missing}}", "d": { "k": "${KEY}" } }), &args, &env);
        assert_eq!(v, json!({ "a": 3, "b": "Hi!", "d": { "k": "s3cret" } }));
        assert_eq!(fill_value(&json!({ "w": "{{width|1600}}" }), &json!({}), &env), json!({ "w": 1600 }));
        // `|json` for frontmatter: quotes and newlines escaped, lists as lists, null when missing.
        let fm = json!({ "title": "Say \"hi\"\nnow", "tags": ["a", "b"], "empty": "" });
        assert_eq!(fill("title: {{title|json}}\ntags: {{tags|json}}\nimage: {{image|json}} {{empty|json}}", &fm, env), "title: \"Say \\\"hi\\\"\\nnow\"\ntags: [\"a\",\"b\"]\nimage: null null");
        assert_eq!(fill_value(&json!({ "c": "{{title|json}}" }), &json!({ "title": "x" }), &env), json!({ "c": "\"x\"" }));
        let p = Provide { map: [("orientation".to_string(), [("landscape".to_string(), json!("1536x1024")), ("*".to_string(), json!("1024x1024"))].into_iter().collect())].into_iter().collect(), ..Default::default() };
        assert_eq!(p.mapped_args(&json!({ "orientation": "landscape" }))["orientation"], "1536x1024");
        assert_eq!(p.mapped_args(&json!({}))["orientation"], "1024x1024");
        assert_eq!(pointer(&json!({ "images": [{ "url": "u" }] }), "/images/0/url"), Some(&json!("u")));
        assert_eq!(pointer(&json!({ "a": { "b": 1 } }), "a.b"), Some(&json!(1)));
        let u = url::Url::parse("https://api.fal.run/x").unwrap();
        assert!(host_allowed(&u, &["fal.run".into()]));
        assert!(!host_allowed(&u, &["al.run".into()]));
        assert!(!host_allowed(&u, &["example.com".into()]));
    }

    #[test]
    fn instructions_frontmatter() {
        let m = instructions_meta("wp", "---\nname: Publish to WordPress\ndescription: When asked to put an article on the blog.\nabilities: [publish]\n---\n\nBody");
        assert_eq!(m.description, "When asked to put an article on the blog.");
        assert_eq!(m.abilities, vec!["publish"]);
        assert_eq!(instructions_meta("plain", "no frontmatter").name, "plain");
    }

    #[test]
    fn old_homes_move_their_folders() {
        let dir = std::env::temp_dir().join(format!("kiqr-move-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let home = crate::paths::Home::at(&dir);
        std::fs::create_dir_all(dir.join("skills").join("x")).unwrap();
        std::fs::write(dir.join("skills").join("x").join("SKILL.md"), "---\nname: X\n---\nbody").unwrap();
        std::fs::create_dir_all(dir.join("projects").join("p").join("skills").join("y")).unwrap();
        // The store creates an empty abilities/ on open; that must not block the move.
        std::fs::create_dir_all(dir.join("abilities")).unwrap();
        assert_eq!(home.ability_folder_moves().len(), 2);
        assert_eq!(home.move_ability_folders().unwrap(), 2);
        assert!(dir.join("abilities").join("x").join("SKILL.md").exists());
        assert!(dir.join("projects").join("p").join("abilities").join("y").is_dir());
        assert!(!dir.join("skills").exists());
        assert!(home.ability_folder_moves().is_empty());
        let reg = Registry::load(&home, &[]);
        let x = reg.entry(Some("p"), "x").unwrap();
        assert_eq!(x.id(), "x");
        assert_eq!(x.instructions.as_ref().unwrap().name, "X");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn attached_folders_are_not_abilities() {
        let a = attached_folders();
        for f in ["crawl-site", "extract-facts", "build-profile", "business-brief", "social-posts", "marketing-plan", "seemo-agent"] {
            assert!(a.contains(&f), "{f}");
        }
        assert_eq!(core_of_folder("business-brief").map(|c| c.id), Some("business-profile"));
        assert!(is_core_id("kiqr-render") && is_core_id("make-images") && is_core_id("browser") && is_core_id("tasks") && !is_core_id("fal-image"));
        assert!(shipped("browser").is_some_and(|(folder, _, _, m)| folder == "browser" && m.service().eq("browser") && m.service_card().group == "sites"));
    }
}
