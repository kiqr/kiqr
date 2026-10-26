//! OAuth as a client, for KIQR's own sign-ins (kiqr.cloud, and UnblockingAPI
//! through its MCP server): discovery (RFC 8414, RFC 9728), registering this
//! KIQR as a public client (RFC 7591), the authorization code flow with PKCE,
//! the device flow (RFC 8628), refresh and revocation. Abilities sign in
//! through their manifest's `connect` block instead (`connect.rs`); this is
//! for the services core may name.

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

/// An authorization server's endpoints.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Server {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: String,
    pub revocation_endpoint: String,
    pub device_authorization_endpoint: String,
}

impl Server {
    pub fn has_device_flow(&self) -> bool {
        !self.device_authorization_endpoint.is_empty()
    }
}

/// Tokens as a server returned them; `expires_at` in unix seconds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tokens {
    pub access: String,
    pub refresh: Option<String>,
    pub expires_at: Option<i64>,
}

/// The device flow's first answer: what to show the user, and what to poll with.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    pub interval: Option<u64>,
}

/// What one poll of the device flow says.
#[derive(Debug, PartialEq)]
pub enum Poll {
    Done(Tokens),
    Pending,
    SlowDown,
    /// Denied or expired: stop.
    Failed(String),
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(crate::fetch::USER_AGENT)
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}

/// `{issuer}/.well-known/oauth-authorization-server`.
pub async fn discover(issuer: &str) -> Result<Server> {
    let url = format!("{}/.well-known/oauth-authorization-server", issuer.trim_end_matches('/'));
    let resp = client()?.get(&url).send().await.with_context(|| format!("reaching {url}"))?;
    if !resp.status().is_success() {
        bail!("{url} answered {}", resp.status());
    }
    let s: Server = resp.json().await.with_context(|| format!("reading {url}"))?;
    if s.authorization_endpoint.is_empty() || s.token_endpoint.is_empty() {
        bail!("{url} names no authorization or token endpoint");
    }
    Ok(s)
}

/// A protected resource's authorization server (RFC 9728), e.g. an MCP server's.
pub async fn resource_issuer(metadata_url: &str) -> Result<String> {
    let resp = client()?.get(metadata_url).send().await.with_context(|| format!("reaching {metadata_url}"))?;
    let v: Value = resp.json().await.with_context(|| format!("reading {metadata_url}"))?;
    v["authorization_servers"][0].as_str().map(str::to_string).ok_or_else(|| anyhow!("{metadata_url} names no authorization server"))
}

/// Register this KIQR as a public client; returns its client id.
pub async fn register(s: &Server, client_name: &str, redirect_uris: &[String], grant_types: &[&str]) -> Result<String> {
    if s.registration_endpoint.is_empty() {
        bail!("{} does not let apps register themselves", s.issuer);
    }
    let body = serde_json::json!({
        "client_name": client_name,
        "redirect_uris": redirect_uris,
        "grant_types": grant_types,
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
    });
    let resp = client()?.post(&s.registration_endpoint).json(&body).send().await.context("registering with the sign-in server")?;
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("registering answered {status}: {}", error_text(&v));
    }
    v["client_id"].as_str().map(str::to_string).ok_or_else(|| anyhow!("the registration answer has no client_id"))
}

/// The page to send the browser to.
pub fn authorize_url(s: &Server, client_id: &str, redirect_uri: &str, scope: &str, state: &str, verifier: &str) -> Result<String> {
    let mut u = url::Url::parse(&s.authorization_endpoint)?;
    u.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", scope)
        .append_pair("state", state)
        .append_pair("code_challenge", &crate::connect::pkce_challenge(verifier))
        .append_pair("code_challenge_method", "S256");
    Ok(u.into())
}

async fn token_call(s: &Server, form: &[(&str, &str)], now: i64) -> Result<std::result::Result<Tokens, Value>> {
    let resp = client()?.post(&s.token_endpoint).header("Accept", "application/json").form(form).send().await.context("reaching the token endpoint")?;
    let ok = resp.status().is_success();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    if !ok {
        return Ok(Err(v));
    }
    Ok(Ok(read_tokens(&v, now)?))
}

