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

/// Render `PROFILE.md`, the single document every agent reads first.
pub fn render_markdown(project_name: &str, project_url: &str, sections: &[Section]) -> String {
    let mut md = format!("# {project_name}\n\nBusiness profile built by KIQR from {project_url}. Every value lists where it was found. Locked fields were set by a person and override anything crawled.\n");
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
    store.write_text(&store.home().profile_md(slug), &render_markdown(project_name, project_url, &sections))
}
