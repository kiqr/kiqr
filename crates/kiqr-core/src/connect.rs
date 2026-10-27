//! Signing in from a manifest. A connection may say how its keys are got
//! with one button instead of typed in: its `connect` block names a generic
//! flow and everything specific to the service (URLs, parameters, which
//! value lands in which variable). KIQR knows the flows, never the services.
//!
//! - `redirect`: open a page; the service sends the browser back to KIQR's
//!   callback with the values in the query (an application password, a key).
//! - `oauth2`: authorization code with PKCE; the code is exchanged at the
//!   token URL, and the access token is refreshed before it runs out.
//! - `service_account`: no browser. A key file on this machine signs a JWT
//!   that the token URL exchanges for an access token, minted at call time.
//! - `cloud`: oauth2 for a provider whose app needs a client secret (Google,
//!   LinkedIn). kiqr.cloud holds the secret: it builds the consent URL, the
//!   provider comes back through its relay, and it exchanges and refreshes
//!   for this KIQR without keeping a token. The manifest names the provider
//!   and the scopes; URLs and client ids live on kiqr.cloud.
//!
//! A manifest may offer several (`connect:` as a list): the first with a
//! button is the button, the others are alternatives (a key file instead of
//! signing in). An account is ready with any one of them set up.
//!
//! The values land in the same `.env` variables the manifest's `auth` names,
//! so readiness, `${…}` and hosts work as for typed keys. This module is
//! pure: building URLs, reading callbacks and token responses. The daemon
//! keeps the pending flows, signs, and makes the requests.

use crate::extensions::{fill, host_allowed, pointer, valid_env_name, Manifest};
use anyhow::{bail, Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Flow {
    #[default]
    Redirect,
    Oauth2,
    ServiceAccount,
    Cloud,
}

/// What the service accepts as a callback. `https`: only an https one (a
/// local daemon on plain http then opens the page without it, and the user
/// pastes what the page shows).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Callback {
    #[default]
    Any,
    Https,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Denied {
    /// The callback parameter that says the user said no.
    pub param: String,
    /// Its value then; empty: the parameter being there at all.
    pub equals: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Connect {
    /// The button: "Connect with WordPress".
    pub label: String,
    pub flow: Flow,
    /// Variables asked for before the flow starts (the site, for a page on it).
    pub needs: Vec<String>,
    /// redirect: the page to open. May hold `${VAR}` and `{{app_id}}`.
    pub url: String,
    /// redirect: the query parameters that carry KIQR's callback URL.
    pub callback_params: Vec<String>,
    pub callback: Callback,
    /// oauth2: where the user approves.
    pub authorize_url: String,
    /// oauth2 and service_account: where a code, a refresh token or a signed JWT becomes a token.
    pub token_url: String,
    /// oauth2: the client id (usually `${VAR}`), and its secret when the provider wants one.
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
    /// Extra parameters on the authorize URL (`access_type: offline`).
    pub params: BTreeMap<String, String>,
    /// redirect: variable → callback parameter. oauth2: variable → pointer
    /// into the token response, beside the token itself.
    pub returns: BTreeMap<String, String>,
    pub denied: Option<Denied>,
    /// oauth2: the variable that holds the access token (stored); service_account:
    /// the variable a request reads it from (minted at call time, never stored).
    pub token_env: String,
    /// oauth2: where the refresh token is kept.
    pub refresh_env: String,
    /// service_account: the variable that holds the path of the key file.
    pub key_file: String,
    /// cloud: the provider kiqr.cloud signs in to (`google`).
    pub provider: String,
}

impl Connect {
    /// Where the access token's expiry (unix seconds) is kept, beside it.
    pub fn expires_env(&self) -> String {
        format!("{}_EXPIRES_AT", self.token_env)
    }

    /// Every variable this flow writes into `.env`.
    pub fn writes(&self) -> Vec<String> {
        let mut v: Vec<String> = self.returns.keys().cloned().collect();
        if self.stores_token() {
            v.push(self.token_env.clone());
            if !self.refresh_env.is_empty() {
                v.push(self.refresh_env.clone());
            }
            v.push(self.expires_env());
        }
        v.sort();
        v.dedup();
        v
    }

    /// The flow keeps a token (and its refresh token) on the account.
    pub fn stores_token(&self) -> bool {
        matches!(self.flow, Flow::Oauth2 | Flow::Cloud)
    }

    /// Opens a page in the browser (as opposed to a key file on this machine).
    pub fn has_button(&self) -> bool {
        self.flow != Flow::ServiceAccount
    }

    /// Is this way of signing in set up on an account, given what it holds?
    pub fn satisfied(&self, has: &impl Fn(&str) -> bool) -> bool {
        match self.flow {
            Flow::Redirect => self.returns.keys().all(|k| has(k)),
            Flow::Oauth2 | Flow::Cloud => has(&self.token_env) || (!self.refresh_env.is_empty() && has(&self.refresh_env)),
            Flow::ServiceAccount => has(&self.key_file),
        }
    }

    /// The variable to name when it is not: what the user is missing.
    pub fn proof(&self) -> &str {
        match self.flow {
            Flow::ServiceAccount => &self.key_file,
            Flow::Redirect => self.returns.keys().next().map(String::as_str).unwrap_or(""),
            _ => &self.token_env,
        }
    }
}

/// The way a call signs in, of those a manifest offers: the first set up on
/// the account, else the first (whose error then says what is missing).
pub fn pick<'a>(all: &'a [Connect], has: &impl Fn(&str) -> bool) -> Option<&'a Connect> {
    all.iter().find(|c| c.satisfied(has)).or_else(|| all.first())
}

