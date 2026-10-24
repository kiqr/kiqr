//! Fact extraction (per page) and profile synthesis (per section). The prompts
//! are the built-in skills; a copy in `~/.kiqr/skills/<name>/SKILL.md`
//! overrides the built-in one so users can tune them.

use crate::events::{EventBus, EventKind};
use crate::crawl::CachedPage;
use crate::profile::{self, Brief, BriefSource, Evidence, Fact, Field, Section, TopicCluster, SECTIONS};
use crate::project::Project;
use crate::provider::{self, Request};
use crate::{Config, Store};
use anyhow::{Context, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};
use url::Url;

const BUILTIN_EXTRACT: &str = include_str!("../../../skills/extract-facts/SKILL.md");
const BUILTIN_BUILD: &str = include_str!("../../../skills/build-profile/SKILL.md");
const BUILTIN_BRIEF: &str = include_str!("../../../skills/business-brief/SKILL.md");
const BUILTIN_POSTS: &str = include_str!("../../../skills/social-posts/SKILL.md");

/// Install the built-in skills into the home directory if they are missing,
/// so users see and can edit them.
pub fn install_builtin_skills(store: &Store) -> Result<bool> {
    let mut wrote = false;
    for (name, body) in [
        ("extract-facts", BUILTIN_EXTRACT),
        ("build-profile", BUILTIN_BUILD),
        ("business-brief", BUILTIN_BRIEF),
        ("crawl-site", include_str!("../../../skills/crawl-site/SKILL.md")),
        ("social-posts", BUILTIN_POSTS),
        ("seemo-agent", crate::agent::BUILTIN_GUIDE),
    ] {
        let path = store.home().skills_dir().join(name).join("SKILL.md");
        if !path.exists() {
            store.write_text(&path, body)?;
            wrote = true;
        }
    }
    Ok(wrote)
}

/// `x` → `X`, `linkedin` → `LinkedIn`.
fn network_name(n: &str) -> String {
    match n {
        "x" | "twitter" => "X".into(),
        "linkedin" => "LinkedIn".into(),
        "youtube" => "YouTube".into(),
        "tiktok" => "TikTok".into(),
        other => {
            let mut c = other.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
    }
}

/// A skill's text: the user's copy in `~/.kiqr/skills/<name>/SKILL.md`, else
/// the built-in one. Frontmatter stripped.
pub fn skill_text(store: &Store, name: &str, builtin: &str) -> String {
    let path = store.home().skills_dir().join(name).join("SKILL.md");
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| builtin.to_string());
    strip_frontmatter(&text)
}

/// The built-in text of a skill, if it ships with KIQR.
pub fn builtin_skill(name: &str) -> Option<&'static str> {
    match name {
        "extract-facts" => Some(BUILTIN_EXTRACT),
        "build-profile" => Some(BUILTIN_BUILD),
        "business-brief" => Some(BUILTIN_BRIEF),
        "crawl-site" => Some(include_str!("../../../skills/crawl-site/SKILL.md")),
        "social-posts" => Some(BUILTIN_POSTS),
        "seemo-agent" => Some(crate::agent::BUILTIN_GUIDE),
        _ => None,
    }
}

