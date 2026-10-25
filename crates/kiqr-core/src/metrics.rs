//! Numbers, kept as snapshots. A source (`traffic`, `search`, `conversions`,
//! `ranks`, `mentions`) is read by a connection once a day into
//! `projects/<slug>/metrics/<source>/<YYYY-MM-DD>.yaml`: rows of
//! (date, metric, value, dimensions) for the last 30 days. The UI, the plan
//! and Seemo read the files, never the API; a KPI on the plan maps to one
//! (source, metric) here.

use crate::Store;
use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The sources a connection may read, with the user's word for each.
pub const SOURCES: &[(&str, &str)] = &[("traffic", "Traffic"), ("search", "Search"), ("conversions", "Conversions"), ("ranks", "Ranks"), ("mentions", "Mentions")];

pub fn source_label(id: &str) -> Option<&'static str> {
    SOURCES.iter().find(|(s, _)| *s == id).map(|(_, l)| *l)
}

/// Which (source, metric) a plan KPI reads, and how the number is taken over a window.
pub fn kpi_binding(kpi: &str) -> Option<(&'static str, &'static str, Agg)> {
    Some(match kpi {
        "organic_traffic" => ("traffic", "organic_sessions", Agg::Sum),
        "signups" => ("conversions", "key_events", Agg::Sum),
        "qualified_leads" => ("conversions", "key_events", Agg::Sum),
        "conversion_rate" => ("traffic", "conversion_rate", Agg::Mean),
        "brand_mentions" => ("mentions", "mentions", Agg::Sum),
        "rank_positions" => ("ranks", "average_position", Agg::Mean),
        "published" => ("kiqr", "published", Agg::Sum),
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agg {
    Sum,
    Mean,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Row {
    pub date: NaiveDate,
    pub metric: String,
    pub value: f64,
    /// Breakdown, when the row is not the day's total: `{query: "..."}`, `{page: "/pricing"}`, `{event: "sign_up"}`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dims: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub source: String,
    /// The connection id that read it.
    pub provider: String,
    pub taken_at: DateTime<Utc>,
    pub from: NaiveDate,
    pub to: NaiveDate,
    #[serde(default)]
    pub rows: Vec<Row>,
}

impl Snapshot {
    /// Write today's file for the source (a second pull the same day replaces it).
    pub fn save(&self, store: &Store, slug: &str) -> Result<()> {
        let path = store.home().metrics_dir(slug, &self.source).join(format!("{}.yaml", self.taken_at.date_naive().format("%Y-%m-%d")));
        store.write_yaml(&path, self)
    }

    /// The newest file for a source.
    pub fn latest(store: &Store, slug: &str, source: &str) -> Option<Snapshot> {
        let dir = store.home().metrics_dir(slug, source);
        let mut names: Vec<String> = std::fs::read_dir(&dir).ok()?.flatten().filter_map(|e| e.file_name().to_str().map(str::to_string)).filter(|n| n.ends_with(".yaml")).collect();
        names.sort();
        let last = names.pop()?;
        store.read_yaml_opt(&dir.join(last)).ok().flatten()
    }

    /// Day totals for one metric (rows without dimensions), oldest first.
    pub fn series(&self, metric: &str) -> Vec<(NaiveDate, f64)> {
        let mut out: Vec<(NaiveDate, f64)> = self.rows.iter().filter(|r| r.metric == metric && r.dims.is_empty()).map(|r| (r.date, r.value)).collect();
        out.sort_by_key(|(d, _)| *d);
        out
    }

    /// The metrics that have day totals.
    pub fn metrics(&self) -> Vec<String> {
        let mut v: Vec<String> = self.rows.iter().filter(|r| r.dims.is_empty()).map(|r| r.metric.clone()).collect();
        v.sort();
        v.dedup();
        v
    }

    /// Rows broken down by one dimension for a metric, summed over the window, largest first.
    pub fn top(&self, metric: &str, dim: &str, n: usize) -> Vec<(String, f64)> {
        let mut by: BTreeMap<String, f64> = BTreeMap::new();
        for r in self.rows.iter().filter(|r| r.metric == metric) {
            if let Some(k) = r.dims.get(dim) {
                *by.entry(k.clone()).or_default() += r.value;
            }
        }
        let mut v: Vec<(String, f64)> = by.into_iter().collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        v.truncate(n);
        v
    }
}

/// The last `days` of a series against the `days` before: (now, before).
pub fn window(series: &[(NaiveDate, f64)], days: i64, agg: Agg, today: NaiveDate) -> (Option<f64>, Option<f64>) {
    let start = today - chrono::Duration::days(days - 1);
    let prev_start = start - chrono::Duration::days(days);
    let take = |from: NaiveDate, to: NaiveDate| -> Option<f64> {
        let vals: Vec<f64> = series.iter().filter(|(d, _)| *d >= from && *d <= to).map(|(_, v)| *v).collect();
        if vals.is_empty() {
            return None;
        }
        Some(match agg {
            Agg::Sum => vals.iter().sum(),
            Agg::Mean => vals.iter().sum::<f64>() / vals.len() as f64,
        })
    };
    (take(start, today), take(prev_start, start - chrono::Duration::days(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_and_tops() {
        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        let mut rows = Vec::new();
        for i in 0..14 {
            rows.push(Row { date: d("2026-10-01") + chrono::Duration::days(i), metric: "sessions".into(), value: if i < 7 { 10.0 } else { 20.0 }, dims: BTreeMap::new() });
        }
        rows.push(Row { date: d("2026-10-14"), metric: "clicks".into(), value: 5.0, dims: BTreeMap::from([("query".to_string(), "scrape idealista".to_string())]) });
        rows.push(Row { date: d("2026-10-13"), metric: "clicks".into(), value: 7.0, dims: BTreeMap::from([("query".to_string(), "scrape idealista".to_string())]) });
        rows.push(Row { date: d("2026-10-13"), metric: "clicks".into(), value: 3.0, dims: BTreeMap::from([("query".to_string(), "residential proxies".to_string())]) });
        let s = Snapshot { source: "traffic".into(), provider: "t".into(), taken_at: Utc::now(), from: d("2026-10-01"), to: d("2026-10-14"), rows };
        let series = s.series("sessions");
        assert_eq!(series.len(), 14);
        assert_eq!(window(&series, 7, Agg::Sum, d("2026-10-14")), (Some(140.0), Some(70.0)));
        assert_eq!(s.metrics(), vec!["sessions".to_string()]);
        assert_eq!(s.top("clicks", "query", 5), vec![("scrape idealista".to_string(), 12.0), ("residential proxies".to_string(), 3.0)]);
        assert_eq!(kpi_binding("organic_traffic").map(|b| b.0), Some("traffic"));
    }
}
