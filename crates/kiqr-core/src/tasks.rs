//! Scheduled tasks: an instruction Seemo carries out on its own, once or on a
//! schedule, with the tools the user confirmed. `projects/<slug>/tasks/<id>/task.yaml`
//! holds the task; each run writes `runs/<ts>.yaml`. Both are committed.

use crate::chat::Step;
use crate::tools::Group;
use crate::Store;
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    /// What to do on each run, written for Seemo itself.
    pub instruction: String,
    pub schedule: Schedule,
    /// Tool groups a run may use.
    #[serde(default)]
    pub allowed: Vec<Group>,
    #[serde(default)]
    pub status: TaskStatus,
    /// `chat:<id>` or `user`.
    #[serde(default)]
    pub created_by: String,
    /// Tell this chat when a run finishes (one-off tasks made in a chat).
    #[serde(default)]
    pub follow_up_chat: Option<String>,
    /// The campaign this task works for, by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub next_run: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_run: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_status: Option<RunStatus>,
    #[serde(default)]
    pub runs: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Active,
    Paused,
    /// A one-off task that has run.
    Done,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Schedule {
    pub kind: ScheduleKind,
    /// `once`: when. `None` means right away.
    pub at: Option<DateTime<Utc>>,
    /// `rrule`: the RRULE value, e.g. `FREQ=WEEKLY;BYDAY=MO;BYHOUR=9;BYMINUTE=0`.
    pub rrule: Option<String>,
    /// IANA timezone the rule is read in. Empty means the configured default.
    pub tz: String,
    /// The schedule in the user's words.
    pub text: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleKind {
    #[default]
    Once,
    Rrule,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    /// A UTC timestamp, `20261024T090000Z`; unique per task.
    pub id: String,
    pub task: String,
    pub started: DateTime<Utc>,
    #[serde(default)]
    pub finished: Option<DateTime<Utc>>,
    pub status: RunStatus,
    /// `schedule`, `now` (run on request), `approval` (a one-off task, just confirmed) or `startup` (caught up after a restart).
    #[serde(default)]
    pub trigger: String,
    /// Seemo's closing words, or why it failed.
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub steps: Vec<Step>,
    /// `type/id` of every asset the run made or changed.
    #[serde(default)]
    pub assets: Vec<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub cost_usd: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Ok,
    Failed,
    /// The daemon was down when it was due, and too long ago to catch up.
    Missed,
}

impl RunStatus {
    pub fn id(self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Ok => "ok",
            RunStatus::Failed => "failed",
            RunStatus::Missed => "missed",
        }
    }
}

impl TaskStatus {
    pub fn id(self) -> &'static str {
        match self {
            TaskStatus::Active => "active",
            TaskStatus::Paused => "paused",
            TaskStatus::Done => "done",
        }
    }
}

/// How late a due run may be, after a restart, and still run.
pub const CATCH_UP: chrono::Duration = chrono::Duration::hours(1);

/// The machine's timezone, or `fallback` when set and valid, else UTC.
pub fn local_tz(configured: &str) -> chrono_tz::Tz {
    if let Ok(tz) = chrono_tz::Tz::from_str(configured.trim()) {
        return tz;
    }
    iana_time_zone::get_timezone().ok().and_then(|s| chrono_tz::Tz::from_str(&s).ok()).unwrap_or(chrono_tz::UTC)
}

impl Schedule {
    pub fn tz(&self, default: chrono_tz::Tz) -> Result<chrono_tz::Tz> {
        if self.tz.trim().is_empty() {
            return Ok(default);
        }
        chrono_tz::Tz::from_str(self.tz.trim()).map_err(|_| anyhow!("unknown timezone '{}'", self.tz))
    }

