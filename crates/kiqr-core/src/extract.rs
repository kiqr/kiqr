//! Fact extraction (per page) and profile synthesis (per section). The prompts
//! are the built-in skills; a copy in `~/.kiqr/skills/<name>/SKILL.md`
//! overrides the built-in one so users can tune them.

use crate::events::{EventBus, EventKind};
use crate::profile::{self, Evidence, Fact, Field, Section, SECTIONS};
use crate::project::Project;
use crate::provider::{self, Request};
use crate::{Config, Store};
use anyhow::{Context, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};
use url::Url;

const BUILTIN_EXTRACT: &str = include_str!("../../../skills/extract-facts/SKILL.md");
const BUILTIN_BUILD: &str = include_str!("../../../skills/build-profile/SKILL.md");

/// Install the built-in skills into the home directory if they are missing,
/// so users see and can edit them.
pub fn install_builtin_skills(store: &Store) -> Result<bool> {
    let mut wrote = false;
    for (name, body) in [
        ("extract-facts", BUILTIN_EXTRACT),
        ("build-profile", BUILTIN_BUILD),
        ("crawl-site", include_str!("../../../skills/crawl-site/SKILL.md")),
    ] {
        let path = store.home().skills_dir().join(name).join("SKILL.md");
        if !path.exists() {
            store.write_text(&path, body)?;
            wrote = true;
        }
    }
    Ok(wrote)
}

fn skill_text(store: &Store, name: &str, builtin: &str) -> String {
    let path = store.home().skills_dir().join(name).join("SKILL.md");
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| builtin.to_string());
    strip_frontmatter(&text)
}

fn strip_frontmatter(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            return rest[end + 4..].trim().to_string();
        }
    }
    s.trim().to_string()
}

pub struct Extractor {
    store: Store,
    bus: EventBus,
    config: Config,
}

impl Extractor {
    pub fn new(store: Store, bus: EventBus, config: Config) -> Self {
        Self { store, bus, config }
    }

    /// Extract facts from one page and write them to `facts/<hash>.yaml`.
    pub async fn extract_page(&self, slug: &str, url: &Url, title: &str, markdown: &str) -> Result<Vec<Fact>> {
        if markdown.trim().len() < 40 {
            return Ok(vec![]);
        }
        let (prov, model) = provider::resolve(&self.config.models.extract)?;
        let system = skill_text(&self.store, "extract-facts", BUILTIN_EXTRACT);
        let body = truncate(markdown, 60_000);
        let prompt = format!("URL: {url}\nTitle: {title}\n\n{body}");
        let text = prov.complete(&Request { system, prompt, model, json: true, max_tokens: 8000 }).await?;
        let value = provider::extract_json(&text)?;
        let mut facts: Vec<Fact> = serde_json::from_value(value).context("facts were not the expected shape")?;
        for f in &mut facts {
            f.url = url.to_string();
            f.confidence = f.confidence.clamp(0.0, 1.0);
            if !SECTIONS.iter().any(|(s, _, _)| *s == f.section) {
                f.section = "company".into();
            }
        }
        let key = hash(url.as_str());
        self.store.write_yaml(&self.store.home().facts_dir(slug).join(format!("{key}.yaml")), &facts)?;
        Ok(facts)
    }

    /// Merge all facts into the profile, section by section, honouring locks,
    /// then render PROFILE.md and commit.
    pub async fn synthesize(&self, project: &Project, automated: bool) -> Result<()> {
        let slug = &project.slug;
        let facts = profile::load_facts(&self.store, slug)?;
        let (prov, model) = provider::resolve(&self.config.models.synthesize)?;
        let system = skill_text(&self.store, "build-profile", BUILTIN_BUILD);

        for (id, title, desc) in SECTIONS {
            let mut section = Section::load(&self.store, slug, id)?;
            let mine: Vec<&Fact> = facts.iter().filter(|f| f.section == *id).collect();
            if mine.is_empty() && section.fields.is_empty() {
                continue;
            }
            let prompt = serde_json::json!({
                "section": { "id": id, "title": title, "description": desc },
                "current_fields": section.fields,
                "facts": mine,
            })
            .to_string();
            let text = prov.complete(&Request { system: system.clone(), prompt, model: model.clone(), json: true, max_tokens: 12_000 }).await?;
            let value = provider::extract_json(&text)?;
            #[derive(serde::Deserialize)]
            struct Out {
                fields: Vec<Field>,
            }
            let out: Out = serde_json::from_value(value).context("section output was not the expected shape")?;

            // Locked fields are never changed by synthesis, whatever the model returned.
            let locked: Vec<Field> = section.fields.iter().filter(|f| f.locked).cloned().collect();
            let mut fields: Vec<Field> = out.fields.into_iter().filter(|f| !locked.iter().any(|l| l.name == f.name)).collect();
            for f in fields.iter_mut() {
                f.locked = false;
                f.evidence.truncate(5);
                f.confidence = f.confidence.clamp(0.0, 1.0);
                dedupe_evidence(&mut f.evidence);
            }
            fields.extend(locked);
            section.fields = fields;
            section.updated_at = Some(Utc::now());
            section.save(&self.store, slug)?;
            self.bus.emit(Some(slug), EventKind::ProfileUpdated { section: id.to_string(), by: "seemo".into() });
        }

        profile::write_markdown(&self.store, slug, &project.name, &project.url)?;
        let prefix = if automated { "[auto] " } else { "" };
        self.store.commit(&format!("{prefix}profile({slug}): synthesize from {} facts", facts.len())).await?;
        Ok(())
    }
}

fn dedupe_evidence(ev: &mut Vec<Evidence>) {
    let mut seen = std::collections::HashSet::new();
    ev.retain(|e| seen.insert((e.url.clone(), e.quote.trim().to_string())));
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n\n[truncated]", &s[..end])
}

fn hash(s: &str) -> String {
    let d = Sha256::digest(s.as_bytes());
    d.iter().take(12).map(|b| format!("{b:02x}")).collect()
}
