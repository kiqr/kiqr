//! How a manifest reads a metrics source: requests, and where in each answer
//! the rows are. KIQR knows the shape of a snapshot (date, metric, value,
//! dimensions) and nothing about any analytics service; the manifest says
//! which request to make and which JSON pointer is the day, a metric or a
//! breakdown, plus a few derived metrics (a filtered sum, a ratio).
//!
//! ```yaml
//! sources:
//!   traffic:
//!     requests:
//!       - request: { method: POST, url: "https://…", json: { from: "{{from}}", to: "{{to}}" } }
//!         rows: /rows                    # the array of rows in the answer
//!         date: /dimensionValues/0/value # a pointer, or a template ("{{to}}") for a list over the window
//!         dims: { channel: /dimensionValues/1/value }
//!         metrics: { sessions: /metricValues/0/value }
//!         totals: [sessions]             # summed per day as the day's total (default: every metric)
//!         breakdown: [sessions]          # kept per dimension (default: every metric, when there are dims)
//!         skip_zero: true                # leave out breakdown rows that are 0
//!         scale: { ctr: 100 }
//!     derive:
//!       - { metric: organic_sessions, sum: sessions, where: { channel: Organic Search } }
//!       - { metric: conversion_rate, ratio: [key_events, sessions], scale: 100 }
//! ```
//!
//! The arguments a request may use are `{{from}}`, `{{to}}` (YYYY-MM-DD) and `{{days}}`.