/// One `connect` block or a list of them.
pub fn one_or_many<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<Connect>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(Box<Connect>),
        Many(Vec<Connect>),
    }
    Ok(match Option::<OneOrMany>::deserialize(d)? {
        None => vec![],
        Some(OneOrMany::One(c)) => vec![*c],
        Some(OneOrMany::Many(v)) => v,
    })
}

/// What the UI needs to offer the button: the label, the flow, what to ask
/// first, what the user pastes when the service shows it instead of sending
/// it back, and whether it goes through kiqr.cloud.
pub fn view(all: &[Connect]) -> Value {
    match all.iter().find(|c| c.has_button()) {
        Some(c) => json!({
            "label": if c.label.is_empty() { "Connect".to_string() } else { c.label.clone() },
            "flow": c.flow,
            "needs": c.needs,
            "returns": c.returns.keys().collect::<Vec<_>>(),
            "https_only": c.callback == Callback::Https,
            "via_cloud": c.flow == Flow::Cloud,
            "provider": (!c.provider.is_empty()).then(|| c.provider.clone()),
        }),
        None => Value::Null,
    }
}

/// The keys a sign-in writes and a person never types (a token, its refresh
/// token, its expiry): left out of the fields the UI shows.
pub fn written_tokens(all: &[Connect]) -> Vec<String> {
    let mut v: Vec<String> = all.iter().filter(|c| c.stores_token()).flat_map(|c| [c.token_env.clone(), c.refresh_env.clone(), c.expires_env()]).filter(|s| !s.is_empty()).collect();
    v.sort();
    v.dedup();
    v
}

