//! The Vault: credentials (logins, API keys, signed-in apps, mail servers,
//! anything with fields), kept for the whole machine in one encrypted file,
//! `vault/vault.enc`, never committed. A credential is scoped to one or more
//! domains, the first being its default (its icon is that domain's favicon).
//!
//! The Vault knows nothing else: not abilities, not channels, not projects.
//! Which ability may use a credential is the project's grant (`access.rs`);
//! which credential a channel publishes as is the channel's. Here a credential
//! is stored and, when asked, a field is handed back.
//!
//! The key: `KIQR_VAULT_KEY_FILE` or a systemd credential (`kiqr-vault`), else
//! `KIQR_VAULT_KEY` (read once, then removed from the process environment so
//! no child inherits it), else the OS keyring when one is reachable, else
//! `vault/key` (0600). With the key in a file next to the vault, encryption
//! keeps reusable secrets from leaking through a commit, a backup or a copy of
//! the vault alone; the machine's user account and disk encryption remain the
//! boundary. Settings says which key is in use.
//!
//! Values never leave through `list()`/`view()`. A login's password and
//! one-time code are `runner_only`: only the sign-in runner gets them
//! (`Caller::Runner`), never a request template.

use crate::paths::Home;
use anyhow::{anyhow, bail, Context, Result};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAGIC: &[u8; 6] = b"KIQRV1";
const KEYRING_SERVICE: &str = "kiqr";
const KEYRING_USER: &str = "vault";

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    #[default]
    Text,
    Secret,
    Totp,
    Url,
    Number,
}