/// Skill names: built-in ones plus any folder the user added.
pub fn skill_names(store: &Store) -> Vec<String> {
    let mut names: Vec<String> = ["extract-facts", "build-profile", "business-brief", "crawl-site", "social-posts", "seemo-agent"].iter().map(|s| s.to_string()).collect();
    if let Ok(rd) = std::fs::read_dir(store.home().skills_dir()) {
        for e in rd.flatten() {
            if e.path().join("SKILL.md").exists() {
                let n = e.file_name().to_string_lossy().to_string();
                if !names.contains(&n) {
                    names.push(n);
                }
            }
        }
    }
    names
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
    /// render PROFILE.md and commit; then write the business brief. Emits
    /// `SynthesisStarted` and always `SynthesisFinished`; on failure Seemo
    /// says what failed.
    pub async fn synthesize(&self, project: &Project, automated: bool) -> Result<()> {
        let slug = &project.slug;
        let mut done = 0usize;
        let mut sections_saved = false;
        let result = self.synthesize_inner(project, automated, &mut done, &mut sections_saved).await;
        if let Err(e) = &result {
            let text = if sections_saved {
                format!("I updated the profile sections, but I couldn't write the business brief: {e}")
            } else {
                format!("I couldn't build the profile: {e}")
            };
            self.bus.emit(Some(slug), EventKind::SeemoSays { text });
        }
        self.bus.emit(Some(slug), EventKind::SynthesisFinished { sections: done });
        result
    }

    async fn synthesize_inner(&self, project: &Project, automated: bool, done: &mut usize, sections_saved: &mut bool) -> Result<()> {
        let slug = &project.slug;
        // Fields that came from other websites' pages go before the model sees the profile.
        let purged = profile::purge_offsite(&self.store, slug)?;
        if purged > 0 {
            tracing::info!(%slug, purged, "dropped profile fields that came from other websites");
        }
        let facts = profile::load_facts(&self.store, slug)?;
        let (prov, model) = provider::resolve(&self.config.models.synthesize)?;
        let system = skill_text(&self.store, "build-profile", BUILTIN_BUILD);

        // Decide up front which sections have anything to synthesize, so the UI
        // knows how many steps to expect.
        let mut todo = Vec::new();
        for (id, title, desc) in SECTIONS {
            let section = Section::load(&self.store, slug, id)?;
            let mine: Vec<&Fact> = facts.iter().filter(|f| f.section == *id).collect();
            if mine.is_empty() && section.fields.is_empty() {
                continue;
            }
            todo.push((*id, *title, *desc, section, mine));
        }
        self.bus.emit(Some(slug), EventKind::SynthesisStarted { total: todo.len() + 1 });

        for (id, title, desc, mut section, mine) in todo {
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
            let out: Out = serde_json::from_value(value).with_context(|| format!("the {title} section was not the expected shape"))?;

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
            *done += 1;
            self.bus.emit(
                Some(slug),
                EventKind::ProfileUpdated {
                    section: id.to_string(),
                    by: "seemo".into(),
                    title: section.title.clone(),
                    items: profile::section_items(&section),
                },
            );
        }

        profile::write_markdown(&self.store, slug, &project.name, &project.url)?;
        let prefix = if automated { "[auto] " } else { "" };
        self.store.commit(&format!("{prefix}profile({slug}): synthesize from {} facts", facts.len())).await?;
        *sections_saved = true;

        self.brief_inner(project, automated).await?;
        *done += 1;
        Ok(())
    }

    /// Regenerate only the business brief from the current sections and
    /// pages. Emits `SynthesisStarted { total: 1 }` and `SynthesisFinished`.
    pub async fn brief(&self, project: &Project, automated: bool) -> Result<()> {
        let slug = &project.slug;
        self.bus.emit(Some(slug), EventKind::SynthesisStarted { total: 1 });
        let result = self.brief_inner(project, automated).await;
        if let Err(e) = &result {
            self.bus.emit(Some(slug), EventKind::SeemoSays { text: format!("I couldn't write the business brief: {e}") });
        }
        self.bus.emit(Some(slug), EventKind::SynthesisFinished { sections: usize::from(result.is_ok()) });
        result.map(|_| ())
    }

    /// Write `count` social media posts from the business brief and add them to
    /// the library as drafts. Uses the chat model: this is Seemo writing.
    pub async fn social_posts(&self, project: &Project, count: usize) -> Result<Vec<crate::assets::Asset>> {
        let slug = &project.slug;
        let result = self.social_posts_inner(project, count).await;
        match &result {
            Ok(made) => {
                let n = made.len();
                self.bus.emit(Some(slug), EventKind::SeemoSays { text: format!("I wrote {n} post{} for you to review in the library.", if n == 1 { "" } else { "s" }) })
            }
            Err(e) => self.bus.emit(Some(slug), EventKind::SeemoSays { text: format!("I couldn't write the posts: {e}") }),
        }
        result
    }

    async fn social_posts_inner(&self, project: &Project, count: usize) -> Result<Vec<crate::assets::Asset>> {
        let slug = &project.slug;
        let brief = Brief::load(&self.store, slug)?.ok_or_else(|| anyhow::anyhow!("there is no business profile yet; crawl the site first"))?;
        let networks: Vec<String> = crate::brand::Brand::load(&self.store, slug)?.map(|b| b.social.into_iter().map(|s| s.network.to_lowercase()).collect()).unwrap_or_default();
        let (prov, model) = provider::resolve(&self.config.models.chat)?;
        let system = skill_text(&self.store, "social-posts", BUILTIN_POSTS);
        let mut b = serde_json::to_value(&brief)?;
        // The brief's page lists are for the content map, not for writing posts.
        if let Some(clusters) = b.get_mut("topic_clusters").and_then(|v| v.as_array_mut()) {
            for c in clusters {
                if let Some(o) = c.as_object_mut() {
                    o.remove("urls");
                }
            }
        }
        let prompt = serde_json::json!({ "brief": b, "networks": networks, "count": count }).to_string();
        let text = prov.complete(&Request { system, prompt, model, json: true, max_tokens: 6_000 }).await?;
        let value = provider::extract_json(&text)?;

        #[derive(serde::Deserialize, Default)]
        #[serde(default)]
        struct PostOut {
            network: String,
            title: String,
            text: String,
        }
        let posts: Vec<PostOut> = value.get("posts").cloned().map(serde_json::from_value).transpose()?.unwrap_or_default();
        let posts: Vec<PostOut> = posts.into_iter().filter(|p| !p.text.trim().is_empty()).take(count).collect();
        if posts.is_empty() {
            anyhow::bail!("the model returned no posts");
        }
        let mut made = Vec::new();
        for p in posts {
            let network = p.network.trim().to_lowercase();
            let title = if p.title.trim().is_empty() { p.text.chars().take(48).collect::<String>() } else { p.title.trim().to_string() };
            let mut source = serde_json::Map::new();
            source.insert("brief_version".into(), brief.version.into());
            let a = crate::assets::create(
                &self.store,
                slug,
                crate::assets::NewAsset {
                    kind: "post".into(),
                    title,
                    summary: if network.is_empty() { String::new() } else { format!("For {}", network_name(&network)) },
                    tags: if network.is_empty() { vec![] } else { vec![network] },
                    by: Some("seemo".into()),
                    source,
                    files: vec![crate::assets::NewTextFile { role: "body".into(), name: "post.md".into(), text: p.text.trim().to_string() + "\n" }],
                    ..Default::default()
                },
            )?;
            self.bus.emit(Some(slug), EventKind::AssetSaved { asset_type: a.kind.clone(), id: a.id.clone(), title: a.title.clone(), status: "draft".into(), created: true });
            made.push(a);
        }
        Ok(made)
    }

    async fn brief_inner(&self, project: &Project, automated: bool) -> Result<Brief> {
        let slug = &project.slug;
        profile::purge_offsite(&self.store, slug)?;
        let (prov, model) = provider::resolve(&self.config.models.synthesize)?;
        let system = skill_text(&self.store, "business-brief", BUILTIN_BRIEF);

        let sections: Vec<serde_json::Value> = profile::load_all(&self.store, slug)?
            .into_iter()
            .filter(|s| !s.fields.is_empty())
            .map(|s| {
                serde_json::json!({
                    "section": s.id,
                    "title": s.title,
                    "fields": s.fields.iter().map(|f| serde_json::json!({ "name": f.name, "value": f.value })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let pages = self.page_list(slug)?;
        let page_json: Vec<serde_json::Value> = pages
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let path = Url::parse(&p.url).map(|u| match u.query() {
                    Some(q) => format!("{}?{q}", u.path()),
                    None => u.path().to_string(),
                });
                let mut v = serde_json::json!({ "n": i + 1, "path": path.unwrap_or_else(|_| p.url.clone()) });
                if !p.title.is_empty() {
                    v["title"] = p.title.clone().into();
                }
                if !p.facts.is_empty() {
                    v["facts"] = p.facts.clone().into();
                }
                v
            })
            .collect();
        if sections.is_empty() && pages.is_empty() {
            anyhow::bail!("there is nothing to write it from yet; crawl the site first");
        }
        let prompt = serde_json::json!({ "website": project.url, "sections": sections, "pages": page_json }).to_string();
        let text = prov.complete(&Request { system, prompt, model, json: true, max_tokens: 16_000 }).await?;
        let value = provider::extract_json(&text)?;

        #[derive(serde::Deserialize, Default)]
        #[serde(default)]
        struct ClusterOut {
            label: String,
            description: String,
            #[serde(alias = "page_numbers", alias = "pageNumbers")]
            pages: Vec<serde_json::Value>,
        }
        let clusters: Vec<ClusterOut> = value
            .get("topic_clusters")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .context("topic clusters were not the expected shape")?
            .unwrap_or_default();
        let mut brief: Brief = {
            let mut v = value;
            if let Some(o) = v.as_object_mut() {
                o.remove("topic_clusters");
                for k in ["version", "source", "updated_at"] {
                    o.remove(k);
                }
            }
            serde_json::from_value(v).context("the brief was not the expected shape")?
        };

        // Map page numbers back to URLs: each page in one cluster, empty clusters dropped.
        let mut seen = std::collections::HashSet::new();
        brief.topic_clusters = clusters
            .into_iter()
            .filter_map(|c| {
                let urls: Vec<String> = c
                    .pages
                    .iter()
                    .filter_map(|n| n.as_f64().or_else(|| n.as_str().and_then(|s| s.trim().parse().ok())))
                    .map(|n| n.round() as i64 - 1)
                    .filter(|&i| i >= 0 && (i as usize) < pages.len() && seen.insert(i))
                    .map(|i| pages[i as usize].url.clone())
                    .collect();
                (!urls.is_empty()).then_some(TopicCluster { label: c.label, description: c.description, page_count: urls.len(), urls })
            })
            .take(8)
            .collect();
        brief.content_gaps.truncate(15);
        brief.normalize();

        let previous = Brief::load(&self.store, slug).ok().flatten();
        brief.version = previous.map(|b| b.version).unwrap_or(0) + 1;
        brief.source = BriefSource::Crawl;
        brief.updated_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        brief.save(&self.store, slug)?;
        profile::write_markdown(&self.store, slug, &project.name, &project.url)?;
        let prefix = if automated { "[auto] " } else { "" };
        self.store.commit(&format!("{prefix}profile({slug}): business brief v{}", brief.version)).await?;
        self.bus.emit(
            Some(slug),
            EventKind::ProfileUpdated {
                section: "brief".into(),
                by: "seemo".into(),
                title: "Business brief".into(),
                items: brief_items(&brief),
            },
        );
        Ok(brief)
    }

    /// Every page that was read, with its title and a few short facts, sorted
    /// by URL. Titles come from the page cache when it is still there.
    fn page_list(&self, slug: &str) -> Result<Vec<PageInfo>> {
        let home = self.store.home();
        let mut pages: std::collections::BTreeMap<String, PageInfo> = Default::default();
        let cache = home.cache_dir(slug).join("pages");
        if cache.is_dir() {
            for entry in std::fs::read_dir(&cache)? {
                let path = entry?.path();
                if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                    continue;
                }
                let Ok(c) = self.store.read_yaml::<CachedPage>(&path) else { continue };
                if c.obstacle.is_some() || !(200..300).contains(&c.status) || crate::crawl::left_site(&c) {
                    continue;
                }
                pages.insert(c.url.clone(), PageInfo { url: c.url, title: c.title, facts: vec![] });
            }
        }
        let facts_dir = home.facts_dir(slug);
        if facts_dir.is_dir() {
            for entry in std::fs::read_dir(&facts_dir)? {
                let path = entry?.path();
                if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                    continue;
                }
                let key = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if crate::crawl::cached_left_site(&self.store, slug, key) {
                    continue;
                }
                let Ok(facts) = self.store.read_yaml::<Vec<Fact>>(&path) else { continue };
                let Some(url) = facts.iter().map(|f| f.url.clone()).find(|u| !u.is_empty()) else { continue };
                let mut best: Vec<&Fact> = facts.iter().collect();
                best.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
                let summary: Vec<String> = best.iter().take(3).map(|f| profile::truncate_chars(f.value.trim(), 120)).collect();
                pages.entry(url.clone()).or_insert_with(|| PageInfo { url, title: String::new(), facts: vec![] }).facts = summary;
            }
        }
        let mut out: Vec<PageInfo> = pages.into_values().collect();
        out.truncate(400);
        Ok(out)
    }
}

struct PageInfo {
    url: String,
    title: String,
    facts: Vec<String>,
}

/// Display strings for the brief in `ProfileUpdated`: the offering names.
pub fn brief_items(brief: &Brief) -> Vec<String> {
    brief.offerings.iter().filter(|o| !o.name.trim().is_empty()).take(6).map(|o| profile::truncate_chars(o.name.trim(), 90)).collect()
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
