//! `config.yaml`: daemon settings and which model runs which task. No secrets
//! live here; they are referenced by environment variable name and loaded from
//! `.env`.

use crate::Store;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub daemon: DaemonConfig,
    pub crawl: CrawlDefaults,
    pub models: Models,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CrawlDefaults {
    pub max_pages: usize,
    /// Pages fetched in parallel (1–10). Fetching is done by KIQR itself, never by the model.
    pub concurrency: usize,
    /// Pages read by the model in parallel. Reading is queued and never blocks fetching.
    pub ai_concurrency: usize,
    /// Escalate blocked or JavaScript-only pages to UnblockingAPI when a key is
    /// present. Pages that cannot be read are still reported either way.
    pub escalate_to_unblockingapi: bool,
}

/// One model assignment per kind of task. The value is `provider/model`,
/// e.g. `anthropic/claude-haiku-5-5`, `claude-cli/sonnet`, `openai/gpt-5`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Models {
    pub extract: String,
    pub synthesize: String,
    pub chat: String,
}

impl Default for Config {
    fn default() -> Self {
        Self { daemon: DaemonConfig::default(), crawl: CrawlDefaults::default(), models: Models::default() }
    }
}
impl Default for DaemonConfig {
    fn default() -> Self {
        Self { host: "127.0.0.1".into(), port: 7777 }
    }
}
impl Default for CrawlDefaults {
    fn default() -> Self {
        Self { max_pages: 200, concurrency: 5, ai_concurrency: 3, escalate_to_unblockingapi: true }
    }
}
impl Default for Models {
    fn default() -> Self {
        Self {
            extract: "claude-cli/haiku".into(),
            synthesize: "claude-cli/sonnet".into(),
            chat: "claude-cli/sonnet".into(),
        }
    }
}

impl Config {
    pub fn load(store: &Store) -> Result<Self> {
        let path = store.home().config_file();
        match store.read_yaml_opt::<Config>(&path)? {
            Some(c) => Ok(c),
            None => {
                let c = Config::default();
                store.write_yaml(&path, &c)?;
                Ok(c)
            }
        }
    }

    pub fn save(&self, store: &Store) -> Result<()> {
        store.write_yaml(&store.home().config_file(), self)
    }
}
