//! The business profile: sections of fields, each field with evidence,
//! confidence and a lock. Stored as `profile/<section>.yaml` (the editable
//! source) and rendered to `PROFILE.md` (what agents read).

use crate::Store;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Every section the profile has, in display order, with the description the
/// UI shows under the title and the extractor uses to route facts.
pub const SECTIONS: &[(&str, &str, &str)] = &[
    ("company", "Company", "Name, tagline, founding, locations, team size, contact and legal entity."),
    ("products", "Products & services", "Each product or service with its features and who it is for."),
    ("pricing", "Pricing", "Plans, tiers, what each includes, free tier, trial, billing terms."),
    ("audience", "Audience", "Ideal customers, personas, industries, company sizes, jobs to be done."),
    ("positioning", "Positioning", "Value propositions, differentiators, proof points, objections the site answers."),
    ("voice", "Brand voice", "Tone, vocabulary, sentences quoted verbatim that show how the brand writes."),
    ("competitors", "Competitors", "Competitors named on the site or implied by comparisons."),
    ("seo", "SEO baseline", "Keywords the site targets, title patterns, topics covered and missing."),
    ("channels", "Channels", "Social accounts, newsletter, community, support channels found on the site."),
    ("content", "Content inventory", "Blog posts, case studies, docs and guides with dates and topics."),
    ("integrations", "Tech & integrations", "What the product integrates with and the stack it reveals."),
    ("trust", "Trust", "Testimonials, customer logos, reviews, certifications, security and compliance claims."),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub url: String,
    pub quote: String,
}

/// A fact extracted from one page, before synthesis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fact {
    pub section: String,
    pub field: String,
    pub value: String,
    pub quote: String,
    pub confidence: f32,
    #[serde(default)]
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub edited_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl Section {
    pub fn empty(id: &str) -> Section {
        let (_, title, desc) = SECTIONS.iter().find(|(s, _, _)| *s == id).copied().unwrap_or((id, id, ""));
        Section { id: id.to_string(), title: title.to_string(), description: desc.to_string(), fields: vec![], updated_at: None }
    }

    pub fn load(store: &Store, slug: &str, id: &str) -> Result<Section> {
        let path = store.home().profile_dir(slug).join(format!("{id}.yaml"));
        Ok(store.read_yaml_opt(&path)?.unwrap_or_else(|| Section::empty(id)))
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        store.write_yaml(&store.home().profile_dir(slug).join(format!("{}.yaml", self.id)), self)
    }
}

pub fn load_all(store: &Store, slug: &str) -> Result<Vec<Section>> {
    SECTIONS.iter().map(|(id, _, _)| Section::load(store, slug, id)).collect()
}

pub fn load_facts(store: &Store, slug: &str) -> Result<Vec<Fact>> {
    let dir = store.home().facts_dir(slug);
    let mut out = Vec::new();
    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                let facts: Vec<Fact> = store.read_yaml(&path)?;
                out.extend(facts);
            }
        }
    }
    Ok(out)
}

/// Render `PROFILE.md`, the single document every agent reads first. The
/// business brief, when there is one, comes first.
pub fn render_markdown(project_name: &str, project_url: &str, brief: Option<&Brief>, brand: Option<&crate::brand::Brand>, sections: &[Section]) -> String {
    let mut md = format!("# {project_name}\n\nBusiness profile built by KIQR from {project_url}. Every value lists where it was found. Locked fields were set by a person and override anything crawled.\n");
    if let Some(b) = brief {
        md.push_str(&render_brief(b));
    }
    if let Some(b) = brand {
        md.push_str(&crate::brand::render_markdown(b));
    }
    for s in sections {
        if s.fields.is_empty() {
            continue;
        }
        md.push_str(&format!("\n## {}\n\n", s.title));
        for f in &s.fields {
            md.push_str(&format!("### {}{}\n\n{}\n\n", f.name, if f.locked { " (locked)" } else { "" }, f.value.trim()));
            for e in &f.evidence {
                md.push_str(&format!("- {} — \"{}\"\n", e.url, e.quote.trim()));
            }
            if !f.evidence.is_empty() {
                md.push('\n');
            }
        }
    }
    md
}

