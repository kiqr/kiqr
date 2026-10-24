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
    /// Skills and connections: `skills/<name>/SKILL.md` and/or `extension.yaml`.
    pub fn skills_dir(&self) -> PathBuf {
        self.root.join("skills")
    }
    /// A project's own skills and connections. Committed.
    pub fn project_skills_dir(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("skills")
    }
    /// Publishing channels: `channels/<id>/channel.yaml`. Committed.
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