impl FieldKind {
    /// Shown to the user (a username, a host); secrets and codes never are.
    pub fn visible(self) -> bool {
        matches!(self, FieldKind::Text | FieldKind::Url | FieldKind::Number)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Field {
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub kind: FieldKind,
    #[serde(default)]
    pub value: String,
    /// Only the sign-in runner may read it (a login's password and one-time code).
    #[serde(default)]
    pub runner_only: bool,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Credential {
    pub id: String,
    pub label: String,
    /// `login`, `api_key`, `oauth`, `smtp` or `custom`: which fields it started with.
    pub template: String,
    /// Where it may be used; the first is the default.
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub fields: Vec<Field>,
    /// The kiqr.cloud provider a signed-in app came from (`oauth`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Where a login signs in, for a site no ability names one for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_url: Option<String>,
    #[serde(default)]
    pub notes: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Credential {
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }
    fn value(&self, name: &str) -> Option<&str> {
        self.field(name).map(|f| f.value.as_str()).filter(|v| !v.is_empty())
    }
    /// Who it is, in words: the username, sender or account it signs in as.
    pub fn identity(&self) -> Option<String> {
        ["username", "from", "account", "email", "user"].iter().find_map(|n| self.value(n).filter(|_| self.field(n).is_some_and(|f| f.kind.visible())).map(str::to_string))
    }
    /// The scopes a signed-in app was granted.
    pub fn scopes(&self) -> Vec<String> {
        self.value("scopes").map(|s| s.split(|c: char| c == ' ' || c == ',').filter(|s| !s.is_empty()).map(str::to_string).collect()).unwrap_or_default()
    }
    /// Required fields still empty.
    pub fn missing(&self) -> Vec<String> {
        self.fields.iter().filter(|f| !f.optional && f.value.trim().is_empty()).map(|f| f.name.clone()).collect()
    }
    /// Whether a value of this credential may be sent to `host`: only to its own domains.
    pub fn allows_host(&self, host: &str) -> bool {
        self.domains.iter().any(|d| same_site(d, host))
    }
    /// Whether it fits what an ability asks for: one of the templates, a
    /// domain of one of the hosts (a mail server fits on its template alone;
    /// an unresolved `${SITE}` host fits any), and for a sign-in every scope.
    pub fn fits(&self, templates: &[String], hosts: &[String], scopes: &[String]) -> bool {
        if !templates.is_empty() && !templates.iter().any(|t| *t == self.template) {
            return false;
        }
        if self.template == "oauth" && !scopes.is_empty() {
            let have = self.scopes();
            if !scopes.iter().all(|s| have.contains(s)) {
                return false;
            }
        }
        let fixed: Vec<&String> = hosts.iter().filter(|h| !h.contains("${")).collect();
        self.template == "smtp" || fixed.len() < hosts.len() || fixed.is_empty() || fixed.iter().any(|h| self.allows_host(h))
    }
    /// A value for a call: the first set of `name`'s alternatives (`token|key`), never a runner-only field.
    pub fn call_value(&self, name: &str) -> Option<String> {
        name.split('|').map(str::trim).find_map(|n| self.field(n).filter(|f| !f.runner_only).map(|f| f.value.clone()).filter(|v| !v.is_empty()))
    }
    pub fn default_domain(&self) -> Option<&str> {
        self.domains.first().map(String::as_str)
    }
    pub fn view(&self) -> CredentialView {
        CredentialView {
            id: self.id.clone(),
            label: self.label.clone(),
            template: self.template.clone(),
            domains: self.domains.clone(),
            identity: self.identity(),
            provider: self.provider.clone(),
            scopes: self.scopes(),
            login_url: self.login_url.clone(),
            notes: self.notes.clone(),
            missing: self.missing(),
            fields: self
                .fields
                .iter()
                .map(|f| FieldView {
                    name: f.name.clone(),
                    label: if f.label.is_empty() { f.name.clone() } else { f.label.clone() },
                    kind: f.kind,
                    set: !f.value.is_empty(),
                    value: f.kind.visible().then(|| f.value.clone()).filter(|v| !v.is_empty()),
                    optional: f.optional,
                })
                .collect(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// A credential as anyone outside the Vault sees it: no secret values.
#[derive(Clone, Debug, Serialize)]
pub struct CredentialView {
    pub id: String,
    pub label: String,
    pub template: String,
    pub domains: Vec<String>,
    pub identity: Option<String>,
    pub provider: Option<String>,
    pub scopes: Vec<String>,
    pub login_url: Option<String>,
    pub notes: String,
    pub missing: Vec<String>,
    pub fields: Vec<FieldView>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FieldView {
    pub name: String,
    pub label: String,
    pub kind: FieldKind,
    pub set: bool,
    /// Only for visible kinds (a username, a host).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub optional: bool,
}

/// A template: the fields a new credential starts with.
#[derive(Clone, Debug, Serialize)]
pub struct Template {
    pub id: &'static str,
    pub label: &'static str,
    pub fields: Vec<Field>,
}

fn f(name: &str, label: &str, kind: FieldKind, optional: bool, runner_only: bool) -> Field {
    Field { name: name.into(), label: label.into(), kind, value: String::new(), runner_only, optional }
}

pub fn templates() -> Vec<Template> {
    use FieldKind::*;
    vec![
        Template {
            id: "login",
            label: "Login",
            // A password is optional: a login signed in by hand in the live view has none.
            fields: vec![f("username", "Username or email", Text, false, false), f("password", "Password", Secret, true, true), f("totp", "Two-factor key", Totp, true, true)],
        },
        Template { id: "api_key", label: "API key", fields: vec![f("key", "API key", Secret, false, false)] },
        // A user name and a password made for an API (WordPress application passwords): sent with calls, unlike a login's.
        Template { id: "app_password", label: "App password", fields: vec![f("username", "Username", Text, false, false), f("password", "App password", Secret, false, false)] },
        Template {
            id: "oauth",
            label: "Signed-in app",
            fields: vec![f("account", "Account", Text, true, false), f("token", "Access token", Secret, false, false), f("refresh", "Refresh token", Secret, true, false), f("expires_at", "Expires", Number, true, false), f("scopes", "Access", Text, true, false)],
        },
        Template {
            id: "smtp",
            label: "Mail server",
            fields: vec![
                f("host", "Server", Text, false, false),
                f("port", "Port", Number, true, false),
                f("security", "Security", Text, true, false),
                f("username", "Username", Text, true, false),
                f("password", "Password", Secret, true, false),
                f("from", "Send as", Text, false, false),
            ],
        },
        Template { id: "custom", label: "Something else", fields: vec![] },
    ]
}

pub fn template(id: &str) -> Option<Template> {
    templates().into_iter().find(|t| t.id == id)
}

/// Who is asking for a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caller {
    /// The sign-in runner, typing into a page's own fields.
    Runner,
    /// A request, command or program an ability runs.
    Call,
}

/// A new credential, or the changes to one. Values for `fields` are set by name.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Draft {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub domains: Option<Vec<String>>,
    /// Field values by name; an empty value clears a field.
    #[serde(default)]
    pub values: std::collections::BTreeMap<String, String>,
    /// Extra fields for a custom credential (or a template's), without values.
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub login_url: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KeySource {
    KeyFile,
    SystemdCredential,
    Env,
    Keyring,
    LocalFile,
}

impl KeySource {
    pub fn describe(&self) -> &'static str {
        match self {
            KeySource::KeyFile => "a key file named by KIQR_VAULT_KEY_FILE",
            KeySource::SystemdCredential => "a systemd credential",
            KeySource::Env => "KIQR_VAULT_KEY",
            KeySource::Keyring => "this computer's keyring",
            KeySource::LocalFile => "a file on this machine, next to the vault",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub unlocked: bool,
    pub key: Option<KeySource>,
    pub key_text: String,
    /// Why the vault cannot be read, when it cannot.
    pub problem: Option<String>,
    pub count: usize,
}

#[derive(Default, Serialize, Deserialize)]
struct Contents {
    #[serde(default)]
    credentials: Vec<Credential>,
}

pub struct Vault {
    file: PathBuf,
    key: Option<[u8; 32]>,
    source: Option<KeySource>,
    problem: Option<String>,
    /// Load-modify-save happens under this lock: token refreshes and edits never race.
    lock: Mutex<()>,
}

impl Vault {
    /// Open the machine's vault, finding (or, for a new vault, making) its key.
    pub fn open(home: &Home) -> Self {
        let dir = home.vault_dir();
        let file = home.vault_file();
        let _ = std::fs::create_dir_all(&dir);
        private_dir(&dir);
        let exists = file.exists();
        match find_key(home, exists) {
            Ok((key, source)) => {
                let v = Self { file, key: Some(key), source: Some(source), problem: None, lock: Mutex::new(()) };
                // A key that does not open the vault is as good as none.
                if exists {
                    if let Err(e) = v.load() {
                        return Self { problem: Some(format!("{e:#}")), key: None, ..v };
                    }
                }
                v
            }
            Err(e) => Self { file, key: None, source: None, problem: Some(format!("{e:#}")), lock: Mutex::new(()) },
        }
    }

    /// A vault with a given key, for tests and tools.
    pub fn with_key(file: PathBuf, key: [u8; 32]) -> Self {
        Self { file, key: Some(key), source: Some(KeySource::KeyFile), problem: None, lock: Mutex::new(()) }
    }

    pub fn status(&self) -> Status {
        Status {
            unlocked: self.key.is_some(),
            key: self.source.clone(),
            key_text: self.source.as_ref().map(|s| s.describe().to_string()).unwrap_or_else(|| "none".into()),
            problem: self.problem.clone(),
            count: self.load().map(|c| c.credentials.len()).unwrap_or(0),
        }
    }

    fn cipher(&self) -> Result<XChaCha20Poly1305> {
        let key = self.key.ok_or_else(|| anyhow!("the vault is locked: {}", self.problem.as_deref().unwrap_or("no key")))?;
        Ok(XChaCha20Poly1305::new(&key.into()))
    }

    fn load(&self) -> Result<Contents> {
        let cipher = self.cipher()?;
        let bytes = match std::fs::read(&self.file) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Contents::default()),
            Err(e) => return Err(e).context("reading the vault"),
        };
        if bytes.len() < MAGIC.len() + 24 || &bytes[..MAGIC.len()] != MAGIC {
            bail!("the vault file is not one KIQR wrote");
        }
        let nonce = XNonce::try_from(&bytes[MAGIC.len()..MAGIC.len() + 24]).map_err(|_| anyhow!("the vault file is damaged"))?;
        let plain = cipher.decrypt(&nonce, &bytes[MAGIC.len() + 24..]).map_err(|_| anyhow!("the vault's key does not open it"))?;
        serde_json::from_slice(&plain).context("reading the vault's contents")
    }

    fn save(&self, c: &Contents) -> Result<()> {
        let cipher = self.cipher()?;
        let nonce_bytes: [u8; 24] = random();
        let nonce = XNonce::from(nonce_bytes);
        let plain = serde_json::to_vec(c)?;
        let sealed = cipher.encrypt(&nonce, plain.as_ref()).map_err(|_| anyhow!("encrypting the vault"))?;
        let mut out = Vec::with_capacity(MAGIC.len() + 24 + sealed.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&sealed);
        let tmp = self.file.with_extension("enc.tmp");
        write_private_bytes(&tmp, &out)?;
        std::fs::rename(&tmp, &self.file).context("saving the vault")?;
        Ok(())
    }

    fn guard(&self) -> std::sync::MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Every credential, without secret values.
    pub fn list(&self) -> Result<Vec<CredentialView>> {
        let _g = self.guard();
        let mut out: Vec<_> = self.load()?.credentials.iter().map(Credential::view).collect();
        out.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
        Ok(out)
    }

    pub fn view(&self, id: &str) -> Result<Option<CredentialView>> {
        let _g = self.guard();
        Ok(self.load()?.credentials.iter().find(|c| c.id == id).map(Credential::view))
    }

    /// The whole credential, values included, for the daemon's own use
    /// (matching, the host check). Never serialise it to a response.
    pub fn get(&self, id: &str) -> Result<Option<Credential>> {
        let _g = self.guard();
        Ok(self.load()?.credentials.into_iter().find(|c| c.id == id))
    }

    pub fn all(&self) -> Result<Vec<Credential>> {
        let _g = self.guard();
        Ok(self.load()?.credentials)
    }

    /// One field's value. A runner-only field is refused to anything but the sign-in runner.
    pub fn field(&self, id: &str, name: &str, caller: Caller) -> Result<Option<String>> {
        let Some(c) = self.get(id)? else { bail!("no credential {id}") };
        let Some(f) = c.field(name) else { return Ok(None) };
        if f.runner_only && caller != Caller::Runner {
            bail!("{} of {} is only typed by the sign-in, never handed to a call", f.name, c.label);
        }
        Ok(Some(f.value.clone()).filter(|v| !v.is_empty()))
    }

    /// Add a credential from a draft. Returns it as seen from outside.
    pub fn create(&self, d: Draft) -> Result<CredentialView> {
        let _g = self.guard();
        let mut contents = self.load()?;
        let template_id = d.template.clone().unwrap_or_else(|| "custom".into());
        let t = template(&template_id).ok_or_else(|| anyhow!("unknown kind of credential: {template_id}"))?;
        let now = Utc::now();
        let mut c = Credential {
            id: new_id(),
            label: String::new(),
            template: template_id,
            domains: vec![],
            fields: t.fields,
            provider: None,
            login_url: None,
            notes: String::new(),
            created_at: now,
            updated_at: now,
        };
        apply(&mut c, d)?;
        if c.label.trim().is_empty() {
            c.label = c.identity().or_else(|| c.default_domain().map(str::to_string)).unwrap_or_else(|| t.label.to_string());
        }
        let view = c.view();
        contents.credentials.push(c);
        self.save(&contents)?;
        Ok(view)
    }

    /// Change a credential: label, domains, notes, field values.
    pub fn update(&self, id: &str, d: Draft) -> Result<CredentialView> {
        let _g = self.guard();
        let mut contents = self.load()?;
        let c = contents.credentials.iter_mut().find(|c| c.id == id).ok_or_else(|| anyhow!("no credential {id}"))?;
        if d.template.as_deref().is_some_and(|t| t != c.template) {
            bail!("a credential keeps its kind; add a new one instead");
        }
        apply(c, d)?;
        c.updated_at = Utc::now();
        let view = c.view();
        self.save(&contents)?;
        Ok(view)
    }

    /// Set values without touching anything else (a refreshed token). Unknown names are added.
    pub fn set_values(&self, id: &str, values: &[(&str, &str)]) -> Result<()> {
        self.update(id, Draft { values: values.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), ..Default::default() }).map(|_| ())
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        let _g = self.guard();
        let mut contents = self.load()?;
        let before = contents.credentials.len();
        contents.credentials.retain(|c| c.id != id);
        let gone = contents.credentials.len() != before;
        if gone {
            self.save(&contents)?;
        }
        Ok(gone)
    }
}

fn apply(c: &mut Credential, d: Draft) -> Result<()> {
    if let Some(l) = d.label {
        c.label = l.trim().to_string();
    }
    if let Some(ds) = d.domains {
        let mut out: Vec<String> = vec![];
        for raw in ds {
            let Some(h) = normalise_host(&raw) else { continue };
            if !out.contains(&h) {
                out.push(h);
            }
        }
        c.domains = out;
    }
    for nf in d.fields {
        if !crate::secrets::valid_field(&nf.name) {
            bail!("a field's name is letters, digits and _: {}", nf.name);
        }
        if c.field(&nf.name).is_none() {
            c.fields.push(Field { value: String::new(), ..nf });
        }
    }
    for (name, value) in d.values {
        match c.fields.iter_mut().find(|f| f.name == name) {
            Some(f) => f.value = value.trim().to_string(),
            None if !value.is_empty() => {
                if !crate::secrets::valid_field(&name) {
                    bail!("a field's name is letters, digits and _: {name}");
                }
                c.fields.push(Field { name: name.clone(), label: name.clone(), kind: FieldKind::Secret, value: value.trim().to_string(), runner_only: false, optional: true });
            }
            None => {}
        }
    }
    if let Some(p) = d.provider {
        c.provider = Some(p).filter(|p| !p.is_empty());
    }
    if let Some(u) = d.login_url {
        c.login_url = Some(u.trim().to_string()).filter(|u| !u.is_empty());
    }
    if let Some(n) = d.notes {
        c.notes = n;
    }
    // A login's password and code are typed by the runner only, whatever a draft says.
    if c.template == "login" {
        for f in c.fields.iter_mut().filter(|f| f.name == "password" || f.name == "totp") {
            f.runner_only = true;
        }
    }
    Ok(())
}

/// A host as KIQR compares it: lower case, no scheme, path, port, `www.` or trailing dot.
pub fn normalise_host(raw: &str) -> Option<String> {
    let s = raw.trim().to_lowercase();
    let s = s.split("://").nth(1).unwrap_or(&s);
    let s = s.split(['/', '?', '#']).next().unwrap_or("");
    let s = s.rsplit_once('@').map(|(_, h)| h).unwrap_or(s);
    let s = if s.starts_with('[') { s } else { s.split(':').next().unwrap_or("") };
    let s = s.trim_end_matches('.');
    let s = s.strip_prefix("www.").unwrap_or(s);
    (!s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']' | ':'))).then(|| s.to_string())
}

/// The registrable part of a host (`api.x.com` → `x.com`); a bare public
/// suffix (`com`, `co.uk`) has none. Hosts with no public suffix (localhost,
/// an IP) are their own.
pub fn site_of(host: &str) -> Option<String> {
    let h = normalise_host(host)?;
    if h.parse::<std::net::IpAddr>().is_ok() || h.starts_with('[') || !h.contains('.') {
        return Some(h);
    }
    psl::domain_str(&h).map(str::to_string)
}

/// Whether two hosts belong together, so a credential for `x.com` fits an
/// ability on `api.x.com`: the same registrable domain (`old.reddit.com`,
/// `www.reddit.com`), or one inside the other (`googleapis.com` holds
/// `analyticsdata.googleapis.com`, a suffix the list keeps private). A bare
/// public suffix (`com`, `co.uk`) holds nothing.
pub fn same_site(a: &str, b: &str) -> bool {
    let (Some(a), Some(b)) = (normalise_host(a), normalise_host(b)) else { return false };
    if a == b {
        return true;
    }
    if inside(&a, &b) || inside(&b, &a) {
        return true;
    }
    matches!((site_of(&a), site_of(&b)), (Some(x), Some(y)) if x == y)
}

/// `host` is `domain` or below it, and `domain` is not a public suffix of the ICANN list.
fn inside(host: &str, domain: &str) -> bool {
    if !host.ends_with(&format!(".{domain}")) || !domain.contains('.') {
        return false;
    }
    !matches!(psl::suffix(domain.as_bytes()), Some(s) if s.typ() == Some(psl::Type::Icann) && s.as_bytes() == domain.as_bytes())
}

/// The icons a page declares (`<link rel="icon">`, `apple-touch-icon`), sharpest useful size first.
pub fn icon_links(html: &str, page: &url::Url) -> Vec<url::Url> {
    let doc = scraper::Html::parse_document(html);
    let Ok(sel) = scraper::Selector::parse("link[rel][href]") else { return vec![] };
    let mut found: Vec<(u32, url::Url)> = doc
        .select(&sel)
        .filter(|e| e.value().attr("rel").is_some_and(|r| r.to_lowercase().split_whitespace().any(|w| w == "icon" || w == "apple-touch-icon")))
        .filter_map(|e| {
            let u = page.join(e.value().attr("href")?).ok()?;
            let size = e.value().attr("sizes").and_then(|s| s.split(['x', 'X']).next()?.parse::<u32>().ok()).unwrap_or(16);
            Some((size, u))
        })
        .collect();
    // Big enough to look sharp, small enough to load fast.
    found.sort_by_key(|(s, _)| if *s >= 32 { *s } else { 10_000 - s });
    found.into_iter().map(|(_, u)| u).collect()
}

fn new_id() -> String {
    let r: [u8; 6] = random();
    format!("cred_{}", r.iter().map(|b| format!("{b:02x}")).collect::<String>())
}

fn random<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    getrandom::fill(&mut out).expect("the system's random source");
    out
}

fn decode_key(text: &str) -> Result<[u8; 32]> {
    use base64::Engine;
    let t = text.trim();
    let bytes = if t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()) {
        (0..32).map(|i| u8::from_str_radix(&t[i * 2..i * 2 + 2], 16)).collect::<Result<Vec<u8>, _>>()?
    } else {
        base64::engine::general_purpose::STANDARD.decode(t).context("the vault key is not base64 or hex")?
    };
    bytes.try_into().map_err(|_| anyhow!("the vault key must be 32 bytes"))
}

fn encode_key(k: &[u8; 32]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(k)
}

/// Find the key, in the order the module describes. A vault that exists is
/// never given a new key: without its own it stays locked.
fn find_key(home: &Home, vault_exists: bool) -> Result<([u8; 32], KeySource)> {
    if let Ok(p) = std::env::var("KIQR_VAULT_KEY_FILE") {
        let text = std::fs::read_to_string(&p).with_context(|| format!("reading KIQR_VAULT_KEY_FILE ({p})"))?;
        return Ok((decode_key(&text)?, KeySource::KeyFile));
    }
    if let Ok(dir) = std::env::var("CREDENTIALS_DIRECTORY") {
        let p = Path::new(&dir).join("kiqr-vault");
        if let Ok(text) = std::fs::read_to_string(&p) {
            return Ok((decode_key(&text)?, KeySource::SystemdCredential));
        }
    }
    if let Ok(v) = std::env::var("KIQR_VAULT_KEY") {
        // Read once, then gone from this process, so no program it starts inherits it.
        std::env::remove_var("KIQR_VAULT_KEY");
        return Ok((decode_key(&v)?, KeySource::Env));
    }
    let local = home.vault_key_file();
    if let Ok(text) = std::fs::read_to_string(&local) {
        return Ok((decode_key(&text)?, KeySource::LocalFile));
    }
    if keyring_reachable() {
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
            match entry.get_password() {
                Ok(text) => return Ok((decode_key(&text)?, KeySource::Keyring)),
                Err(_) if !vault_exists => {
                    let k: [u8; 32] = random();
                    if entry.set_password(&encode_key(&k)).is_ok() && entry.get_password().ok().as_deref() == Some(encode_key(&k).as_str()) {
                        return Ok((k, KeySource::Keyring));
                    }
                }
                Err(_) => {}
            }
        }
    }
    if vault_exists {
        bail!("the vault's key was not found (no key file, KIQR_VAULT_KEY or keyring entry)");
    }
    let k: [u8; 32] = random();
    write_private_bytes(&local, encode_key(&k).as_bytes())?;
    Ok((k, KeySource::LocalFile))
}

