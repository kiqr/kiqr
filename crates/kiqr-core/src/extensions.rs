//! Extensions: one folder, one manifest. A folder under `skills/` (global, or
//! a project's own) may hold `SKILL.md` (a skill: how Seemo does something),
//! `extension.yaml` (a connection: what provides an ability, or a few tools of
//! its own, or a channel kind), or both (a pack, e.g. "Publish to WordPress").
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
    /// A link for the Connections page.
    pub homepage: String,
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
        }
    }
}

/// SKILL.md frontmatter: when Seemo should use it and what it needs.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub abilities: Vec<String>,
    pub channels: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "scope", content = "project")]
pub enum Scope {
    Global,
    Project(String),
}

/// One folder under `skills/`.
#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    /// Folder name.
    pub name: String,
    pub dir: PathBuf,
    pub scope: Scope,
    pub builtin: bool,
    pub skill: Option<SkillMeta>,
    pub manifest: Option<Manifest>,
    /// Why the manifest could not be used, if so.
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub entries: Vec<Entry>,
}

/// Built-in folders: name → (SKILL.md, extension.yaml). Installed on first run
/// so users see and can edit them; a user's copy wins.
pub const BUILTIN_EXTENSIONS: &[(&str, Option<&str>, Option<&str>)] = &[
    ("make-images", Some(include_str!("../../../skills/make-images/SKILL.md")), Some(include_str!("../../../skills/make-images/extension.yaml"))),
    ("email-smtp", None, Some(include_str!("../../../skills/email-smtp/extension.yaml"))),
    ("fal-image", None, Some(include_str!("../../../skills/fal-image/extension.yaml"))),
    ("openai-image", None, Some(include_str!("../../../skills/openai-image/extension.yaml"))),
];

pub fn builtin_names() -> Vec<&'static str> {
    BUILTIN_EXTENSIONS.iter().map(|(n, _, _)| *n).collect()
}

impl Registry {
    /// Every folder in the global `skills/` and in each project's `skills/`.
    pub fn load(home: &Home) -> Registry {
        let mut entries = Vec::new();
        entries.extend(scan(&home.skills_dir(), Scope::Global));
        if let Ok(rd) = std::fs::read_dir(home.projects_dir()) {
            for p in rd.flatten() {
                if !p.path().is_dir() {
                    continue;
                }
                let slug = p.file_name().to_string_lossy().to_string();
                entries.extend(scan(&home.project_skills_dir(&slug), Scope::Project(slug)));
            }
        }
        Registry { entries }
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

    /// The connections (entries with a usable manifest) a project sees.
    pub fn connections(&self, slug: &str) -> Vec<(&Entry, &Manifest)> {
        self.visible(slug).into_iter().filter_map(|e| e.manifest.as_ref().filter(|_| e.error.is_none()).map(|m| (e, m))).collect()
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
        let skill = std::fs::read_to_string(path.join("SKILL.md")).ok().map(|t| skill_meta(&name, &t));
        let (manifest, error) = match std::fs::read_to_string(path.join("extension.yaml")) {
            Ok(text) => match parse_manifest(&text) {
                Ok(m) => (Some(m), None),
                Err(err) => (None, Some(err.to_string())),
            },
            Err(_) => (None, None),
        };
        if skill.is_none() && manifest.is_none() && error.is_none() {
            continue;
        }
        let builtin = builtin_names().contains(&name.as_str()) || crate::extract::builtin_skill(&name).is_some();
        out.push(Entry { name, dir: path, scope: scope.clone(), builtin, skill, manifest, error });
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
        if h.trim().is_empty() || h.contains('/') || h.contains(':') {
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
            let url = url::Url::parse(&fill(&r.url, &Value::Null, |_| Some("x".into()))).with_context(|| format!("{what}: the request url does not parse"))?;
            if !host_allowed(&url, &m.hosts) {
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

/// Frontmatter of a SKILL.md: name, description, abilities, channels.
pub fn skill_meta(folder: &str, text: &str) -> SkillMeta {
    let mut meta = SkillMeta::default();
    if let Some(rest) = text.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            if let Ok(m) = serde_yaml::from_str::<SkillMeta>(&rest[..end]) {
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
        let dir = store.home().skills_dir().join(name);
        if let Some(s) = skill {
            let p = dir.join("SKILL.md");
            if !p.exists() {
                store.write_text(&p, s)?;
                wrote = true;
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
    fn skill_frontmatter() {
        let m = skill_meta("wp", "---\nname: Publish to WordPress\ndescription: When asked to put an article on the blog.\nabilities: [publish]\n---\n\nBody");
        assert_eq!(m.description, "When asked to put an article on the blog.");
        assert_eq!(m.abilities, vec!["publish"]);
        assert_eq!(skill_meta("plain", "no frontmatter").name, "plain");
    }
}