/// Checked with the rest of the manifest.
pub fn validate(m: &Manifest, c: &Connect) -> Result<()> {
    let declared = |n: &str| m.auth.iter().any(|a| a.env == n);
    let per_project = m.hosts.iter().any(|h| h.starts_with("${"));
    let check_host = |what: &str, u: &str| -> Result<()> {
        let url = url::Url::parse(&fill(u, &json!({ "app_id": "x" }), |_| Some("https://placeholder.invalid".into()))).with_context(|| format!("connect: the {what} does not parse"))?;
        if !per_project && !host_allowed(&url, &m.hosts) {
            bail!("connect: the {what} is on {}, which is not in hosts", url.host_str().unwrap_or(""));
        }
        Ok(())
    };
    for n in &c.needs {
        if !declared(n) {
            bail!("connect needs {n}, which auth does not declare");
        }
    }
    for n in c.returns.keys() {
        if !declared(n) {
            bail!("connect returns {n}, which auth does not declare");
        }
    }
    if let Some(d) = &c.denied {
        if d.param.trim().is_empty() {
            bail!("connect: denied needs a param");
        }
    }
    match c.flow {
        Flow::Redirect => {
            if c.url.trim().is_empty() {
                bail!("a redirect connect needs a url");
            }
            if c.returns.is_empty() {
                bail!("a redirect connect must say what it returns");
            }
            check_host("url", &c.url)?;
        }
        Flow::Oauth2 => {
            if c.authorize_url.trim().is_empty() || c.token_url.trim().is_empty() || c.client_id.trim().is_empty() {
                bail!("an oauth2 connect needs authorize_url, token_url and client_id");
            }
            if !declared(&c.token_env) {
                bail!("an oauth2 connect keeps its token in token_env, which auth must declare");
            }
            if !c.refresh_env.is_empty() && !valid_env_name(&c.refresh_env) {
                bail!("refresh_env '{}' is not a variable name", c.refresh_env);
            }
            check_host("authorize_url", &c.authorize_url)?;
            check_host("token_url", &c.token_url)?;
        }
        Flow::ServiceAccount => {
            if c.token_url.trim().is_empty() || c.scopes.is_empty() {
                bail!("a service_account connect needs token_url and scopes");
            }
            if !declared(&c.key_file) {
                bail!("a service_account connect reads its key file from key_file, which auth must declare");
            }
            // Minted, never stored: auth may name it only as optional (another way of signing in keeps it).
            if !valid_env_name(&c.token_env) || m.auth.iter().any(|a| a.env == c.token_env && !a.optional) {
                bail!("a service_account connect needs a token_env that auth does not require (the token is minted, not stored)");
            }
            check_host("token_url", &c.token_url)?;
        }
        Flow::Cloud => {
            if !crate::tools::valid_ext_id(&c.provider) {
                bail!("a cloud connect names its provider (`google`): lowercase letters, digits and dashes");
            }
            if c.scopes.is_empty() {
                bail!("a cloud connect needs the scopes it asks for");
            }
            if !declared(&c.token_env) {
                bail!("a cloud connect keeps its token in token_env, which auth must declare");
            }
            if !c.refresh_env.is_empty() && !valid_env_name(&c.refresh_env) {
                bail!("refresh_env '{}' is not a variable name", c.refresh_env);
            }
            if !c.authorize_url.is_empty() || !c.token_url.is_empty() || !c.client_id.is_empty() || !c.client_secret.is_empty() {
                bail!("a cloud connect leaves the URLs and the client to kiqr.cloud");
            }
        }
    }
    Ok(())
}

/// A random string for `state` and the PKCE verifier (64 characters, unreserved).
pub fn random_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

/// The PKCE S256 challenge for a verifier.
pub fn pkce_challenge(verifier: &str) -> String {
    use sha2::Digest;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()))
}

/// What the browser opens. `callback` is KIQR's callback URL, or None when
/// the service would refuse it (a redirect that wants https, on plain http).
pub fn start_url(c: &Connect, env: &impl Fn(&str) -> Option<String>, app_id: &str, state: &str, callback: Option<&str>, verifier: &str) -> Result<String> {
    let args = json!({ "app_id": app_id });
    let filled = |s: &str| fill(s, &args, env);
    let leftover = |s: &str| -> Result<()> {
        if let Some(i) = s.find("${") {
            let end = s[i..].find('}').map(|e| i + e + 1).unwrap_or(s.len());
            bail!("{} is not set yet", &s[i..end]);
        }
        Ok(())
    };
    match c.flow {
        Flow::Redirect => {
            let s = filled(&c.url);
            leftover(&s)?;
            let mut u = url::Url::parse(&s).with_context(|| format!("{s} is not a URL"))?;
            if let Some(cb) = callback {
                let cb = with_state(cb, state)?;
                for p in &c.callback_params {
                    u.query_pairs_mut().append_pair(p, &cb);
                }
            }
            Ok(u.to_string())
        }
        Flow::Oauth2 => {
            let s = filled(&c.authorize_url);
            leftover(&s)?;
            let client_id = filled(&c.client_id);
            leftover(&client_id)?;
            let cb = callback.context("this service needs a callback KIQR can offer")?;
            let mut u = url::Url::parse(&s).with_context(|| format!("{s} is not a URL"))?;
            {
                let mut q = u.query_pairs_mut();
                q.append_pair("response_type", "code").append_pair("client_id", &client_id).append_pair("redirect_uri", cb).append_pair("state", state);
                if !c.scopes.is_empty() {
                    q.append_pair("scope", &c.scopes.join(" "));
                }
                q.append_pair("code_challenge", &pkce_challenge(verifier)).append_pair("code_challenge_method", "S256");
                for (k, v) in &c.params {
                    q.append_pair(k, &filled(v));
                }
            }
            Ok(u.to_string())
        }
        Flow::ServiceAccount => bail!("a service account signs in without a browser"),
        Flow::Cloud => bail!("kiqr.cloud builds this sign-in's page"),
    }
}

