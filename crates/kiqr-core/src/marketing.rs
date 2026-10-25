//! The marketing plan Seemo owns, and the campaigns under it. A plan is a
//! few objectives for a period, each tied to one KPI with a target; a
//! campaign is an initiative towards an objective that groups the assets and
//! tasks made for it. Seemo proposes both on cards; the user approves or
//! edits. `projects/<slug>/marketing/plan.yaml` and
//! `projects/<slug>/campaigns/<id>/campaign.yaml`, committed.

use crate::assets::slugify;
use crate::Store;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// The KPIs a plan may name. A fixed vocabulary so metrics sources, reports
/// and the plan agree on words; `label` is what the user sees.
pub const KPIS: &[(&str, &str)] = &[
    ("organic_traffic", "Organic traffic"),
    ("qualified_leads", "Qualified leads"),
    ("signups", "Sign-ups"),
    ("conversion_rate", "Conversion rate"),
    ("retention", "Retention"),
    ("brand_mentions", "Brand mentions"),
    ("rank_positions", "Rank positions"),
    ("revenue", "Revenue"),
    ("cac", "Customer acquisition cost"),
    ("published", "Pieces published"),
];

/// A readable id from a title: the slug cut at a word boundary, six words or so.
pub fn short_id(title: &str) -> String {
    let full = slugify(title);
    if full.len() <= 40 {
        return full;
    }
    match full[..40].rfind('-') {
        Some(i) if i >= 12 => full[..i].to_string(),
        _ => full[..40].to_string(),
    }
}

