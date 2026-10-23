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
    pub fn skills_dir(&self) -> PathBuf {
        self.root.join("skills")
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
    pub fn chats_dir(&self, slug: &str) -> PathBuf {
        self.project_dir(slug).join("chats")
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