/// The keyring is a desktop service: on a server with no session bus, asking
/// it only waits and fails.
fn keyring_reachable() -> bool {
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        return true;
    }
    std::env::var("DBUS_SESSION_BUS_ADDRESS").is_ok_and(|v| !v.is_empty())
}

fn write_private_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    #[cfg(not(unix))]
    std::fs::write(path, bytes)?;
    Ok(())
}

/// A folder only this user may enter (0700): the vault, a login's browser profile.
pub fn private_dir(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    let _ = dir;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kiqr-vault-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn round_trip_without_leaking_values() {
        let d = tmp("rt");
        let v = Vault::with_key(d.join("vault.enc"), [7u8; 32]);
        let mut values = std::collections::BTreeMap::new();
        values.insert("username".to_string(), "kiqr_dev".to_string());
        values.insert("password".to_string(), "hunter2-secret".to_string());
        let c = v.create(Draft { template: Some("login".into()), domains: Some(vec!["https://www.X.com/".into(), "twitter.com".into()]), values, ..Default::default() }).unwrap();
        assert_eq!(c.domains, vec!["x.com", "twitter.com"]);
        assert_eq!(c.identity.as_deref(), Some("kiqr_dev"));
        assert_eq!(c.label, "kiqr_dev");
        let listed = serde_json::to_string(&v.list().unwrap()).unwrap();
        assert!(!listed.contains("hunter2"), "a value leaked: {listed}");
        let raw = std::fs::read(d.join("vault.enc")).unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("hunter2"));
        // A password is typed by the runner only.
        assert!(v.field(&c.id, "password", Caller::Call).is_err());
        assert_eq!(v.field(&c.id, "password", Caller::Runner).unwrap().as_deref(), Some("hunter2-secret"));
        assert_eq!(v.field(&c.id, "username", Caller::Call).unwrap().as_deref(), Some("kiqr_dev"));
        // Another key opens nothing.
        let other = Vault::with_key(d.join("vault.enc"), [8u8; 32]);
        assert!(other.list().is_err());
        assert!(v.delete(&c.id).unwrap());
        assert!(v.list().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_draft_cannot_free_a_login_password() {
        let d = tmp("ro");
        let v = Vault::with_key(d.join("vault.enc"), [1u8; 32]);
        let c = v.create(Draft { template: Some("login".into()), values: [("username".to_string(), "a".to_string()), ("password".to_string(), "p".to_string())].into(), ..Default::default() }).unwrap();
        v.update(&c.id, Draft { fields: vec![Field { name: "password".into(), runner_only: false, ..Default::default() }], ..Default::default() }).unwrap();
        assert!(v.field(&c.id, "password", Caller::Call).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sites_match_on_the_registrable_domain() {
        assert!(same_site("x.com", "api.x.com"));
        assert!(same_site("https://www.reddit.com/r/x", "old.reddit.com"));
        assert!(same_site("google.com", "accounts.google.com"));
        assert!(!same_site("google.com", "analyticsdata.googleapis.com"));
        assert!(same_site("googleapis.com", "analyticsdata.googleapis.com"));
        assert!(!same_site("com", "x.com"));
        assert!(!same_site("example.co.uk", "other.co.uk"));
        assert!(same_site("localhost", "localhost:8080"));
        assert!(!same_site("127.0.0.1", "127.0.0.2"));
        assert_eq!(site_of("co.uk"), None);
    }
}