/// The answer of a token endpoint as `Tokens`.
pub fn read_tokens(v: &Value, now: i64) -> Result<Tokens> {
    let access = v["access_token"].as_str().filter(|t| !t.is_empty()).ok_or_else(|| anyhow!("no access_token in the answer"))?;
    Ok(Tokens {
        access: access.to_string(),
        refresh: v["refresh_token"].as_str().map(str::to_string),
        expires_at: v["expires_in"].as_i64().map(|s| now + s),
    })
}

pub async fn exchange_code(s: &Server, client_id: &str, code: &str, redirect_uri: &str, verifier: &str, now: i64) -> Result<Tokens> {
    let form = [("grant_type", "authorization_code"), ("code", code), ("redirect_uri", redirect_uri), ("client_id", client_id), ("code_verifier", verifier)];
    token_call(s, &form, now).await?.map_err(|v| anyhow!("the sign-in was refused: {}", error_text(&v)))
}

pub async fn refresh(s: &Server, client_id: &str, refresh_token: &str, now: i64) -> Result<Tokens> {
    let form = [("grant_type", "refresh_token"), ("refresh_token", refresh_token), ("client_id", client_id)];
    let t = token_call(s, &form, now).await?.map_err(|v| anyhow!("signing in again is needed: {}", error_text(&v)))?;
    // A server that does not rotate refresh tokens keeps the old one valid.
    Ok(Tokens { refresh: t.refresh.or_else(|| Some(refresh_token.to_string())), ..t })
}

pub async fn device_start(s: &Server, client_id: &str, scope: &str) -> Result<DeviceCode> {
    if !s.has_device_flow() {
        bail!("{} has no device sign-in", s.issuer);
    }
    let resp = client()?.post(&s.device_authorization_endpoint).form(&[("client_id", client_id), ("scope", scope)]).send().await.context("starting the device sign-in")?;
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("the device sign-in answered {status}: {}", error_text(&v));
    }
    Ok(serde_json::from_value(v)?)
}

pub async fn device_poll(s: &Server, client_id: &str, device_code: &str, now: i64) -> Result<Poll> {
    let form = [("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("device_code", device_code), ("client_id", client_id)];
    Ok(match token_call(s, &form, now).await? {
        Ok(t) => Poll::Done(t),
        Err(v) => poll_error(&v),
    })
}

pub fn poll_error(v: &Value) -> Poll {
    match v["error"].as_str().unwrap_or("") {
        "authorization_pending" => Poll::Pending,
        "slow_down" => Poll::SlowDown,
        _ => Poll::Failed(error_text(v)),
    }
}

/// Best effort: the server forgets the token; KIQR forgets it either way.
pub async fn revoke(s: &Server, client_id: &str, token: &str) {
    if s.revocation_endpoint.is_empty() {
        return;
    }
    if let Ok(c) = client() {
        let _ = c.post(&s.revocation_endpoint).form(&[("token", token), ("client_id", client_id)]).send().await;
    }
}

pub fn error_text(v: &Value) -> String {
    v["error_description"].as_str().or_else(|| v["error"].as_str()).unwrap_or("no reason given").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_tokens_and_polls() {
        let s = Server { issuer: "https://id.test".into(), authorization_endpoint: "https://id.test/oauth/authorize".into(), token_endpoint: "https://id.test/oauth/token".into(), ..Default::default() };
        let u = url::Url::parse(&authorize_url(&s, "kiqr-1", "http://127.0.0.1:7777/api/signins/callback", "mcp", "st", "verifier-verifier-verifier-verifier-1234").unwrap()).unwrap();
        let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["code_challenge"], crate::connect::pkce_challenge("verifier-verifier-verifier-verifier-1234"));
        assert_eq!((q["client_id"].as_str(), q["scope"].as_str(), q["state"].as_str()), ("kiqr-1", "mcp", "st"));

        let t = read_tokens(&serde_json::json!({ "access_token": "a", "refresh_token": "r", "expires_in": 3600 }), 1000).unwrap();
        assert_eq!(t, Tokens { access: "a".into(), refresh: Some("r".into()), expires_at: Some(4600) });
        assert!(read_tokens(&serde_json::json!({ "error": "x" }), 0).is_err());

        assert_eq!(poll_error(&serde_json::json!({ "error": "authorization_pending" })), Poll::Pending);
        assert_eq!(poll_error(&serde_json::json!({ "error": "slow_down" })), Poll::SlowDown);
        assert_eq!(poll_error(&serde_json::json!({ "error": "expired_token", "error_description": "gone" })), Poll::Failed("gone".into()));
    }
}
