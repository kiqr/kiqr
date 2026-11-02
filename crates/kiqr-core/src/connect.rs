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

use crate::extensions::{fill, pointer};
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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Denied {
    /// The callback parameter that says the user said no.
    pub param: String,
    /// Its value then; empty: the parameter being there at all.
    pub equals: String,
}

/// `grant: { label, url, help }`: where access to more of the service is granted, in the person's own browser.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Grant {
    /// The link: "Install KIQR on your repositories".
    pub label: String,
    /// An https page at the service.
    pub url: String,
    /// One line under it: what happens there.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub help: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    /// A page at the service where the person grants access to more of it after signing in
    /// (a GitHub App installed on a repository, a workspace added): shown beside a key picked
    /// from the account's list when what they look for is not in it.
    pub grant: Option<Grant>,
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
            "grant": c.grant.as_ref().filter(|g| g.url.starts_with("https://") && !g.label.is_empty()),
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
