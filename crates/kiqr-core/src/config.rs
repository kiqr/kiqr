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
    pub agent: AgentConfig,
    /// Which installed ability serves a contract when several could: `image: fal-image`.
    /// One not listed uses the built-in one, else the only ready one.
    pub abilities: std::collections::BTreeMap<String, String>,
    pub marketplace: MarketplaceConfig,
    /// kiqr.cloud: accounts, and the relay that finishes sign-ins for a KIQR agent.
    #[serde(default)]
    pub cloud: CloudConfig,
    /// Built-in abilities the user switched off (`write`, `read-web`, `tasks`…).
    pub core_off: Vec<String>,
}

impl Config {
    /// Is a built-in ability on?
    pub fn core_on(&self, id: &str) -> bool {
        !self.core_off.iter().any(|x| x == id)
    }
}

/// Where the list of abilities comes from. The index is fetched and cached;
/// without a network the shipped list is used.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct MarketplaceConfig {
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CloudConfig {
    pub url: String,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self { url: "https://kiqr.cloud".into() }
    }
}

impl Default for MarketplaceConfig {
    fn default() -> Self {
        Self { url: "https://raw.githubusercontent.com/kiqr/marketplace/main/index.json".into() }
    }
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

/// Seemo's agent sessions: chats and task runs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    /// Spend ceiling for one chat turn or one task run, in USD (API-key billing;
    /// subscriptions ignore it).
    pub max_budget_usd: f64,
    /// Agent processes allowed at once, across all projects.
    pub max_concurrent: usize,
    /// IANA timezone used for schedules and for telling Seemo the local time.
    /// Empty means the machine's timezone.
    pub timezone: String,
}

impl Default for Config {
    fn default() -> Self {
        Self { daemon: DaemonConfig::default(), crawl: CrawlDefaults::default(), models: Models::default(), agent: AgentConfig::default(), abilities: Default::default(), marketplace: MarketplaceConfig::default(), cloud: CloudConfig::default(), core_off: vec![] }
    }
}
impl Default for AgentConfig {
    fn default() -> Self {
        Self { max_budget_usd: 2.0, max_concurrent: 3, timezone: String::new() }
    }
}
impl Default for DaemonConfig {
    fn default() -> Self {
        Self { host: "127.0.0.1".into(), port: 7777 }
    }
}
impl Default for CrawlDefaults {
    fn default() -> Self {
        Self { max_pages: 200, concurrency: 5, ai_concurrency: 3 }
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
