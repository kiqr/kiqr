use std::path::{Path, PathBuf};

/// The KIQR home directory and the paths inside it.
///
/// Resolution order: `KIQR_HOME`, then `/data` when it exists and is writable
/// (the Docker convention), then `~/.kiqr`.
#[derive(Clone, Debug)]
pub struct Home {
    root: PathBuf,
}

impl Home {
    pub fn resolve() -> Self {
        if let Ok(p) = std::env::var("KIQR_HOME") {
            return Self { root: PathBuf::from(p) };
        }
        let data = Path::new("/data");
        if data.is_dir() && is_writable(data) {
            return Self { root: data.to_path_buf() };
        }
        let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
        Self { root: home.join(".kiqr") }
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn env_file(&self) -> PathBuf {
        self.root.join(".env")
    }
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.yaml")
    }
    /// Abilities: `abilities/<id>/SKILL.md` (instructions) and/or `extension.yaml` (a connection).
    pub fn abilities_dir(&self) -> PathBuf {
        self.root.join("abilities")
    }
    /// A project's own abilities. Committed.
    pub fn project_abilities_dir(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("abilities")
    }
    /// Folders older homes used before the rename, with where each goes now.
    /// Empty once everything is in place.
    pub fn ability_folder_moves(&self) -> Vec<(PathBuf, PathBuf)> {
        // An empty `abilities/` (the store creates it on open) is as good as none.
        fn vacant(p: &Path) -> bool {
            !p.exists() || std::fs::read_dir(p).map(|mut rd| rd.next().is_none()).unwrap_or(false)
        }
        let mut out = vec![];
        let old = self.root.join("skills");
        if old.is_dir() && vacant(&self.abilities_dir()) {
            out.push((old, self.abilities_dir()));
        }
        if let Ok(rd) = std::fs::read_dir(self.projects_dir()) {
            for p in rd.flatten() {
                let old = p.path().join("skills");
                let new = p.path().join("abilities");
                if old.is_dir() && vacant(&new) {
                    out.push((old, new));
                }
            }
        }
        out
    }

    /// Carry out `ability_folder_moves`. Returns how many folders moved.
    pub fn move_ability_folders(&self) -> std::io::Result<usize> {
        let moves = self.ability_folder_moves();
        for (from, to) in &moves {
            if to.exists() {
                std::fs::remove_dir(to)?;
            }
            std::fs::rename(from, to)?;
        }
        Ok(moves.len())
    }
    /// A project's connections: `connections/<id>/connection.yaml` (committed) and `.env` (not).
    pub fn connections_dir(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("connections")
    }
    /// Where channels were kept before connections; read once, by the migration.
    pub fn channels_dir(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("channels")
    }
    /// Brand files fetched for rendering (logo, icon). Ignored by git, like the cache.
    pub fn brand_files_dir(&self, slug: &str) -> PathBuf {
        self.cache_dir(slug).join("brand")
    }
    /// Scratch space for rendering (HTML handed to a headless browser). Ignored.
    pub fn render_dir(&self) -> PathBuf {
        self.root.join("tmp").join("render")
    }
    pub fn seemo_dir(&self) -> PathBuf {
        self.root.join("seemo")
    }
    /// Asset type definitions: `asset-types/<type>/TYPE.yaml`.
    pub fn asset_types_dir(&self) -> PathBuf {
        self.root.join("asset-types")
    }
    pub fn projects_dir(&self) -> PathBuf {
        self.root.join("projects")
    }
    pub fn project_dir(&self, slug: &str) -> PathBuf {
        self.projects_dir().join(slug)
    }
    pub fn project_file(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("project.yaml")
    }
    pub fn profile_dir(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("profile")
    }
    /// The business brief: one summary document on top of the sections.
    pub fn brief_file(&self, slug: &str) -> PathBuf {
        self.profile_dir(slug).join("brief.yaml")
    }
    /// Brand assets collected from the crawl: logo, colours, fonts, imagery.
    pub fn brand_file(&self, slug: &str) -> PathBuf {
        self.profile_dir(slug).join("brand.yaml")
    }
    pub fn profile_md(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("PROFILE.md")
    }
    pub fn facts_dir(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("facts")
    }
    /// Seemo conversations: `chats/<id>/chat.yaml` + `messages.jsonl`. Committed.
    pub fn chats_dir(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("chats")
    }
    pub fn chat_dir(&self, slug: &str, chat: &str) -> PathBuf {
        self.chats_dir(slug).join(chat)
    }
    /// Metrics snapshots: `metrics/<source>/<YYYY-MM-DD>.yaml`, one per day and source. Committed.
    pub fn metrics_dir(&self, slug: &str, source: &str) -> PathBuf {
        self.project_dir(slug).join("metrics").join(source)
    }
    /// The marketing plan Seemo owns: `marketing/plan.yaml`. Committed.
    pub fn plan_file(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("marketing").join("plan.yaml")
    }
    /// Campaigns: `campaigns/<id>/campaign.yaml`. Committed.
    pub fn campaigns_dir(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("campaigns")
    }
    pub fn campaign_dir(&self, slug: &str, id: &str) -> PathBuf {
        self.campaigns_dir(slug).join(id)
    }
    /// Scheduled tasks: `tasks/<id>/task.yaml` and their runs in `tasks/<id>/runs/<ts>.yaml`. Committed.
    pub fn tasks_dir(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("tasks")
    }
    pub fn task_dir(&self, slug: &str, task: &str) -> PathBuf {
        self.tasks_dir(slug).join(task)
    }
    pub fn runs_dir(&self, slug: &str, task: &str) -> PathBuf {
        self.task_dir(slug, task).join("runs")
    }
    /// Working directory for the agent processes Seemo runs in. Empty on
    /// purpose, so nothing from the daemon's own directory leaks into them;
    /// `tmp/` is ignored by git.
    pub fn agent_dir(&self) -> PathBuf {
        self.root.join("tmp").join("agent")
    }
    /// Generated assets: `assets/<type>/<id>/`. Ignored by git.
    pub fn assets_dir(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("assets")
    }
    /// The project's own secrets, read before the global `.env`. Ignored by git.
    pub fn project_env_file(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join(".env")
    }
    /// Raw fetches and converted markdown. Ignored by git.
    pub fn cache_dir(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("cache")
    }
}

fn is_writable(p: &Path) -> bool {
    let probe = p.join(".kiqr-write-probe");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}
