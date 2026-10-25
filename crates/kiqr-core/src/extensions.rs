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
    /// Rust inside KIQR (`kiqr-render`, `kiqr-smtp`).
    Builtin,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthKey {
    /// Environment variable name; the value lives in `.env`.
    pub env: String,
    pub label: String,
    /// Where to get it.
    pub help: String,
    /// Optional: a key the connection works without.
    pub optional: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    /// JSON body; strings may hold `{{arg}}` and `${ENV}` placeholders.
    pub json: Option<Value>,
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
    #[serde(flatten)]
    pub provide: Provide,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelKind {
    pub kind: String,
    pub label: String,
    /// Destination fields: name → {type, label, help}.
    pub destination: BTreeMap<String, Value>,
    /// The extra tool, or the ability, that performs a publish.
    pub publish: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub summary: String,
    pub transport: Transport,
    pub auth: Vec<AuthKey>,
    pub hosts: Vec<String>,
    /// ability id → how this connection provides it.
    pub provides: BTreeMap<String, Provide>,
    pub tools: Vec<ExtraTool>,
    pub channels: Vec<ChannelKind>,
    // MCP transport.
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<String>,
    pub cwd: Option<String>,
    /// MCP: which of the server's tools to offer raw (`all`, or names). Empty: none.
    pub expose: Vec<String>,
    /// A link for the Abilities page.
    pub homepage: String,
    /// Marketplace shelves: images, video, music, email, publishing, browser, tools.
    pub categories: Vec<String>,
    /// A lucide icon name for the card; the ability's icon when empty.
    pub icon: String,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: "0.1.0".into(),
            summary: String::new(),
            transport: Transport::Http,
            auth: vec![],
            hosts: vec![],
            provides: Default::default(),
            tools: vec![],
            channels: vec![],
            command: String::new(),
            args: vec![],
            env: vec![],
            cwd: None,
            expose: vec![],
            homepage: String::new(),
            categories: vec![],
            icon: String::new(),
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
    pub channels: Vec<String>,
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
/// installed on every start; the rest wait in the marketplace until the
/// user installs them. A user's copy of a SKILL.md wins.
pub const BUILTIN_EXTENSIONS: &[(&str, Option<&str>, Option<&str>)] = &[
    ("make-images", Some(include_str!("../../../abilities/make-images/SKILL.md")), Some(include_str!("../../../abilities/make-images/extension.yaml"))),
    ("email-smtp", None, Some(include_str!("../../../abilities/email-smtp/extension.yaml"))),
    ("fal-image", None, Some(include_str!("../../../abilities/fal-image/extension.yaml"))),
    ("openai-image", None, Some(include_str!("../../../abilities/openai-image/extension.yaml"))),
];

/// On without installing: the built-in renderer (folder `make-images`, id
/// `kiqr-render`). Web reading is a built-in tool, not a folder.
pub const CORE_FOLDERS: &[&str] = &["make-images"];
pub const CORE_IDS: &[&str] = &["kiqr-render"];

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

    /// Every installed folder with this ability id, paused or not, any scope.
    pub fn installed(&self, id: &str) -> Vec<&Entry> {
        self.entries.iter().filter(|e| e.id() == id).collect()
    }

    pub fn connection(&self, slug: &str, id: &str) -> Option<(&Entry, &Manifest)> {
        self.connections(slug).into_iter().find(|(_, m)| m.id == id)
    }

    /// Which connection serves an ability for this project: the configured
    /// one if it is ready, else a ready built-in, else the only ready one.
    pub fn provider<'a>(&'a self, slug: &str, ability: &str, configured: Option<&str>, ready: impl Fn(&Manifest) -> bool) -> Option<(&'a Entry, &'a Manifest)> {
        let candidates: Vec<(&Entry, &Manifest)> = self.connections(slug).into_iter().filter(|(_, m)| m.provides.contains_key(ability) && ready(m)).collect();
        if let Some(c) = configured {
            if let Some(found) = candidates.iter().find(|(_, m)| m.id == c) {
                return Some(*found);
            }
        }
        if let Some(found) = candidates.iter().find(|(_, m)| m.transport == Transport::Builtin) {
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
        if t.input != Value::Null && t.input["type"] != "object" {
            bail!("tool '{}' input must be a JSON schema object", t.name);
        }
        check_provide(m, &t.provide, &format!("tool '{}'", t.name))?;
    }
    for (ability, p) in &m.provides {
        if crate::abilities::find(ability).is_none() {
            bail!("'{ability}' is not an ability (image, video, music, email)");
        }
        check_provide(m, p, &format!("ability '{ability}'"))?;
    }
    match m.transport {
        Transport::Mcp if m.command.trim().is_empty() => bail!("an mcp connection needs a command"),
        Transport::Http if m.hosts.is_empty() && (!m.provides.is_empty() || !m.tools.is_empty()) => bail!("an http connection must list the hosts it may call"),
        _ => {}
    }
    for n in m.env.iter().chain(m.tools.iter().flat_map(|t| t.provide.command.iter().flat_map(|c| c.env.iter()))) {
        if !valid_env_name(n) {
            bail!("env '{n}' is not a variable name");
        }
    }
    for c in &m.channels {
        if c.kind.trim().is_empty() || c.publish.trim().is_empty() {
            bail!("a channel needs a kind and a publish tool");
        }
        if !m.tools.iter().any(|t| t.name == c.publish) && !m.provides.contains_key(&c.publish) {
            bail!("channel '{}' publishes with '{}', which this connection does not define", c.kind, c.publish);
        }
    }
    Ok(())
}

fn check_provide(m: &Manifest, p: &Provide, what: &str) -> Result<()> {
    match m.transport {
        Transport::Http => {
            let r = p.request.as_ref().with_context(|| format!("{what} needs a request"))?;
            // `${VAR}` in the URL stands for a site the project names; the host is checked at call time then.
            let url = url::Url::parse(&fill(&r.url, &Value::Null, |_| Some("https://placeholder.invalid".into()))).with_context(|| format!("{what}: the request url does not parse"))?;
            let per_project = m.hosts.iter().any(|h| h.starts_with("${"));
            if !per_project && !host_allowed(&url, &m.hosts) {
                bail!("{what} calls {}, which is not in hosts", url.host_str().unwrap_or(""));
            }
            if !matches!(r.method.to_ascii_uppercase().as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE") {
                bail!("{what}: method '{}' is not one of GET, POST, PUT, PATCH, DELETE", r.method);
            }
        }
        Transport::Command => {
            if p.command.as_ref().map(|c| c.command.trim().is_empty()).unwrap_or(true) {
                bail!("{what} needs a command");
            }
        }
        Transport::Mcp => {
            if p.tool.as_ref().map(|t| t.trim().is_empty()).unwrap_or(true) {
                bail!("{what} needs the server's tool name");
            }
        }
        Transport::Builtin => {}
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

/// Fill `{{arg}}` / `{{arg|default}}` from the arguments and `${ENV}` from
/// `env`. A missing argument with no default becomes empty; a missing
/// variable stays as written so the caller can refuse.
pub fn fill(template: &str, args: &Value, env: impl Fn(&str) -> Option<String>) -> String {
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
            let Some(end) = rest.find("}}") else {
                out.push_str(rest);
                return out;
            };
            let inner = &rest[2..end];
            let (key, default) = inner.split_once('|').map(|(k, d)| (k.trim(), Some(d.trim()))).unwrap_or((inner.trim(), None));
            let v = args.get(key).filter(|v| !v.is_null());
            match v {
                Some(Value::String(s)) if !s.is_empty() => out.push_str(s),
                Some(Value::String(_)) | None => out.push_str(default.unwrap_or("")),
                Some(other) => out.push_str(&other.to_string()),
            }
            rest = &rest[end + 2..];
        } else {
            let Some(end) = rest.find('}') else {
                out.push_str(rest);
                return out;
            };
            let name = &rest[2..end];
            match env(name) {
                Some(v) if valid_env_name(name) => out.push_str(&v),
                _ => out.push_str(&rest[..=end]),
            }
            rest = &rest[end + 1..];
        }
    }
}

/// Fill every string inside a JSON value. A string that is exactly one
/// `{{arg}}` takes the argument's own JSON value (numbers stay numbers).
pub fn fill_value(v: &Value, args: &Value, env: &impl Fn(&str) -> Option<String>) -> Value {
    match v {
        Value::String(s) => {
            let t = s.trim();
            if t.starts_with("{{") && t.ends_with("}}") && t.matches("{{").count() == 1 {
                let inner = &t[2..t.len() - 2];
                let (key, default) = inner.split_once('|').map(|(k, d)| (k.trim(), Some(d.trim()))).unwrap_or((inner.trim(), None));
                return match args.get(key) {
                    Some(x) if !x.is_null() && !(x.is_string() && x.as_str() == Some("")) => x.clone(),
                    _ => default.map(|d| d.parse::<i64>().map(Value::from).or_else(|_| d.parse::<f64>().map(Value::from)).unwrap_or_else(|_| Value::String(d.to_string()))).unwrap_or(Value::Null),
                };
            }
            Value::String(fill(s, args, env))
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| fill_value(x, args, env)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), fill_value(x, args, env))).filter(|(_, x)| !x.is_null()).collect()),
        other => other.clone(),
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

/// Does the manifest's auth say the connection has what it needs?
pub fn missing_keys(m: &Manifest, has: impl Fn(&str) -> bool) -> Vec<String> {
    m.auth.iter().filter(|a| !a.optional && !has(&a.env)).map(|a| a.env.clone()).collect()
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
    use super::*;
    use serde_json::json;

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
channels:
  - { kind: wordpress, label: WordPress blog, publish: publish_article, destination: { site: { type: string, label: Site } } }
"#;

    #[test]
    fn manifests_parse_and_validate() {
        let m = parse_manifest(WP).unwrap();
        assert_eq!(m.tools.len(), 1);
        assert_eq!(m.channels[0].publish, "publish_article");
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
        for f in ["crawl-site", "extract-facts", "build-profile", "business-brief", "social-posts", "seemo-agent"] {
            assert!(a.contains(&f), "{f}");
        }
        assert_eq!(core_of_folder("business-brief").map(|c| c.id), Some("business-profile"));
        assert!(is_core_id("kiqr-render") && is_core_id("make-images") && is_core_id("tasks") && !is_core_id("fal-image"));
    }
}