pub fn write_markdown(store: &Store, slug: &str, project_name: &str, project_url: &str) -> Result<()> {
    let sections = load_all(store, slug)?;
    let brief = Brief::load(store, slug)?;
    let brand = crate::brand::Brand::load(store, slug)?;
    store.write_text(&store.home().profile_md(slug), &render_markdown(project_name, project_url, brief.as_ref(), brand.as_ref(), &sections))
}

// ---------- business brief ----------

/// Where the current brief came from: the model after a crawl, or a person.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BriefSource {
    #[default]
    Crawl,
    Edited,
}

/// The business brief: a one-document summary of the business built from the
/// merged sections and the page list, the way a content strategist would
/// write it. Stored at `profile/brief.yaml`. Every field defaults so partial
/// model output and older files load.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Brief {
    pub version: u32,
    pub source: BriefSource,
    /// RFC 3339.
    pub updated_at: String,
    pub business_name: String,
    pub summary: String,
    pub language: String,
    pub geographies: Vec<String>,
    pub offerings: Vec<Offering>,
    pub audiences: Vec<Audience>,
    pub tone_of_voice: ToneOfVoice,
    pub brand_terms: Vec<String>,
    pub differentiators: Vec<String>,
    pub competitors: Vec<Competitor>,
    pub pricing_summary: String,
    pub topic_clusters: Vec<TopicCluster>,
    pub content_gaps: Vec<ContentGap>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Offering {
    pub name: String,
    /// "product" or "service".
    pub kind: String,
    pub description: String,
    pub url: String,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Audience {
    pub name: String,
    pub needs: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ToneOfVoice {
    pub description: String,
    pub traits: Vec<String>,
    pub examples: Vec<String>,
    pub avoid: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Competitor {
    pub name: String,
    /// How the site compares itself to this competitor.
    pub note: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TopicCluster {
    pub label: String,
    pub description: String,
    pub page_count: usize,
    pub urls: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ContentGap {
    pub topic: String,
    pub rationale: String,
    /// "informational", "commercial", "transactional" or "navigational".
    pub intent: String,
}

impl Brief {
    pub fn load(store: &Store, slug: &str) -> Result<Option<Brief>> {
        store.read_yaml_opt(&store.home().brief_file(slug))
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        store.write_yaml(&store.home().brief_file(slug), self)
    }

    /// Clamp free-text enums to the values the schema allows.
    pub fn normalize(&mut self) {
        for o in &mut self.offerings {
            o.kind = if o.kind.to_ascii_lowercase().contains("product") { "product".into() } else { "service".into() };
            o.sources.retain(|s| !s.trim().is_empty());
            o.sources.dedup();
        }
        for g in &mut self.content_gaps {
            let i = g.intent.to_ascii_lowercase();
            g.intent = ["informational", "commercial", "transactional", "navigational"]
                .into_iter()
                .find(|k| i.contains(k))
                .unwrap_or("informational")
                .to_string();
        }
        for c in &mut self.topic_clusters {
            c.page_count = c.urls.len();
        }
        self.language = self.language.trim().to_ascii_lowercase();
    }
}

fn render_brief(b: &Brief) -> String {
    let mut md = String::from("\n## Business brief\n\n");
    if !b.business_name.trim().is_empty() {
        md.push_str(&format!("**{}**", b.business_name.trim()));
        let mut meta = Vec::new();
        if !b.language.is_empty() {
            meta.push(format!("language: {}", b.language));
        }
        if !b.geographies.is_empty() {
            meta.push(format!("serves: {}", b.geographies.join(", ")));
        }
        if !meta.is_empty() {
            md.push_str(&format!(" ({})", meta.join("; ")));
        }
        md.push_str("\n\n");
    }
    if !b.summary.trim().is_empty() {
        md.push_str(&format!("{}\n\n", b.summary.trim()));
    }
    let list = |md: &mut String, title: &str, items: Vec<String>| {
        if items.is_empty() {
            return;
        }
        md.push_str(&format!("### {title}\n\n"));
        for i in items {
            md.push_str(&format!("- {i}\n"));
        }
        md.push('\n');
    };
    list(
        &mut md,
        "Offerings",
        b.offerings
            .iter()
            .map(|o| {
                let mut s = format!("{} ({}): {}", o.name, o.kind, o.description.trim());
                if !o.url.is_empty() {
                    s.push_str(&format!(" {}", o.url));
                }
                s
            })
            .collect(),
    );
    list(
        &mut md,
        "Audiences",
        b.audiences
            .iter()
            .map(|a| if a.needs.is_empty() { a.name.clone() } else { format!("{}: {}", a.name, a.needs.join("; ")) })
            .collect(),
    );
    let t = &b.tone_of_voice;
    if !t.description.is_empty() || !t.traits.is_empty() || !t.examples.is_empty() || !t.avoid.is_empty() {
        md.push_str("### Tone of voice\n\n");
        if !t.description.trim().is_empty() {
            md.push_str(&format!("{}\n\n", t.description.trim()));
        }
        if !t.traits.is_empty() {
            md.push_str(&format!("- Traits: {}\n", t.traits.join(", ")));
        }
        for e in &t.examples {
            md.push_str(&format!("- Example: \"{}\"\n", e.trim()));
        }
        if !t.avoid.is_empty() {
            md.push_str(&format!("- Avoid: {}\n", t.avoid.join("; ")));
        }
        md.push('\n');
    }
    list(&mut md, "Differentiators", b.differentiators.clone());
    if !b.pricing_summary.trim().is_empty() {
        md.push_str(&format!("### Pricing\n\n{}\n\n", b.pricing_summary.trim()));
    }
    list(
        &mut md,
        "Competitors",
        b.competitors.iter().map(|c| if c.note.is_empty() { c.name.clone() } else { format!("{}: {}", c.name, c.note) }).collect(),
    );
    if !b.brand_terms.is_empty() {
        md.push_str(&format!("### Brand terms\n\nSpell exactly: {}\n\n", b.brand_terms.join(", ")));
    }
    list(
        &mut md,
        "Topic clusters",
        b.topic_clusters.iter().map(|c| format!("{} ({} page{}): {}", c.label, c.page_count, if c.page_count == 1 { "" } else { "s" }, c.description)).collect(),
    );
    list(
        &mut md,
        "Content gaps",
        b.content_gaps.iter().map(|g| format!("{} [{}]: {}", g.topic, g.intent, g.rationale)).collect(),
    );
    md
}

/// Up to six short strings that show what a section now holds, for the live
/// profile view: "Plans: Five tiers…".
pub fn section_items(section: &Section) -> Vec<String> {
    section.fields.iter().take(6).map(|f| truncate_chars(&format!("{}: {}", humanize(&f.name), squash(&f.value)), 90)).collect()
}

/// `free_tier` → `Free tier`.
pub fn humanize(name: &str) -> String {
    let s = name.replace(['_', '-'], " ");
    let s = s.trim();
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brief_loads_partial_and_normalizes() {
        let mut b: Brief = serde_json::from_str(r#"{"business_name":"Acme","offerings":[{"name":"Widget","kind":"Product line"}],"content_gaps":[{"topic":"x","intent":"Commercial investigation"}],"source":"edited"}"#).unwrap();
        b.normalize();
        assert_eq!(b.source, BriefSource::Edited);
        assert_eq!(b.offerings[0].kind, "product");
        assert_eq!(b.content_gaps[0].intent, "commercial");
        assert_eq!(b.version, 0);
        let v = serde_json::to_value(&b).unwrap();
        assert_eq!(v["tone_of_voice"]["traits"], serde_json::json!([]));
    }

    #[test]
    fn section_items_are_short() {
        let mut s = Section::empty("pricing");
        s.fields.push(Field { name: "free_tier".into(), value: "x ".repeat(100), evidence: vec![], confidence: 1.0, locked: false, edited_at: None });
        let items = section_items(&s);
        assert!(items[0].starts_with("Free tier: x x"));
        assert!(items[0].chars().count() <= 90);
    }
}