pub fn kpi_label(id: &str) -> Option<&'static str> {
    KPIS.iter().find(|(k, _)| *k == id).map(|(_, l)| *l)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Plan {
    /// Counts up on every save; the card shows what version it replaces.
    pub version: u32,
    /// The period in the user's words: "Q4 2026", "November".
    pub period: String,
    /// Two or three sentences: where the business stands and what this period is for.
    pub summary: String,
    pub objectives: Vec<Objective>,
    /// Channel kinds in play this period: email, reddit, linkedin, blog…
    pub channels: Vec<String>,
    /// `user`, `chat:<id>` or `task:<id>`.
    pub updated_by: String,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Objective {
    /// Slug of the title, stable across edits.
    pub id: String,
    pub title: String,
    /// Why this, from the brief: one or two sentences.
    pub why: String,
    /// One of [`KPIS`].
    pub kpi: String,
    /// The number to reach, in `unit`; none when the KPI has no source yet.
    pub target: Option<f64>,
    /// "sessions/month", "%", "per week"…
    pub unit: String,
    pub by: Option<NaiveDate>,
    pub status: ObjectiveStatus,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveStatus {
    #[default]
    Active,
    Done,
    Dropped,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Campaign {
    pub id: String,
    pub title: String,
    /// What it should achieve, one or two sentences.
    pub goal: String,
    /// The objective it serves, by id.
    pub objective: Option<String>,
    pub channels: Vec<String>,
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
    pub status: CampaignStatus,
    pub created_by: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CampaignStatus {
    #[default]
    Active,
    Done,
    Dropped,
}

impl Plan {
    pub fn load(store: &Store, slug: &str) -> Result<Option<Plan>> {
        store.read_yaml_opt(&store.home().plan_file(slug))
    }

    /// Check the shape, give objectives their ids, bump the version and write.
    pub fn save(&mut self, store: &Store, slug: &str, by: &str) -> Result<()> {
        self.validate()?;
        for o in &mut self.objectives {
            if o.id.is_empty() {
                o.id = short_id(&o.title);
            }
        }
        self.version += 1;
        self.updated_by = by.to_string();
        self.updated_at = Some(Utc::now());
        store.write_yaml(&store.home().plan_file(slug), self)
    }

    pub fn validate(&self) -> Result<()> {
        if self.objectives.is_empty() {
            bail!("a plan needs at least one objective");
        }
        if self.objectives.len() > 5 {
            bail!("keep a plan to five objectives at most");
        }
        for o in &self.objectives {
            if o.title.trim().is_empty() {
                bail!("an objective needs a title");
            }
            if kpi_label(&o.kpi).is_none() {
                bail!("objective \"{}\" names the KPI '{}', which is not one of: {}", o.title, o.kpi, KPIS.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", "));
            }
        }
        Ok(())
    }

    /// One line per active objective, for the system prompt.
    pub fn prompt_lines(&self) -> Vec<String> {
        self.objectives
            .iter()
            .filter(|o| o.status == ObjectiveStatus::Active)
            .map(|o| {
                let target = match (o.target, o.unit.trim()) {
                    (Some(t), "") => format!(" → {t}"),
                    (Some(t), u) => format!(" → {t} {u}"),
                    (None, _) => String::new(),
                };
                let by = o.by.map(|d| format!(" by {}", d.format("%-d %b"))).unwrap_or_default();
                format!("{} ({}): {}{}{}", o.title, o.id, kpi_label(&o.kpi).unwrap_or(&o.kpi), target, by)
            })
            .collect()
    }
}

impl Campaign {
    pub fn load(store: &Store, slug: &str, id: &str) -> Result<Campaign> {
        if !crate::assets::valid_name(id) {
            bail!("no campaign {id}");
        }
        store.read_yaml_opt(&store.home().campaign_dir(slug, id).join("campaign.yaml"))?.with_context(|| format!("no campaign {id}"))
    }

    pub fn list(store: &Store, slug: &str) -> Result<Vec<Campaign>> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(store.home().campaigns_dir(slug)).into_iter().flatten().flatten() {
            if let Ok(Some(c)) = store.read_yaml_opt::<Campaign>(&e.path().join("campaign.yaml")) {
                out.push(c);
            }
        }
        out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(out)
    }

    /// A free id from the title: `launch-visual-editor`, `-2` when taken.
    pub fn new_id(store: &Store, slug: &str, title: &str) -> String {
        let base = slugify(title);
        let base = if base.is_empty() { "campaign".to_string() } else { base };
        let mut id = base.clone();
        let mut n = 2;
        while store.home().campaign_dir(slug, &id).exists() {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }

    pub fn save(&mut self, store: &Store, slug: &str) -> Result<()> {
        if self.title.trim().is_empty() {
            bail!("a campaign needs a title");
        }
        if !crate::assets::valid_name(&self.id) {
            bail!("a campaign id is lowercase letters, digits and dashes");
        }
        let now = Utc::now();
        if self.created_at.is_none() {
            self.created_at = Some(now);
        }
        self.updated_at = Some(now);
        store.write_yaml(&store.home().campaign_dir(slug, &self.id).join("campaign.yaml"), self)
    }

    pub fn delete(store: &Store, slug: &str, id: &str) -> Result<()> {
        let dir = store.home().campaign_dir(slug, id);
        if !dir.join("campaign.yaml").exists() {
            bail!("no campaign {id}");
        }
        std::fs::remove_dir_all(dir)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Home;

    #[test]
    fn plan_validates_and_versions() {
        let dir = std::env::temp_dir().join(format!("kiqr-plan-{}", std::process::id()));
        let store = Store::open(Home::at(&dir)).unwrap();
        let mut plan = Plan { period: "Q4 2026".into(), objectives: vec![Objective { title: "Double organic traffic".into(), kpi: "organic_traffic".into(), target: Some(4000.0), unit: "sessions/month".into(), ..Default::default() }], ..Default::default() };
        plan.save(&store, "acme", "chat:x").unwrap();
        assert_eq!(plan.version, 1);
        assert_eq!(plan.objectives[0].id, "double-organic-traffic");
        let loaded = Plan::load(&store, "acme").unwrap().unwrap();
        assert_eq!(loaded.prompt_lines()[0], "Double organic traffic (double-organic-traffic): Organic traffic → 4000 sessions/month");
        let bad = Plan { objectives: vec![Objective { title: "x".into(), kpi: "likes".into(), ..Default::default() }], ..Default::default() };
        assert!(bad.validate().is_err());
        assert_eq!(short_id("Publish the eight highest-intent articles from the content gaps"), "publish-the-eight-highest-intent");
        let mut c = Campaign { id: Campaign::new_id(&store, "acme", "Launch: Visual Editor"), title: "Launch: Visual Editor".into(), ..Default::default() };
        c.save(&store, "acme").unwrap();
        assert_eq!(c.id, "launch-visual-editor");
        assert_eq!(Campaign::new_id(&store, "acme", "Launch: Visual Editor"), "launch-visual-editor-2");
        assert_eq!(Campaign::list(&store, "acme").unwrap().len(), 1);
        Campaign::delete(&store, "acme", &c.id).unwrap();
        std::fs::remove_dir_all(dir).ok();
    }
}
