//! Fetching pages. The built-in fetcher handles everything that returns real
//! content; pages that come back blocked or as an empty JavaScript shell are
//! escalated to UnblockingAPI when a key is configured, and reported as
//! unreadable otherwise.

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::time::{Duration, Instant};
use url::Url;

pub const USER_AGENT: &str = concat!("KIQR/", env!("CARGO_PKG_VERSION"), " (+https://kiqr.dev; marketing profile crawler)");

#[derive(Clone, Debug)]
pub struct Fetched {
    pub url: Url,
    pub final_url: Url,
    pub status: u16,
    pub content_type: String,
    pub body: String,
    pub via: &'static str,
    pub elapsed: Duration,
}

/// Why the built-in fetcher could not read a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Obstacle {
    Blocked(u16),
    ChallengePage,
    JavaScriptShell,
}

impl Obstacle {
    pub fn reason(&self) -> String {
        match self {
            Obstacle::Blocked(code) => format!("blocked ({code})"),
            Obstacle::ChallengePage => "bot challenge".into(),
            Obstacle::JavaScriptShell => "needs JavaScript".into(),
        }
    }
}

#[async_trait]
pub trait Fetcher: Send + Sync {
    async fn fetch(&self, url: &Url) -> Result<Fetched>;
}

pub struct LocalFetcher {
    client: reqwest::Client,
}

impl LocalFetcher {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;
        Ok(Self { client })
    }
}

#[async_trait]
impl Fetcher for LocalFetcher {
    async fn fetch(&self, url: &Url) -> Result<Fetched> {
        let started = Instant::now();
        let resp = self.client.get(url.clone()).header("Accept", "text/html,application/xhtml+xml").send().await?;
        let status = resp.status().as_u16();
        let final_url = resp.url().clone();
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = resp.text().await.unwrap_or_default();
        Ok(Fetched { url: url.clone(), final_url, status, content_type, body, via: "local", elapsed: started.elapsed() })
    }
}

/// UnblockingAPI: rotating residential proxies and a real browser for the
/// pages the local fetcher cannot read. `POST /unblock` returns cleaned HTML
/// (scripts, styles and cookie banners already stripped); the API decides per
/// site whether to render. A rendered fetch can take up to ~170 s, hence the
/// long timeout. Failures are free; every success costs one credit.
pub struct UnblockingFetcher {
    client: reqwest::Client,
    api_key: String,
    endpoint: String,
}

impl UnblockingFetcher {
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("UNBLOCKINGAPI_KEY").ok().filter(|k| !k.trim().is_empty())?;
        let endpoint = std::env::var("UNBLOCKINGAPI_ENDPOINT").unwrap_or_else(|_| "https://api.unblockingapi.com/unblock".into());
        let client = reqwest::Client::builder().user_agent(USER_AGENT).timeout(Duration::from_secs(180)).build().ok()?;
        Some(Self { client, api_key, endpoint })
    }
}

#[async_trait]
impl Fetcher for UnblockingFetcher {
    async fn fetch(&self, url: &Url) -> Result<Fetched> {
        let started = Instant::now();
        #[derive(serde::Serialize)]
        struct Req<'a> {
            url: &'a str,
        }
        #[derive(serde::Deserialize)]
        struct Resp {
            status: String,
            #[serde(default)]
            http_response_code: Option<u16>,
            #[serde(default)]
            response: Option<serde_json::Value>,
            #[serde(default)]
            error: Option<String>,
        }
        let http = self
            .client
            .post(&self.endpoint)
            .header("X-Api-Key", &self.api_key)
            .json(&Req { url: url.as_str() })
            .send()
            .await
            .context("calling UnblockingAPI")?;
        let code = http.status().as_u16();
        let parsed: Resp = http.json().await.context("reading UnblockingAPI response")?;
        if parsed.status != "succeeded" {
            anyhow::bail!("UnblockingAPI {}: {}", code, parsed.error.unwrap_or_else(|| "request failed".into()));
        }
        let body = match parsed.response {
            Some(serde_json::Value::String(s)) => s,
            Some(other) => other.to_string(),
            None => String::new(),
        };
        Ok(Fetched {
            url: url.clone(),
            final_url: url.clone(),
            status: parsed.http_response_code.unwrap_or(200),
            content_type: "text/html".into(),
            body,
            via: "unblockingapi",
            elapsed: started.elapsed(),
        })
    }
}

/// Decide whether a locally fetched page is usable, and if not, why.
pub fn obstacle(f: &Fetched, text_len: usize, script_len: usize) -> Option<Obstacle> {
    match f.status {
        401 | 403 | 429 | 503 => return Some(Obstacle::Blocked(f.status)),
        _ => {}
    }
    let lower = f.body.to_ascii_lowercase();
    if lower.contains("cf-challenge") || lower.contains("just a moment...") || lower.contains("captcha") && text_len < 400 {
        return Some(Obstacle::ChallengePage);
    }
    // Little visible text next to a lot of script is the signature of a client-rendered shell.
    if text_len < 200 && script_len > 2000 {
        return Some(Obstacle::JavaScriptShell);
    }
    None
}