    /// Check the rule parses and yields something.
    pub fn validate(&self, default_tz: chrono_tz::Tz) -> Result<()> {
        match self.kind {
            ScheduleKind::Once => Ok(()),
            ScheduleKind::Rrule => {
                let rule = self.rrule.as_deref().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| anyhow!("a repeating schedule needs an rrule"))?;
                if rule.to_ascii_uppercase().contains("DTSTART") {
                    bail!("give the RRULE value only, without DTSTART");
                }
                let tz = self.tz(default_tz)?;
                let set = self.rule_set(Utc::now(), tz)?;
                if set.after(Utc::now().with_timezone(&rrule::Tz::Tz(tz))).all(1).dates.is_empty() {
                    bail!("the schedule has no future occurrence");
                }
                Ok(())
            }
        }
    }

    fn rule_set(&self, anchor: DateTime<Utc>, tz: chrono_tz::Tz) -> Result<rrule::RRuleSet> {
        let rule = self.rrule.as_deref().unwrap_or("").trim().trim_start_matches("RRULE:");
        let start = anchor.with_timezone(&tz).format("%Y%m%dT%H%M%S");
        let text = format!("DTSTART;TZID={}:{start}\nRRULE:{rule}", tz.name());
        rrule::RRuleSet::from_str(&text).with_context(|| format!("invalid schedule '{rule}'"))
    }

    /// The next `n` occurrences strictly after `after`. `anchor` is the task's
    /// creation time, which the rule counts from.
    pub fn upcoming(&self, after: DateTime<Utc>, anchor: DateTime<Utc>, n: usize, default_tz: chrono_tz::Tz) -> Result<Vec<DateTime<Utc>>> {
        match self.kind {
            ScheduleKind::Once => {
                let at = self.at.unwrap_or(anchor);
                Ok(if at > after { vec![at] } else { vec![] })
            }
            ScheduleKind::Rrule => {
                let tz = self.tz(default_tz)?;
                let set = self.rule_set(anchor, tz)?;
                let from = (after + chrono::Duration::seconds(1)).with_timezone(&rrule::Tz::Tz(tz));
                Ok(set.after(from).all(n.min(u16::MAX as usize) as u16).dates.into_iter().map(|d| d.with_timezone(&Utc)).collect())
            }
        }
    }

    /// The schedule in words: the user's own when given, else a plain reading
    /// of the rule.
    pub fn describe(&self, default_tz: chrono_tz::Tz) -> String {
        if !self.text.trim().is_empty() {
            return self.text.trim().to_string();
        }
        let tz = self.tz(default_tz).unwrap_or(default_tz);
        match self.kind {
            ScheduleKind::Once => match self.at {
                Some(at) => format!("once, {}", at.with_timezone(&tz).format("%a %-d %b %H:%M")),
                None => "once, right away".into(),
            },
            ScheduleKind::Rrule => describe_rrule(self.rrule.as_deref().unwrap_or("")),
        }
    }
}

/// "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR=10;BYMINUTE=0" → "every weekday at 10:00".
pub fn describe_rrule(rule: &str) -> String {
    let mut freq = "";
    let mut interval = 1u32;
    let mut days: Vec<&str> = vec![];
    let mut hour: Option<u32> = None;
    let mut minute = 0u32;
    let mut until = None;
    let mut count = None;
    for part in rule.trim().trim_start_matches("RRULE:").split(';') {
        let Some((k, v)) = part.split_once('=') else { continue };
        match k.to_ascii_uppercase().as_str() {
            "FREQ" => freq = match v.to_ascii_uppercase().as_str() {
                "DAILY" => "day",
                "WEEKLY" => "week",
                "MONTHLY" => "month",
                "YEARLY" => "year",
                "HOURLY" => "hour",
                _ => "",
            },
            "INTERVAL" => interval = v.parse().unwrap_or(1),
            "BYDAY" => days = v.split(',').collect(),
            "BYHOUR" => hour = v.split(',').next().and_then(|h| h.parse().ok()),
            "BYMINUTE" => minute = v.split(',').next().and_then(|m| m.parse().ok()).unwrap_or(0),
            "UNTIL" => until = Some(v.to_string()),
            "COUNT" => count = v.parse::<u32>().ok(),
            _ => {}
        }
    }
    if freq.is_empty() {
        return rule.to_string();
    }
    fn day_name(d: &str) -> String {
        match d.trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '+') {
            "MO" => "Monday",
            "TU" => "Tuesday",
            "WE" => "Wednesday",
            "TH" => "Thursday",
            "FR" => "Friday",
            "SA" => "Saturday",
            "SU" => "Sunday",
            other => other,
        }
        .to_string()
    }
    let weekdays = ["MO", "TU", "WE", "TH", "FR"];
    let mut s = if !days.is_empty() && freq == "week" && interval == 1 {
        if days.len() == 5 && weekdays.iter().all(|d| days.contains(d)) {
            "every weekday".to_string()
        } else if days.len() == 7 {
            "every day".to_string()
        } else {
            format!("every {}", join_words(&days.iter().map(|d| day_name(d)).collect::<Vec<_>>()))
        }
    } else if interval == 1 {
        format!("every {freq}")
    } else {
        format!("every {interval} {freq}s")
    };
    if let Some(h) = hour {
        s.push_str(&format!(" at {h:02}:{minute:02}"));
    }
    if let Some(u) = until {
        if u.len() >= 8 {
            s.push_str(&format!(" until {}-{}-{}", &u[..4], &u[4..6], &u[6..8]));
        }
    }
    if let Some(c) = count {
        s.push_str(&format!(", {c} times"));
    }
    s
}

