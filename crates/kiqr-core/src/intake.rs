//! What the owner told Seemo while it read the site for the first time:
//! the goal, the best customers, the time they have, where they already are.
//! `projects/<slug>/intake.yaml`. Not part of the profile (no evidence; it is
//! the owner's word), but read with it: the brief and the plan lean on it.

use crate::store::Store;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Intake {
    /// `customers` (more new customers), `repeat` (more from existing ones), `known` (get known), `launch` (launch something), or empty.
    pub goal: String,
    /// The goal in the owner's own words, when they wrote some.
    pub goal_note: String,
    /// Who their best customers are.
    pub customers: String,
    /// `lt1`, `1to3` or `3plus` hours a week.
    pub time: String,
    /// Networks they already post on, by id (`instagram`, `linkedin`…).
    pub networks: Vec<String>,
    /// Questions they skipped, so they are not asked again.
    pub skipped: Vec<String>,
    /// RFC 3339.
    pub updated_at: String,
}

impl Intake {
    fn path(store: &Store, slug: &str) -> std::path::PathBuf {
        store.home().project_dir(slug).join("intake.yaml")
    }

    pub fn load(store: &Store, slug: &str) -> Result<Option<Intake>> {
        store.read_yaml_opt(&Self::path(store, slug))
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        store.write_yaml(&Self::path(store, slug), self)
    }

    pub fn is_empty(&self) -> bool {
        self.goal.is_empty() && self.goal_note.trim().is_empty() && self.customers.trim().is_empty() && self.time.is_empty() && self.networks.is_empty()
    }

    /// One line per answer, in plain words, for a prompt.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        let goal = match self.goal.as_str() {
            "customers" => "win more new customers",
            "repeat" => "sell more to existing customers",
            "known" => "get known",
            "launch" => "launch something new",
            _ => "",
        };
        match (goal, self.goal_note.trim()) {
            ("", "") => {}
            ("", note) => out.push(format!("Main goal right now: {note}")),
            (g, "") => out.push(format!("Main goal right now: {g}")),
            (g, note) => out.push(format!("Main goal right now: {g} ({note})")),
        }
        if !self.customers.trim().is_empty() {
            out.push(format!("Best customers, in the owner's words: {}", self.customers.trim()));
        }
        let time = match self.time.as_str() {
            "lt1" => "less than an hour",
            "1to3" => "one to three hours",
            "3plus" => "more than three hours",
            _ => "",
        };
        if !time.is_empty() {
            out.push(format!("Time for marketing each week: {time}"));
        }
        if !self.networks.is_empty() {
            out.push(format!("Already posts on: {}", self.networks.join(", ")));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_say_only_what_was_answered() {
        assert!(Intake::default().lines().is_empty());
        let i = Intake { goal: "customers".into(), time: "lt1".into(), networks: vec!["instagram".into()], ..Default::default() };
        assert_eq!(i.lines(), vec!["Main goal right now: win more new customers", "Time for marketing each week: less than an hour", "Already posts on: instagram"]);
        let i = Intake { goal_note: "fill the spring courses".into(), ..Default::default() };
        assert_eq!(i.lines(), vec!["Main goal right now: fill the spring courses"]);
    }
}
