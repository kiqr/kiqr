//! LLM providers. Two families: HTTP APIs (key in `.env`) and local CLIs
//! (`claude`, `codex`) that use whatever login the user already has.
//!
//! A model is named `provider/model`:
//! `anthropic/claude-haiku-5-5`, `openai/gpt-5-mini`, `claude-cli/sonnet`,
//! `codex-cli/gpt-5`.

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;

#[derive(Clone, Debug)]
pub struct Request {
    pub system: String,
    pub prompt: String,
    pub model: String,
    /// Ask for a JSON-only answer; providers do what they can to enforce it.
    pub json: bool,
    pub max_tokens: u32,
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn complete(&self, req: &Request) -> Result<String>;
}

/// Resolve a `provider/model` string to a provider and the bare model name.
pub fn resolve(spec: &str) -> Result<(Box<dyn Provider>, String)> {
    let (provider, model) = spec.split_once('/').ok_or_else(|| anyhow!("model spec '{spec}' must be provider/model"))?;
    let p: Box<dyn Provider> = match provider {
        "anthropic" => Box::new(AnthropicApi::from_env()?),
        "openai" => Box::new(OpenAiApi::from_env()?),
        "claude-cli" => Box::new(ClaudeCli),
        "codex-cli" => Box::new(CodexCli),
        other => bail!("unknown provider '{other}'"),
    };
    Ok((p, model.to_string()))
}

/// Is this spec usable right now (key present, binary on PATH)?
pub async fn available(spec: &str) -> bool {
    let Some((provider, _)) = spec.split_once('/') else { return false };
    match provider {
        "anthropic" => has_env("ANTHROPIC_API_KEY"),
        "openai" => has_env("OPENAI_API_KEY"),
        "claude-cli" => which("claude").await,
        "codex-cli" => which("codex").await,
        _ => false,
    }
}

/// The keys KIQR itself uses (the model providers, UnblockingAPI, kiqr.cloud):
/// the machine's `.env`, never an ability's connection.
pub const CORE_KEYS: &[&str] = &["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN", "UNBLOCKINGAPI_KEY", "KIQR_CLOUD_TOKEN"];

/// Where KIQR keeps its own sign-ins (kiqr.cloud, UnblockingAPI by account):
/// tokens, their expiry, and the client id this machine registered. Set by
/// the daemon only, never typed.
pub const SIGNIN_KEYS: &[&str] = &[
    "KIQR_CLOUD_TOKEN", "KIQR_CLOUD_REFRESH_TOKEN", "KIQR_CLOUD_TOKEN_EXPIRES_AT", "KIQR_CLOUD_CLIENT_ID", "KIQR_CLOUD_ACCOUNT",
    "UNBLOCKINGAPI_TOKEN", "UNBLOCKINGAPI_REFRESH_TOKEN", "UNBLOCKINGAPI_TOKEN_EXPIRES_AT", "UNBLOCKINGAPI_CLIENT_ID", "UNBLOCKINGAPI_ACCOUNT",
];

pub fn is_core_key(k: &str) -> bool {
    CORE_KEYS.contains(&k) || SIGNIN_KEYS.contains(&k)
}

pub fn has_env(k: &str) -> bool {
    std::env::var(k).map(|v| !v.trim().is_empty()).unwrap_or(false)
}

pub async fn which(bin: &str) -> bool {
    tokio::process::Command::new(bin)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------- Anthropic API

pub struct AnthropicApi {
    client: reqwest::Client,
    key: String,
}

impl AnthropicApi {
    pub fn from_env() -> Result<Self> {
        let key = std::env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY is not set in .env")?;
        Ok(Self { client: reqwest::Client::new(), key })
    }
}

#[async_trait]
impl Provider for AnthropicApi {
    fn id(&self) -> &'static str {
        "anthropic"
    }
    async fn complete(&self, req: &Request) -> Result<String> {
        let body = serde_json::json!({
            "model": req.model,
            "max_tokens": req.max_tokens,
            "system": req.system,
            "messages": [{"role": "user", "content": req.prompt}],
        });
        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        let v: serde_json::Value = resp.json().await?;
        if !status.is_success() {
            bail!("anthropic {}: {}", status, v["error"]["message"].as_str().unwrap_or("request failed"));
        }
        let text = v["content"]
            .as_array()
            .map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""))
            .unwrap_or_default();
        Ok(text)
    }
}