fn join_words(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        n => format!("{} and {}", items[..n - 1].join(", "), items[n - 1]),
    }
}

pub fn run_id(at: DateTime<Utc>) -> String {
    at.format("%Y%m%dT%H%M%SZ").to_string()
}

impl Task {
    pub fn load(store: &Store, slug: &str, id: &str) -> Result<Task> {
        if !valid_id(id) {
            return Err(anyhow!("no task '{id}'"));
        }
        store.read_yaml_opt(&store.home().task_dir(slug, id).join("task.yaml"))?.ok_or_else(|| anyhow!("no task '{id}'"))
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        store.write_yaml(&store.home().task_dir(slug, &self.id).join("task.yaml"), self)
    }

    /// Newest first.
    pub fn list(store: &Store, slug: &str) -> Result<Vec<Task>> {
        let dir = store.home().tasks_dir(slug);
        let mut out = Vec::new();
        if dir.is_dir() {
            for e in std::fs::read_dir(dir)? {
                let p = e?.path().join("task.yaml");
                if p.exists() {
                    match store.read_yaml::<Task>(&p) {
                        Ok(t) => out.push(t),
                        Err(e) => tracing::warn!(path = %p.display(), error = %e, "skipping unreadable task"),
                    }
                }
            }
        }
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }

    pub fn delete(store: &Store, slug: &str, id: &str) -> Result<()> {
        let _ = Task::load(store, slug, id)?;
        std::fs::remove_dir_all(store.home().task_dir(slug, id))?;
        Ok(())
    }

    /// Recompute `next_run` from the schedule, after `after`.
    pub fn plan_next(&mut self, after: DateTime<Utc>, default_tz: chrono_tz::Tz) -> Result<()> {
        self.next_run = match self.status {
            TaskStatus::Active => self.schedule.upcoming(after, self.created_at, 1, default_tz)?.into_iter().next(),
            _ => None,
        };
        Ok(())
    }

    pub fn runs(&self, store: &Store, slug: &str) -> Result<Vec<Run>> {
        Run::list(store, slug, &self.id)
    }
}

