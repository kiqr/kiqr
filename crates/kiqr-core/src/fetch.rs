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
/// pages the local fetcher cannot read. With an API key, `POST /unblock`; with
/// an account the user signed in to instead (an OAuth token), the same fetch
/// through UnblockingAPI's hosted MCP server (tool `unblock_fetch`), so KIQR
/// never holds the key. Either returns cleaned HTML (scripts, styles and cookie
/// banners already stripped); the API decides per site whether to render. A
/// rendered fetch can take up to ~170 s, hence the long timeout. Failures are
/// free; every success costs one credit.
pub struct UnblockingFetcher {
    client: reqwest::Client,
    auth: UnblockingAuth,
    endpoint: String,
}

/// How KIQR reaches UnblockingAPI: a key the user pasted, or a token from
/// signing in (`Bearer`, refreshed by the daemon before it builds the fetcher).
#[derive(Clone, Debug, PartialEq)]
pub enum UnblockingAuth {
    Key(String),
    Token(String),
}

impl UnblockingAuth {
    /// A key, when there is one.
    pub fn key(k: Option<String>) -> Option<Self> {
        k.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).map(Self::Key)
    }
}

/// UnblockingAPI's hosted MCP server; `UNBLOCKINGAPI_MCP_URL` overrides it.
pub fn unblocking_mcp_url() -> String {
    std::env::var("UNBLOCKINGAPI_MCP_URL").unwrap_or_else(|_| "https://mcp.unblockingapi.com/mcp".into())
}

/// The largest page the MCP tool returns (its `max_chars` ceiling).
const MCP_MAX_CHARS: u32 = 140_000;

impl UnblockingFetcher {
    /// With the machine's key from the environment.
    pub fn from_env() -> Option<Self> {
        Self::with_auth(UnblockingAuth::key(std::env::var("UNBLOCKINGAPI_KEY").ok()))
    }

    /// With what the caller resolved: a project's own key, the machine's, or
    /// the machine's sign-in. None: no escalation.
    pub fn with_auth(auth: Option<UnblockingAuth>) -> Option<Self> {
        let auth = auth?;
        let endpoint = match &auth {
            UnblockingAuth::Key(_) => std::env::var("UNBLOCKINGAPI_ENDPOINT").unwrap_or_else(|_| "https://api.unblockingapi.com/unblock".into()),
            UnblockingAuth::Token(_) => unblocking_mcp_url(),
        };
        let client = reqwest::Client::builder().user_agent(USER_AGENT).timeout(Duration::from_secs(180)).build().ok()?;
        Some(Self { client, auth, endpoint })
    }

    /// `tools/call unblock_fetch` on the MCP server. Stateless: no handshake
    /// first. The answer is one server-sent event whose text is the fetch's
    /// metadata as JSON, notes, a `---` line and the page.
    async fn fetch_mcp(&self, url: &Url, token: &str) -> Result<(u16, String)> {
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "unblock_fetch", "arguments": { "url": url.as_str(), "format": "html", "max_chars": MCP_MAX_CHARS } }
        });
        let resp = self
            .client
            .post(&self.endpoint)
            .bearer_auth(token)
            .header("Accept", "application/json, text/event-stream")
            .json(&body)
            .send()
            .await
            .context("calling UnblockingAPI")?;
        let status = resp.status();
        let text = resp.text().await.context("reading UnblockingAPI's answer")?;
        if status.as_u16() == 401 {
            anyhow::bail!("UnblockingAPI: the sign-in has expired; sign in again under Settings");
        }
        parse_mcp_fetch(&text).with_context(|| format!("UnblockingAPI {status}"))
    }
}

/// The JSON-RPC answer inside an SSE (`data:` lines) or plain JSON body, then
/// the fetch inside the tool's text: `(http status of the page, html)`.
pub fn parse_mcp_fetch(body: &str) -> Result<(u16, String)> {
    let json_text: String = if body.trim_start().starts_with('{') {
        body.to_string()
    } else {
        body.lines().filter_map(|l| l.strip_prefix("data:")).map(str::trim_start).collect::<Vec<_>>().join("\n")
    };
    let v: serde_json::Value = serde_json::from_str(&json_text).context("the answer is not JSON-RPC")?;
    if let Some(e) = v.get("error") {
        anyhow::bail!("{}", e["message"].as_str().unwrap_or("request failed"));
    }
    let result = &v["result"];
    let text = result["content"].as_array().and_then(|c| c.iter().find_map(|p| p["text"].as_str())).unwrap_or("");
    if result["isError"].as_bool() == Some(true) {
        let reason = text.lines().next().unwrap_or("request failed").trim_start_matches("Request failed: ");
        anyhow::bail!("{reason}");
    }
    // Metadata JSON first, then notes, then "---", then the page itself.
    let (head, page) = text.split_once("\n\n---\n\n").ok_or_else(|| anyhow::anyhow!("the answer has no page"))?;
    let meta_end = head.find("\n\n").unwrap_or(head.len());
    let meta: serde_json::Value = serde_json::from_str(&head[..meta_end]).unwrap_or(serde_json::Value::Null);
    let code = meta["http_response_code"].as_u64().unwrap_or(200) as u16;
    Ok((code, page.to_string()))
}

#[async_trait]
impl Fetcher for UnblockingFetcher {
    async fn fetch(&self, url: &Url) -> Result<Fetched> {
        let started = Instant::now();
        let api_key = match &self.auth {
            UnblockingAuth::Key(k) => k.as_str(),
            UnblockingAuth::Token(t) => {
                let (status, body) = self.fetch_mcp(url, t).await?;
                return Ok(Fetched { url: url.clone(), final_url: url.clone(), status, content_type: "text/html".into(), body, via: "unblockingapi", elapsed: started.elapsed(), redirect_to: None });
            }
        };
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
            .header("X-Api-Key", api_key)
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

/// Read one page. `unblocking` is how to reach UnblockingAPI to escalate
/// with when the page cannot be read (none: no escalation); `max_chars` caps
/// the Markdown (0 means no cap).
pub async fn read_page(url: &Url, unblocking: Option<UnblockingAuth>, max_chars: usize) -> Result<ReadPage> {
    anyhow::ensure!(matches!(url.scheme(), "http" | "https"), "only http and https pages can be read");
    let local = LocalFetcher::new()?;
    let mut fetched = local.fetch(url).await?;
    let mut page = crate::markdown::parse(&fetched.final_url, &fetched.body);
    let mut hit = obstacle(&fetched, page.text_len, page.script_len);
    if hit.is_some() {
        if let Some(unblocking) = UnblockingFetcher::with_auth(unblocking) {
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

#[cfg(test)]
mod mcp_tests {
    use super::parse_mcp_fetch;

    #[test]
    fn reads_the_tool_answer() {
        let text = "{\n  \"status\": \"succeeded\",\n  \"http_response_code\": 203\n}\n\nCaptured without scrolling.\n\n---\n\n<html><p>hi</p></html>";
        let sse = format!("event: message\ndata: {}\n\n", serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": { "content": [{ "type": "text", "text": text }] } }));
        assert_eq!(parse_mcp_fetch(&sse).unwrap(), (203, "<html><p>hi</p></html>".to_string()));
        let failed = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": { "isError": true, "content": [{ "type": "text", "text": "Request failed: out of credits\n\n{}" }] } }).to_string();
        assert_eq!(parse_mcp_fetch(&failed).unwrap_err().to_string(), "out of credits");
        let rpc = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "error": { "code": -32001, "message": "Sign in required" } }).to_string();
        assert!(parse_mcp_fetch(&rpc).unwrap_err().to_string().contains("Sign in required"));
    }
}
