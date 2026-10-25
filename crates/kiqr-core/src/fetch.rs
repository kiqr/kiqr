//! Fetching pages. The built-in fetcher handles everything that returns real
//! content; pages that come back blocked or as an empty JavaScript shell are
//! escalated to UnblockingAPI when a key is configured, and reported as
//! unreadable otherwise.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
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
    /// Where a redirect the fetcher would not follow pointed: another website.
    pub redirect_to: Option<Url>,
}

/// `www.acme.com` and `acme.com` are the same site.
pub fn site_host(u: &Url) -> String {
    u.host_str().unwrap_or("").trim_start_matches("www.").to_ascii_lowercase()
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
            // Follow redirects within the site (http→https, www, moved pages), never
            // to another website: those are outbound or affiliate links, and the
            // caller sees the 3xx with `redirect_to` instead of the other site's page.
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                let same = attempt.previous().first().is_some_and(|first| site_host(first) == site_host(attempt.url()));
                if attempt.previous().len() >= 5 {
                    attempt.error("too many redirects")
                } else if same {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
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
        let redirect_to = if resp.status().is_redirection() {
            resp.headers().get(reqwest::header::LOCATION).and_then(|v| v.to_str().ok()).and_then(|l| final_url.join(l).ok())
        } else {
            None
        };
        let body = resp.text().await.unwrap_or_default();
        Ok(Fetched { url: url.clone(), final_url, status, content_type, body, via: "local", elapsed: started.elapsed(), redirect_to })
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
    /// With the machine's key from the environment.
    pub fn from_env() -> Option<Self> {
        Self::with_key(std::env::var("UNBLOCKINGAPI_KEY").ok())
    }

    /// With a key the caller resolved: a project's own account first, the
    /// machine's as the fallback (`secrets::lookup`). None: no escalation.
    pub fn with_key(key: Option<String>) -> Option<Self> {
        let api_key = key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty())?;
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
            redirect_to: None,
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

/// One web page as Seemo reads it: the same path a crawl takes (local fetch,
/// then UnblockingAPI when the page is blocked or an empty JavaScript shell
/// and escalation is on), returned as Markdown instead of being cached.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadPage {
    pub url: String,
    pub final_url: String,
    pub status: u16,
    /// `local` or `unblockingapi`.
    pub via: String,
    pub title: String,
    pub description: Option<String>,
    pub markdown: String,
    pub words: usize,
    /// Set when the page could not be read properly; `markdown` is then whatever came back.
    pub obstacle: Option<String>,
    /// True when `markdown` was cut at the caller's limit.
    pub truncated: bool,
}

/// Read one page. `unblocking_key` is the UnblockingAPI key to escalate
/// with when the page cannot be read (none: no escalation); `max_chars` caps
/// the Markdown (0 means no cap).
pub async fn read_page(url: &Url, unblocking_key: Option<String>, max_chars: usize) -> Result<ReadPage> {
    anyhow::ensure!(matches!(url.scheme(), "http" | "https"), "only http and https pages can be read");
    let local = LocalFetcher::new()?;
    let mut fetched = local.fetch(url).await?;
    let mut page = crate::markdown::parse(&fetched.final_url, &fetched.body);
    let mut hit = obstacle(&fetched, page.text_len, page.script_len);
    if hit.is_some() {
        if let Some(unblocking) = UnblockingFetcher::with_key(unblocking_key) {
            match unblocking.fetch(url).await {
                Ok(f) => {
                    page = crate::markdown::parse(&f.final_url, &f.body);
                    fetched = f;
                    hit = None;
                }
                Err(e) => tracing::warn!(%url, error = %e, "escalation failed"),
            }
        }
    }
    if !fetched.content_type.is_empty() && !fetched.content_type.contains("html") && !fetched.content_type.contains("text/") {
        anyhow::bail!("{} is {}, not a web page", fetched.final_url, fetched.content_type);
    }
    let (markdown, truncated) = cut(&page.markdown, max_chars);
    Ok(ReadPage {
        url: url.to_string(),
        final_url: fetched.final_url.to_string(),
        status: fetched.status,
        via: fetched.via.to_string(),
        title: page.title,
        description: page.description,
        words: page.markdown.split_whitespace().count(),
        markdown,
        obstacle: hit.map(|o| o.reason()),
        truncated,
    })
}

/// Cut text at `max` characters on a line boundary where possible; 0 means no cut.
pub fn cut(text: &str, max: usize) -> (String, bool) {
    if max == 0 || text.chars().count() <= max {
        return (text.to_string(), false);
    }
    let head: String = text.chars().take(max).collect();
    let at = head.rfind('\n').filter(|&i| i > max / 2).unwrap_or(head.len());
    (head[..at].trim_end().to_string(), true)
}

#[cfg(test)]
mod read_tests {
    use super::*;

    #[test]
    fn cut_keeps_whole_lines() {
        let text = "one line\ntwo line\nthree line\n";
        assert_eq!(cut(text, 0), (text.to_string(), false));
        assert_eq!(cut(text, 100), (text.to_string(), false));
        assert_eq!(cut(text, 20), ("one line\ntwo line".to_string(), true));
        // No usable line break in the second half: a hard cut.
        assert_eq!(cut("abcdefghij", 4), ("abcd".to_string(), true));
    }
}