/// A redirect flow carries `state` in the callback URL itself (the service
/// returns to it as given); oauth2 sends it as a parameter of its own.
fn with_state(cb: &str, state: &str) -> Result<String> {
    let mut u = url::Url::parse(cb).with_context(|| format!("{cb} is not a URL"))?;
    u.query_pairs_mut().append_pair("state", state);
    Ok(u.to_string())
}

/// The values a redirect callback carries, as (variable, value).
pub fn read_redirect(c: &Connect, query: &BTreeMap<String, String>) -> Result<Vec<(String, String)>> {
    denied(c, query)?;
    let mut out = vec![];
    for (env, param) in &c.returns {
        match query.get(param).map(|v| v.trim()).filter(|v| !v.is_empty()) {
            Some(v) => out.push((env.clone(), v.to_string())),
            None => bail!("the service did not send {param}"),
        }
    }
    Ok(out)
}

fn denied(c: &Connect, query: &BTreeMap<String, String>) -> Result<()> {
    if query.contains_key("error") {
        bail!("the service said no: {}", query.get("error_description").or(query.get("error")).cloned().unwrap_or_default());
    }
    if let Some(d) = &c.denied {
        if let Some(v) = query.get(&d.param) {
            if d.equals.is_empty() || v == &d.equals {
                bail!("you declined on the service's page");
            }
        }
    }
    Ok(())
}

/// The code an oauth2 callback carries.
pub fn read_code(c: &Connect, query: &BTreeMap<String, String>) -> Result<String> {
    denied(c, query)?;
    query.get("code").filter(|v| !v.is_empty()).cloned().context("the service sent no code")
}

/// The form that exchanges a code for a token.
pub fn code_form(c: &Connect, env: &impl Fn(&str) -> Option<String>, code: &str, callback: &str, verifier: &str) -> Vec<(String, String)> {
    let mut f = vec![
        ("grant_type".into(), "authorization_code".into()),
        ("code".into(), code.into()),
        ("redirect_uri".into(), callback.into()),
        ("client_id".into(), fill(&c.client_id, &Value::Null, env)),
        ("code_verifier".into(), verifier.into()),
    ];
    if !c.client_secret.is_empty() {
        f.push(("client_secret".into(), fill(&c.client_secret, &Value::Null, env)));
    }
    f
}

/// The form that refreshes a token.
pub fn refresh_form(c: &Connect, env: &impl Fn(&str) -> Option<String>, refresh: &str) -> Vec<(String, String)> {
    let mut f = vec![("grant_type".into(), "refresh_token".into()), ("refresh_token".into(), refresh.into()), ("client_id".into(), fill(&c.client_id, &Value::Null, env))];
    if !c.client_secret.is_empty() {
        f.push(("client_secret".into(), fill(&c.client_secret, &Value::Null, env)));
    }
    f
}

/// What a token response means for `.env`: the token, the refresh token
/// when one came, its expiry, and whatever `returns` points at.
pub fn read_token(c: &Connect, body: &Value, now: i64) -> Result<Vec<(String, String)>> {
    let token = body["access_token"].as_str().filter(|s| !s.is_empty()).with_context(|| format!("the token response has no access_token: {}", error_text(body)))?;
    let mut out = vec![(c.token_env.clone(), token.to_string())];
    if !c.refresh_env.is_empty() {
        if let Some(r) = body["refresh_token"].as_str().filter(|s| !s.is_empty()) {
            out.push((c.refresh_env.clone(), r.to_string()));
        }
    }
    let expires_in = body["expires_in"].as_i64().or_else(|| body["expires_in"].as_str().and_then(|s| s.parse().ok())).unwrap_or(3600);
    out.push((c.expires_env(), (now + expires_in).to_string()));
    for (env, p) in &c.returns {
        if let Some(v) = pointer(body, p) {
            let v = v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string());
            out.push((env.clone(), v));
        }
    }
    Ok(out)
}

