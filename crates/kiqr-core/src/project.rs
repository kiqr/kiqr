//! Projects: one per website, stored at `projects/<slug>/project.yaml`.

use crate::{Store, events::{EventBus, EventKind}};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub slug: String,
    pub name: String,
    pub url: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub crawl: CrawlSettings,
    #[serde(default)]
    pub last_crawl: Option<CrawlSummary>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CrawlSettings {
    /// `None` means the global default from `config.yaml`.
    pub max_pages: Option<usize>,
    pub concurrency: Option<usize>,
    pub ai_concurrency: Option<usize>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrawlSummary {
    pub finished_at: DateTime<Utc>,
    pub pages: usize,
    pub blocked: usize,
    pub failed: usize,
}

impl Project {
    pub fn slug_for(url: &Url) -> String {
        let host = url.host_str().unwrap_or("site").trim_start_matches("www.");
        host.chars()
            .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    }

    pub async fn create(store: &Store, bus: &EventBus, raw_url: &str) -> Result<Project> {
        let mut url = Url::parse(raw_url).or_else(|_| Url::parse(&format!("https://{raw_url}")))?;
        if url.scheme() != "https" && url.scheme() != "http" {
            return Err(anyhow!("only http(s) URLs are supported"));
        }
        url.set_fragment(None);
        let slug = Self::slug_for(&url);
        let path = store.home().project_file(&slug);
        if path.exists() {
            return Err(anyhow!("project '{slug}' already exists"));
        }
        let project = Project {
            name: url.host_str().unwrap_or(&slug).trim_start_matches("www.").to_string(),
            slug: slug.clone(),
            url: url.to_string(),
            created_at: Utc::now(),
            crawl: CrawlSettings::default(),
            last_crawl: None,
        };
        store.write_yaml(&path, &project)?;
        for dir in [store.home().profile_dir(&slug), store.home().facts_dir(&slug), store.home().chats_dir(&slug)] {
            std::fs::create_dir_all(&dir)?;
            std::fs::write(dir.join(".gitkeep"), "")?;
        }
        store.commit(&format!("project({slug}): add {}", project.url)).await?;
        bus.emit(Some(&slug), EventKind::ProjectCreated { slug: slug.clone(), url: project.url.clone() });
        Ok(project)
    }

    pub fn load(store: &Store, slug: &str) -> Result<Project> {
        store
            .read_yaml_opt(&store.home().project_file(slug))?
            .ok_or_else(|| anyhow!("no project '{slug}'"))
    }

    pub fn save(&self, store: &Store) -> Result<()> {
        store.write_yaml(&store.home().project_file(&self.slug), self)
    }

    pub fn list(store: &Store) -> Result<Vec<Project>> {
        let dir = store.home().projects_dir();
        let mut out = Vec::new();
        if dir.exists() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let file = entry.path().join("project.yaml");
                if file.exists() {
                    out.push(store.read_yaml::<Project>(&file)?);
                }
            }
        }
        out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(out)
    }
}