impl Run {
    pub fn load(store: &Store, slug: &str, task: &str, id: &str) -> Result<Run> {
        if !valid_id(id) || !valid_id(task) {
            return Err(anyhow!("no run '{id}'"));
        }
        store.read_yaml_opt(&store.home().runs_dir(slug, task).join(format!("{id}.yaml")))?.ok_or_else(|| anyhow!("no run '{id}'"))
    }

    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        store.write_yaml(&store.home().runs_dir(slug, &self.task).join(format!("{}.yaml", self.id)), self)
    }

    /// Newest first.
    pub fn list(store: &Store, slug: &str, task: &str) -> Result<Vec<Run>> {
        let dir = store.home().runs_dir(slug, task);
        let mut out = Vec::new();
        if dir.is_dir() {
            for e in std::fs::read_dir(dir)? {
                let p = e?.path();
                if p.extension().is_some_and(|x| x == "yaml") {
                    if let Ok(r) = store.read_yaml::<Run>(&p) {
                        out.push(r);
                    }
                }
            }
        }
        out.sort_by(|a, b| b.started.cmp(&a.started));
        Ok(out)
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sched(rule: &str, tz: &str) -> Schedule {
        Schedule { kind: ScheduleKind::Rrule, at: None, rrule: Some(rule.into()), tz: tz.into(), text: String::new() }
    }

    #[test]
    fn weekday_mornings_in_stockholm() {
        let s = sched("FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR=9;BYMINUTE=0", "Europe/Stockholm");
        s.validate(chrono_tz::UTC).unwrap();
        // Created Saturday 24 Oct 2026 12:00 UTC: next runs are Mon, Tue, Wed 09:00 local (07:00 UTC in CEST... CET from 25 Oct: 08:00 UTC).
        let anchor = Utc.with_ymd_and_hms(2026, 10, 24, 12, 0, 0).unwrap();
        let next = s.upcoming(anchor, anchor, 3, chrono_tz::UTC).unwrap();
        assert_eq!(next.len(), 3);
        assert_eq!(next[0], Utc.with_ymd_and_hms(2026, 10, 26, 8, 0, 0).unwrap());
        assert_eq!(next[1], Utc.with_ymd_and_hms(2026, 10, 27, 8, 0, 0).unwrap());
        assert_eq!(next[2], Utc.with_ymd_and_hms(2026, 10, 28, 8, 0, 0).unwrap());
        assert_eq!(s.describe(chrono_tz::UTC), "every weekday at 09:00");
    }

    #[test]
    fn next_is_strictly_after() {
        let s = sched("FREQ=DAILY;BYHOUR=10;BYMINUTE=0", "UTC");
        let anchor = Utc.with_ymd_and_hms(2026, 12, 1, 9, 0, 0).unwrap();
        let first = s.upcoming(anchor, anchor, 1, chrono_tz::UTC).unwrap()[0];
        assert_eq!(first, Utc.with_ymd_and_hms(2026, 12, 1, 10, 0, 0).unwrap());
        let second = s.upcoming(first, anchor, 1, chrono_tz::UTC).unwrap()[0];
        assert_eq!(second, Utc.with_ymd_and_hms(2026, 12, 2, 10, 0, 0).unwrap());
    }

    #[test]
    fn december_until_the_24th() {
        let s = sched("FREQ=DAILY;BYHOUR=8;BYMINUTE=30;UNTIL=20261224T235959Z", "Europe/Stockholm");
        let anchor = Utc.with_ymd_and_hms(2026, 11, 30, 12, 0, 0).unwrap();
        let all = s.upcoming(anchor, anchor, 50, chrono_tz::UTC).unwrap();
        assert_eq!(all.len(), 24);
        assert_eq!(s.describe(chrono_tz::UTC), "every day at 08:30 until 2026-12-24");
    }

    #[test]
    fn once_and_bad_rules() {
        let now = Utc::now();
        let s = Schedule { kind: ScheduleKind::Once, at: None, rrule: None, tz: String::new(), text: String::new() };
        assert_eq!(s.upcoming(now - chrono::Duration::seconds(1), now, 3, chrono_tz::UTC).unwrap(), vec![now]);
        assert!(s.upcoming(now, now, 3, chrono_tz::UTC).unwrap().is_empty());
        assert_eq!(s.describe(chrono_tz::UTC), "once, right away");
        assert!(sched("FREQ=SOMETIMES", "UTC").validate(chrono_tz::UTC).is_err());
        assert!(sched("DTSTART:20260101T000000Z\nRRULE:FREQ=DAILY", "UTC").validate(chrono_tz::UTC).is_err());
        assert!(sched("FREQ=DAILY", "Mars/Olympus").validate(chrono_tz::UTC).is_err());
        assert_eq!(describe_rrule("FREQ=WEEKLY;INTERVAL=2;BYDAY=MO;BYHOUR=9"), "every 2 weeks at 09:00");
        assert_eq!(describe_rrule("FREQ=WEEKLY;BYDAY=MO,WE;BYHOUR=9"), "every Monday and Wednesday at 09:00");
    }
}