/// A provider's error, readable.
pub fn error_text(body: &Value) -> String {
    body["error_description"].as_str().or(body["error"]["message"].as_str()).or(body["error"].as_str()).map(str::to_string).unwrap_or_else(|| crate::profile::truncate_chars(&body.to_string(), 200))
}

/// Does a stored oauth2 token need refreshing (gone, or within a minute of expiring)?
pub fn needs_refresh(c: &Connect, env: &impl Fn(&str) -> Option<String>, now: i64) -> bool {
    if env(&c.token_env).is_none() {
        return true;
    }
    env(&c.expires_env()).and_then(|e| e.parse::<i64>().ok()).is_some_and(|e| e - 60 <= now)
}

/// A service-account key file: who signs, with what.
pub struct ServiceKey {
    pub client_email: String,
    pub private_key_pem: String,
}

pub fn read_service_key(text: &str) -> Result<ServiceKey> {
    let k: Value = serde_json::from_str(text).context("the key file is not JSON")?;
    let email = k["client_email"].as_str().context("the key file has no client_email")?;
    let pem = k["private_key"].as_str().context("the key file has no private_key")?;
    Ok(ServiceKey { client_email: email.to_string(), private_key_pem: pem.to_string() })
}

/// The unsigned JWT (`header.claims`, base64url) a service account signs with RS256.
pub fn jwt_unsigned(c: &Connect, key: &ServiceKey, now: i64) -> String {
    let b64 = |v: &Value| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v.to_string().as_bytes());
    let header = json!({ "alg": "RS256", "typ": "JWT" });
    let claims = json!({ "iss": key.client_email, "scope": c.scopes.join(" "), "aud": c.token_url, "iat": now, "exp": now + 3600 });
    format!("{}.{}", b64(&header), b64(&claims))
}

/// The DER bytes inside a PEM private key.
pub fn pem_to_der(pem: &str) -> Result<Vec<u8>> {
    let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    base64::engine::general_purpose::STANDARD.decode(body.trim()).context("the private key is not base64 PEM")
}