// ------------------------------------------------------------------- OpenAI API

pub struct OpenAiApi {
    client: reqwest::Client,
    key: String,
    base: String,
}

impl OpenAiApi {
    pub fn from_env() -> Result<Self> {
        let key = std::env::var("OPENAI_API_KEY").context("OPENAI_API_KEY is not set in .env")?;
        let base = std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".into());
        Ok(Self { client: reqwest::Client::new(), key, base })
    }
}

#[async_trait]
impl Provider for OpenAiApi {
    fn id(&self) -> &'static str {
        "openai"
    }
    async fn complete(&self, req: &Request) -> Result<String> {
        let mut body = serde_json::json!({
            "model": req.model,
            "messages": [
                {"role": "system", "content": req.system},
                {"role": "user", "content": req.prompt}
            ],
        });
        if req.json {
            body["response_format"] = serde_json::json!({"type": "json_object"});
        }
        let resp = self.client.post(format!("{}/chat/completions", self.base)).bearer_auth(&self.key).json(&body).send().await?;
        let status = resp.status();
        let v: serde_json::Value = resp.json().await?;
        if !status.is_success() {
            bail!("openai {}: {}", status, v["error"]["message"].as_str().unwrap_or("request failed"));
        }
        Ok(v["choices"][0]["message"]["content"].as_str().unwrap_or_default().to_string())
    }
}

// ------------------------------------------------------------------ Claude CLI

/// Runs `claude -p` with the user's existing login (Pro/Max/Team subscription,
/// or `CLAUDE_CODE_OAUTH_TOKEN` from `claude setup-token` in Docker).
pub struct ClaudeCli;

#[async_trait]
impl Provider for ClaudeCli {
    fn id(&self) -> &'static str {
        "claude-cli"
    }
    async fn complete(&self, req: &Request) -> Result<String> {
        let mut cmd = tokio::process::Command::new("claude");
        cmd.arg("-p")
            .arg("--output-format")
            .arg("json")
            .arg("--model")
            .arg(&req.model)
            .arg("--system-prompt")
            .arg(&req.system)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().context("running `claude` — is Claude Code installed and logged in?")?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(req.prompt.as_bytes()).await?;
        }
        let out = child.wait_with_output().await?;
        if !out.status.success() {
            bail!("claude exited {}: {}", out.status, String::from_utf8_lossy(&out.stderr).trim());
        }
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).context("claude did not return JSON")?;
        if let Some(r) = v["result"].as_str() {
            return Ok(r.to_string());
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

// ------------------------------------------------------------------- Codex CLI

/// Runs `codex exec` non-interactively with the user's ChatGPT login.
pub struct CodexCli;

#[async_trait]
impl Provider for CodexCli {
    fn id(&self) -> &'static str {
        "codex-cli"
    }
    async fn complete(&self, req: &Request) -> Result<String> {
        let prompt = format!("{}\n\n{}", req.system, req.prompt);
        let mut child = tokio::process::Command::new("codex")
            .arg("exec")
            .arg("--model")
            .arg(&req.model)
            .arg("--skip-git-repo-check")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("running `codex` — is the Codex CLI installed and logged in?")?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await?;
        }
        let out = child.wait_with_output().await?;
        if !out.status.success() {
            bail!("codex exited {}: {}", out.status, String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

/// Pull the first JSON value out of a model answer that may be wrapped in
/// prose or a code fence.
pub fn extract_json(text: &str) -> Result<serde_json::Value> {
    let t = text.trim();
    if let Ok(v) = serde_json::from_str(t) {
        return Ok(v);
    }
    let start = t.find(['[', '{']).ok_or_else(|| anyhow!("no JSON in model output"))?;
    let end = t.rfind([']', '}']).ok_or_else(|| anyhow!("no JSON in model output"))?;
    serde_json::from_str(&t[start..=end]).context("model output was not valid JSON")
}