use crate::extensions::{fill, pointer, HttpRequest};
use crate::metrics::Row;
use anyhow::{bail, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceSpec {
    pub requests: Vec<SourceRequest>,
    pub derive: Vec<Derive>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceRequest {
    pub request: HttpRequest,
    /// Pointer to the array of rows; empty: the answer is the array.
    pub rows: String,
    /// Pointer to the day in a row, or a template such as `{{to}}`.
    pub date: String,
    pub dims: BTreeMap<String, String>,
    pub metrics: BTreeMap<String, String>,
    pub totals: Option<Vec<String>>,
    pub breakdown: Option<Vec<String>>,
    pub skip_zero: bool,
    pub scale: BTreeMap<String, f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Derive {
    pub metric: String,
    /// Sum this metric over the rows whose dimensions match `where`, per day.
    pub sum: Option<String>,
    #[serde(rename = "where")]
    pub filter: BTreeMap<String, String>,
    /// `[a, b]`: a / b per day, from the day's totals.
    pub ratio: Option<(String, String)>,
    pub scale: Option<f64>,
}

/// `sources:` is a map of specs; a plain list of names (older manifests, and
/// the marketplace index) parses as names with nothing to read them with.
pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, SourceSpec>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Either {
        Names(Vec<String>),
        Specs(BTreeMap<String, SourceSpec>),
    }
    Ok(match Option::<Either>::deserialize(d)? {
        None => BTreeMap::new(),
        Some(Either::Names(v)) => v.into_iter().map(|n| (n, SourceSpec::default())).collect(),
        Some(Either::Specs(m)) => m,
    })
}

/// The arguments a source's requests are filled with.
pub fn args(from: NaiveDate, to: NaiveDate) -> Value {
    json!({ "from": from.format("%Y-%m-%d").to_string(), "to": to.format("%Y-%m-%d").to_string(), "days": (to - from).num_days() + 1 })
}

pub fn validate(name: &str, s: &SourceSpec) -> Result<()> {
    for (i, r) in s.requests.iter().enumerate() {
        if r.metrics.is_empty() {
            bail!("source '{name}' request {} maps no metrics", i + 1);
        }
        if r.date.trim().is_empty() {
            bail!("source '{name}' request {} has no date", i + 1);
        }
        for m in r.totals.iter().flatten().chain(r.breakdown.iter().flatten()).chain(r.scale.keys()) {
            if !r.metrics.contains_key(m) {
                bail!("source '{name}' request {} names {m}, which its metrics do not map", i + 1);
            }
        }
    }
    for d in &s.derive {
        if d.metric.trim().is_empty() || (d.sum.is_none() == d.ratio.is_none()) {
            bail!("source '{name}': a derived metric needs a name and either sum or ratio");
        }
    }
    Ok(())
}

fn day(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    NaiveDate::parse_from_str(s, "%Y%m%d").or_else(|_| NaiveDate::parse_from_str(s, "%Y-%m-%d")).or_else(|_| NaiveDate::parse_from_str(s.get(..10).unwrap_or(""), "%Y-%m-%d")).ok()
}

fn number(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

fn text(v: &Value) -> String {
    v.as_str().map(str::to_string).unwrap_or_else(|| if v.is_null() { String::new() } else { v.to_string() })
}

/// One reading before it becomes rows: a day, its dimensions, a metric.
struct Obs {
    date: NaiveDate,
    dims: BTreeMap<String, String>,
    metric: String,
    value: f64,
}

/// Turn the answers to a source's requests (in order) into snapshot rows.
pub fn read(spec: &SourceSpec, answers: &[Value], args: &Value) -> Vec<Row> {
    let mut totals: BTreeMap<(NaiveDate, String), f64> = BTreeMap::new();
    let mut rows: Vec<Row> = Vec::new();
    let mut seen: Vec<Obs> = Vec::new();
    for (r, answer) in spec.requests.iter().zip(answers) {
        let list = pointer(answer, &r.rows).and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let fixed = if r.date.trim().starts_with('/') { None } else { day(&fill(&r.date, args, |_| None)) };
        let totals_of: Vec<&String> = match &r.totals {
            Some(t) => t.iter().collect(),
            None => r.metrics.keys().collect(),
        };
        let breakdown_of: Vec<&String> = match &r.breakdown {
            Some(b) => b.iter().collect(),
            None if !r.dims.is_empty() => r.metrics.keys().collect(),
            None => vec![],
        };
        for item in &list {
            let Some(date) = fixed.or_else(|| pointer(item, &r.date).and_then(|v| v.as_str().map(str::to_string).or_else(|| v.as_i64().map(|n| n.to_string()))).and_then(|s| day(&s))) else { continue };
            let dims: BTreeMap<String, String> = r.dims.iter().map(|(k, p)| (k.clone(), pointer(item, p).map(text).unwrap_or_default())).collect();
            for (metric, p) in &r.metrics {
                let value = pointer(item, p).and_then(number).unwrap_or(0.0) * r.scale.get(metric).copied().unwrap_or(1.0);
                if totals_of.contains(&metric) {
                    *totals.entry((date, metric.clone())).or_default() += value;
                }
                if breakdown_of.contains(&metric) && !dims.is_empty() && !(r.skip_zero && value == 0.0) {
                    rows.push(Row { date, metric: metric.clone(), value, dims: dims.clone() });
                }
                seen.push(Obs { date, dims: dims.clone(), metric: metric.clone(), value });
            }
        }
    }
    let days: BTreeSet<NaiveDate> = totals.keys().map(|(d, _)| *d).collect();
    for d in &spec.derive {
        if let Some(of) = &d.sum {
            for day in &days {
                let v: f64 = seen.iter().filter(|o| o.date == *day && &o.metric == of && d.filter.iter().all(|(k, want)| o.dims.get(k) == Some(want))).map(|o| o.value).sum();
                totals.insert((*day, d.metric.clone()), v * d.scale.unwrap_or(1.0));
            }
        }
        if let Some((a, b)) = &d.ratio {
            for day in &days {
                let top = totals.get(&(*day, a.clone())).copied().unwrap_or(0.0);
                let bottom = totals.get(&(*day, b.clone())).copied().unwrap_or(0.0);
                totals.insert((*day, d.metric.clone()), if bottom > 0.0 { top / bottom * d.scale.unwrap_or(1.0) } else { 0.0 });
            }
        }
    }
    rows.extend(totals.into_iter().map(|((date, metric), value)| Row { date, metric, value, dims: BTreeMap::new() }));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }
    fn get(rows: &[Row], date: &str, metric: &str, dims: &[(&str, &str)]) -> Option<f64> {
        let dims: BTreeMap<String, String> = dims.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        rows.iter().find(|r| r.date == d(date) && r.metric == metric && r.dims == dims).map(|r| r.value)
    }

    #[test]
    fn channels_with_totals_and_derived() {
        let spec: SourceSpec = serde_yaml::from_str(
            r#"
requests:
  - request: { method: POST, url: "https://api.test/report" }
    rows: /rows
    date: /dimensionValues/0/value
    dims: { channel: /dimensionValues/1/value }
    metrics: { sessions: /metricValues/0/value, key_events: /metricValues/1/value }
    breakdown: [sessions]
    skip_zero: true
derive:
  - { metric: organic_sessions, sum: sessions, where: { channel: Organic Search } }
  - { metric: conversion_rate, ratio: [key_events, sessions], scale: 100 }
"#,
        )
        .unwrap();
        validate("traffic", &spec).unwrap();
        let answer = json!({ "rows": [
            { "dimensionValues": [{ "value": "20261020" }, { "value": "Organic Search" }], "metricValues": [{ "value": "30" }, { "value": "3" }] },
            { "dimensionValues": [{ "value": "20261020" }, { "value": "Direct" }], "metricValues": [{ "value": "10" }, { "value": "1" }] },
            { "dimensionValues": [{ "value": "20261021" }, { "value": "Direct" }], "metricValues": [{ "value": "0" }, { "value": "0" }] }
        ]});
        let rows = read(&spec, &[answer], &args(d("2026-10-20"), d("2026-10-21")));
        assert_eq!(get(&rows, "2026-10-20", "sessions", &[]), Some(40.0));
        assert_eq!(get(&rows, "2026-10-20", "sessions", &[("channel", "Direct")]), Some(10.0));
        assert_eq!(get(&rows, "2026-10-21", "sessions", &[("channel", "Direct")]), None);
        assert_eq!(get(&rows, "2026-10-20", "organic_sessions", &[]), Some(30.0));
        assert_eq!(get(&rows, "2026-10-21", "organic_sessions", &[]), Some(0.0));
        assert_eq!(get(&rows, "2026-10-20", "conversion_rate", &[]), Some(10.0));
        assert_eq!(get(&rows, "2026-10-21", "conversion_rate", &[]), Some(0.0));
    }

    #[test]
    fn day_totals_and_a_top_list() {
        let spec: SourceSpec = serde_yaml::from_str(
            r#"
requests:
  - request: { method: POST, url: "https://api.test/q" }
    rows: /rows
    date: /keys/0
    metrics: { clicks: /clicks, ctr: /ctr }
    scale: { ctr: 100 }
  - request: { method: POST, url: "https://api.test/q" }
    rows: /rows
    date: "{{to}}"
    dims: { query: /keys/0 }
    metrics: { clicks: /clicks }
    totals: []
"#,
        )
        .unwrap();
        let days = json!({ "rows": [{ "keys": ["2026-10-20"], "clicks": 5, "ctr": 0.05 }] });
        let top = json!({ "rows": [{ "keys": ["kiqr"], "clicks": 4 }, { "keys": ["seemo"], "clicks": 0 }] });
        let rows = read(&spec, &[days, top], &args(d("2026-10-01"), d("2026-10-20")));
        assert_eq!(get(&rows, "2026-10-20", "clicks", &[]), Some(5.0));
        assert_eq!(get(&rows, "2026-10-20", "ctr", &[]), Some(5.0));
        assert_eq!(get(&rows, "2026-10-20", "clicks", &[("query", "seemo")]), Some(0.0));
        assert_eq!(rows.iter().filter(|r| r.dims.is_empty() && r.metric == "clicks").count(), 1);
    }

    #[test]
    fn names_still_parse() {
        #[derive(Deserialize)]
        struct M {
            #[serde(deserialize_with = "deserialize", default)]
            sources: BTreeMap<String, SourceSpec>,
        }
        let m: M = serde_yaml::from_str("sources: [traffic, search]").unwrap();
        assert_eq!(m.sources.keys().collect::<Vec<_>>(), vec!["search", "traffic"]);
    }
}