pub fn jwt_form(signed: &str) -> Vec<(String, String)> {
    vec![("grant_type".into(), "urn:ietf:params:oauth:grant-type:jwt-bearer".into()), ("assertion".into(), signed.into())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::parse_manifest;

    const WP: &str = r#"
id: blog
name: Publish to a blog
transport: http
auth:
  - { env: SITE, label: Site }
  - { env: USER, label: User }
  - { env: PASS, label: Password }
hosts: ["${SITE}"]
connect:
  label: Connect with the blog
  flow: redirect
  needs: [SITE]
  url: "${SITE}/authorize?app_name=KIQR&app_id={{app_id}}"
  callback_params: [success_url, reject_url]
  callback: https
  returns: { SITE: site_url, USER: user_login, PASS: password }
  denied: { param: success, equals: "false" }
"#;

    fn q(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn redirect_round_trip() {
        let m = parse_manifest(WP).unwrap();
        let c = m.connect[0].clone();
        let env = |k: &str| (k == "SITE").then(|| "https://blog.test".to_string());
        let u = start_url(&c, &env, "abc", "st", Some("https://kiqr.local/api/connect/callback"), "v").unwrap();
        assert!(u.starts_with("https://blog.test/authorize?app_name=KIQR&app_id=abc&success_url="));
        assert!(u.contains("reject_url=") && u.contains("state%3Dst"));
        // On plain http the page opens without a callback.
        let bare = start_url(&c, &env, "abc", "st", None, "v").unwrap();
        assert!(!bare.contains("success_url"));
        let got = read_redirect(&c, &q(&[("site_url", "https://blog.test"), ("user_login", "jane"), ("password", "abcd efgh")])).unwrap();
        assert_eq!(got.len(), 3);
        assert!(got.contains(&("PASS".into(), "abcd efgh".into())));
        assert!(read_redirect(&c, &q(&[("success", "false")])).unwrap_err().to_string().contains("declined"));
        assert!(read_redirect(&c, &q(&[("site_url", "x")])).unwrap_err().to_string().contains("did not send"));
    }

    #[test]
    fn site_must_be_set_first() {
        let c = parse_manifest(WP).unwrap().connect[0].clone();
        assert!(start_url(&c, &|_| None, "a", "s", None, "v").unwrap_err().to_string().contains("${SITE}"));
    }

    #[test]
    fn validation() {
        assert!(parse_manifest(&WP.replace("returns: { SITE: site_url, USER: user_login, PASS: password }", "returns: { OTHER: x }")).unwrap_err().to_string().contains("OTHER"));
        assert!(parse_manifest(&WP.replace("needs: [SITE]", "needs: [NOPE]")).unwrap_err().to_string().contains("NOPE"));
        let fixed = WP.replace("hosts: [\"${SITE}\"]", "hosts: [blog.test]").replace("${SITE}/authorize", "https://elsewhere.test/authorize");
        assert!(parse_manifest(&fixed).unwrap_err().to_string().contains("elsewhere.test"));
    }

    const OAUTH: &str = r#"
id: svc
name: A service
transport: http
auth:
  - { env: SVC_CLIENT_ID, label: Client id }
  - { env: SVC_TOKEN, label: Token }
hosts: [svc.test]
connect:
  label: Connect
  flow: oauth2
  authorize_url: https://svc.test/oauth/authorize
  token_url: https://svc.test/oauth/token
  client_id: "${SVC_CLIENT_ID}"
  scopes: [read, write]
  params: { access_type: offline }
  token_env: SVC_TOKEN
  refresh_env: SVC_REFRESH
"#;

    #[test]
    fn oauth2() {
        let c = parse_manifest(OAUTH).unwrap().connect[0].clone();
        let env = |k: &str| (k == "SVC_CLIENT_ID").then(|| "cid".to_string());
        let u = start_url(&c, &env, "", "st", Some("http://127.0.0.1:7878/api/connect/callback"), "verifier").unwrap();
        assert!(u.contains("client_id=cid") && u.contains("state=st") && u.contains("scope=read+write") && u.contains("code_challenge_method=S256") && u.contains("access_type=offline"));
        assert!(u.contains(&pkce_challenge("verifier")));
        assert_eq!(read_code(&c, &q(&[("code", "C")])).unwrap(), "C");
        assert!(read_code(&c, &q(&[("error", "access_denied")])).is_err());
        let t = read_token(&c, &json!({ "access_token": "AT", "refresh_token": "RT", "expires_in": 100 }), 1000).unwrap();
        assert!(t.contains(&("SVC_TOKEN".into(), "AT".into())) && t.contains(&("SVC_REFRESH".into(), "RT".into())) && t.contains(&("SVC_TOKEN_EXPIRES_AT".into(), "1100".into())));
        assert_eq!(c.writes(), vec!["SVC_REFRESH", "SVC_TOKEN", "SVC_TOKEN_EXPIRES_AT"]);
        let stored = |k: &str| match k {
            "SVC_TOKEN" => Some("AT".to_string()),
            "SVC_TOKEN_EXPIRES_AT" => Some("1100".to_string()),
            _ => None,
        };
        assert!(!needs_refresh(&c, &stored, 1000));
        assert!(needs_refresh(&c, &stored, 1050));
        // RFC 7636 appendix B.
        assert_eq!(pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn service_account() {
        let sa = r#"
id: reader
name: Read numbers
transport: http
auth:
  - { env: KEY_FILE, label: Key file }
hosts: [api.test, auth.test]
connect:
  flow: service_account
  token_url: https://auth.test/token
  scopes: [https://api.test/read]
  key_file: KEY_FILE
  token_env: READER_TOKEN
"#;
        let c = parse_manifest(sa).unwrap().connect[0].clone();
        let key = ServiceKey { client_email: "bot@x.test".into(), private_key_pem: String::new() };
        let unsigned = jwt_unsigned(&c, &key, 10);
        let claims: Value = serde_json::from_slice(&base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(unsigned.split('.').nth(1).unwrap()).unwrap()).unwrap();
        assert_eq!(claims["aud"], "https://auth.test/token");
        assert_eq!(claims["exp"], 3610);
        assert!(parse_manifest(&sa.replace("token_env: READER_TOKEN", "token_env: KEY_FILE")).is_err());
    }

    /// Sign in through kiqr.cloud, or use a key file: two ways, one account.
    const BOTH: &str = r#"
id: numbers
name: Read numbers
transport: http
auth:
  - { env: PROP, label: Property }
  - { env: NUM_TOKEN, label: Token, optional: true }
  - { env: KEY_FILE, label: Key file, optional: true }
hosts: [api.test, auth.test]
connect:
  - { label: Connect with the service, flow: cloud, provider: svc, scopes: [read], token_env: NUM_TOKEN, refresh_env: NUM_REFRESH }
  - { flow: service_account, token_url: https://auth.test/token, scopes: [read], key_file: KEY_FILE, token_env: NUM_TOKEN }
"#;

    #[test]
    fn cloud_or_a_key_file() {
        let m = parse_manifest(BOTH).unwrap();
        assert_eq!(m.connect.len(), 2);
        let v = view(&m.connect);
        assert_eq!((v["label"].as_str(), v["via_cloud"].as_bool(), v["provider"].as_str()), (Some("Connect with the service"), Some(true), Some("svc")));
        assert_eq!(written_tokens(&m.connect), vec!["NUM_REFRESH", "NUM_TOKEN", "NUM_TOKEN_EXPIRES_AT"]);
        let has = |set: &'static [&'static str]| move |k: &str| set.contains(&k);
        // Nothing yet: the property and the first way's token are missing.
        assert_eq!(crate::extensions::missing_keys(&m, has(&[])), vec!["PROP", "NUM_TOKEN"]);
        // Either way is enough.
        assert!(crate::extensions::missing_keys(&m, has(&["PROP", "NUM_REFRESH"])).is_empty());
        assert!(crate::extensions::missing_keys(&m, has(&["PROP", "KEY_FILE"])).is_empty());
        assert_eq!(pick(&m.connect, &has(&["KEY_FILE"])).unwrap().flow, Flow::ServiceAccount);
        assert_eq!(pick(&m.connect, &has(&["NUM_TOKEN", "KEY_FILE"])).unwrap().flow, Flow::Cloud);
        assert_eq!(pick(&m.connect, &has(&[])).unwrap().flow, Flow::Cloud);
        // The cloud keeps the URLs and the client.
        assert!(parse_manifest(&BOTH.replace("provider: svc,", "provider: svc, token_url: https://auth.test/token,")).is_err());
        assert!(parse_manifest(&BOTH.replace("provider: svc, scopes: [read],", "provider: svc, scopes: [],")).is_err());
        // A key file's minted token may not be a required key.
        assert!(parse_manifest(&BOTH.replace("{ env: NUM_TOKEN, label: Token, optional: true }", "{ env: NUM_TOKEN, label: Token }")).is_err());
        // One block still reads as one way.
        assert_eq!(parse_manifest(WP).unwrap().connect.len(), 1);
    }

    #[test]
    fn choices_from_an_answer() {
        let answer = json!({ "accountSummaries": [
            { "displayName": "Acme", "propertySummaries": [{ "property": "properties/1", "displayName": "acme.com" }, { "property": "properties/2", "displayName": "shop" }] },
            { "displayName": "Other", "propertySummaries": [{ "property": "properties/9", "displayName": "other.org" }] },
            { "displayName": "Empty" }
        ] });
        let items = crate::extensions::pointer_all(&answer, "/accountSummaries/*/propertySummaries/*");
        assert_eq!(items.iter().filter_map(|i| i["property"].as_str()).collect::<Vec<_>>(), vec!["properties/1", "properties/2", "properties/9"]);
        assert_eq!(crate::extensions::pointer_all(&answer, "/accountSummaries/0/displayName")[0], "Acme");
        assert!(crate::extensions::pointer_all(&answer, "/nothing/*").is_empty());
    }
}
