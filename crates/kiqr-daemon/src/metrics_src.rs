//! Reading the metrics sources: whichever installed ability declares a
//! source makes the requests its manifest describes, mapped into rows by
//! `kiqr_core::sources`; KIQR's own count of what went out needs none. Each
//! reads the last 30 days into a `Snapshot`; the daemon calls this once a
//! day and on "Refresh". Keys come from `.env` by name, as everywhere.

use crate::App;
use anyhow::{anyhow, Context, Result};
use chrono::{Duration, NaiveDate, Utc};
use kiqr_core::assets;
use kiqr_core::extensions::{Manifest, Provide};
use kiqr_core::metrics::{Row, Snapshot};
use std::collections::BTreeMap;

const DAYS: i64 = 30;

/// Which connection reads a source for this project: the configured one
/// (`config.abilities["source:<name>"]`), else the first that is on and ready.
pub async fn provider_for(app: &App, slug: &str, source: &str) -> Option<Manifest> {
    let cfg = app.config.read().await.clone();
    let configured = cfg.abilities.get(&format!("source:{source}")).cloned();
    let reg = app.registry.read().await;
    let off = crate::ext::project_off(app, slug);
    let candidates: Vec<&Manifest> = reg.connections(slug).into_iter().map(|(_, m)| m).filter(|m| m.sources.contains_key(source) && !off.contains(&m.id) && crate::ext::ready_in(app, Some(slug), m)).collect();
    if let Some(c) = configured {
        if let Some(m) = candidates.iter().find(|m| m.id == c) {
            return Some((*m).clone());
        }
    }
    candidates.first().map(|m| (*m).clone())
}

/// Every connection that could read a source, ready or not, for the status list.
pub async fn candidates(app: &App, slug: &str, source: &str) -> Vec<(String, String, bool)> {
    let reg = app.registry.read().await;
    let off = crate::ext::project_off(app, slug);
    reg.connections(slug).into_iter().map(|(_, m)| m).filter(|m| m.sources.contains_key(source) && !off.contains(&m.id)).map(|m| (m.id.clone(), m.name.clone(), crate::ext::ready_in(app, Some(slug), m))).collect()
}

/// Read one source now and keep the snapshot. `kiqr` needs no connection.
pub async fn pull(app: &App, slug: &str, source: &str) -> Result<Snapshot> {
    let today = Utc::now().date_naive();
    let from = today - Duration::days(DAYS - 1);
    let snap = if source == "kiqr" {
        kiqr_rows(app, slug, from, today)?
    } else {
        let m = provider_for(app, slug, source).await.ok_or_else(|| anyhow!("nothing reads {source} for this project yet; install and connect an analytics ability"))?;
        let rows = read_source(app, slug, &m, source, from, today).await?;
        Snapshot { source: source.to_string(), provider: m.id.clone(), taken_at: Utc::now(), from, to: today, rows }
    };
    snap.save(&app.store, slug)?;
    Ok(snap)
}

/// Make the manifest's requests for a source, in order, and map the answers
/// into rows (`kiqr_core::sources`). Each request goes through the same
/// executor as any other call: hosts, keys and sign-in tokens included.
async fn read_source(app: &App, slug: &str, m: &Manifest, source: &str, from: NaiveDate, to: NaiveDate) -> Result<Vec<Row>> {
    let spec = m.sources.get(source).filter(|s| !s.requests.is_empty()).ok_or_else(|| anyhow!("{} does not say how to read {source}; update it under Setup → Abilities", m.name))?;
    let args = kiqr_core::sources::args(from, to);
    let mut answers = Vec::with_capacity(spec.requests.len());
    for r in &spec.requests {
        let provide = Provide { request: Some(r.request.clone()), ..Default::default() };
        let out = crate::ext::execute(app, slug, m, &provide, None, None, &args, None).await.with_context(|| format!("{} could not read {source}", m.name))?;
        answers.push(out.json);
    }
    Ok(kiqr_core::sources::read(spec, &answers, &args))
}

/// What KIQR itself knows: pieces published per day, by channel.
fn kiqr_rows(app: &App, slug: &str, from: NaiveDate, to: NaiveDate) -> Result<Snapshot> {
    let mut by_day: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    let mut by_channel: BTreeMap<(NaiveDate, String), f64> = BTreeMap::new();
    let mut d = from;
    while d <= to {
        by_day.insert(d, 0.0);
        d += Duration::days(1);
    }
    for a in assets::list(&app.store, slug)? {
        for p in &a.publications {
            let day = p.at.date_naive();
            if day >= from && day <= to {
                *by_day.entry(day).or_default() += 1.0;
                *by_channel.entry((day, p.connector.clone())).or_default() += 1.0;
            }
        }
    }
    let mut rows: Vec<Row> = by_day.into_iter().map(|(date, value)| Row { date, metric: "published".into(), value, dims: BTreeMap::new() }).collect();
    rows.extend(by_channel.into_iter().map(|((date, ch), value)| Row { date, metric: "published".into(), value, dims: BTreeMap::from([("channel".to_string(), ch)]) }));
    Ok(Snapshot { source: "kiqr".into(), provider: "kiqr".into(), taken_at: Utc::now(), from, to, rows })
}
