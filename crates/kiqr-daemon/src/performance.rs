//! Performance: the numbers against the plan. One view built from the
//! snapshots (`GET …/performance`), a refresh that reads the sources again,
//! and the daily pull the scheduler runs. Nothing here waits on an API when
//! the page loads; refreshing is explicit or scheduled.

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use kiqr_core::events::EventKind;
use kiqr_core::marketing::{kpi_label, Plan};
use kiqr_core::metrics::{kpi_binding, source_label, window, Agg, Snapshot, SOURCES};
use kiqr_core::project::Project;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn routes() -> Router<App> {
    Router::new().route("/api/projects/{slug}/performance", get(get_performance)).route("/api/projects/{slug}/performance/refresh", post(refresh))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PerfQuery {
    source: Option<String>,
    refresh: Option<bool>,
}

/// What a metric is called on a tile.
fn metric_label(m: &str) -> &str {
    match m {
        "sessions" => "Sessions",
        "users" => "Users",
        "pageviews" => "Page views",
        "organic_sessions" => "Organic sessions",
        "key_events" => "Key events",
        "conversion_rate" => "Conversion rate",
        "clicks" => "Clicks",
        "impressions" => "Impressions",
        "ctr" => "Click-through rate",
        "average_position" => "Average position",
        "published" => "Pieces published",
        "mentions" => "Mentions",
        other => other,
    }
}

/// Lower is better for a few metrics; the trend colour follows.
fn lower_is_better(m: &str) -> bool {
    matches!(m, "average_position" | "cac")
}

/// The whole view: sources with their state, every metric with its window
/// and series, the tops, and each plan KPI against its target.
pub async fn view(app: &App, slug: &str, only: Option<&str>) -> Result<Value, ApiError> {
    let today = Utc::now().date_naive();
    let plan = Plan::load(&app.store, slug)?;
    let mut sources = Vec::new();
    let mut metrics = Vec::new();
    let mut tops = serde_json::Map::new();
    let ids: Vec<&str> = SOURCES.iter().map(|(id, _)| *id).chain(std::iter::once("kiqr")).collect();
    for id in ids {
        if only.is_some_and(|o| o != id) {
            continue;
        }
        let snap = Snapshot::latest(&app.store, slug, id);
        let (provider, candidates) = if id == "kiqr" { (Some("kiqr".to_string()), vec![]) } else { (crate::metrics_src::provider_for(app, slug, id).await.map(|m| m.id), crate::metrics_src::candidates(app, slug, id).await) };
        sources.push(json!({
            "id": id,
            "label": if id == "kiqr" { "Published by KIQR" } else { source_label(id).unwrap_or(id) },
            "provider": provider,
            "candidates": candidates.iter().map(|(id, name, ready)| json!({ "id": id, "name": name, "ready": ready })).collect::<Vec<_>>(),
            "taken_at": snap.as_ref().map(|s| s.taken_at),
            "from": snap.as_ref().map(|s| s.from),
            "to": snap.as_ref().map(|s| s.to),
            "rows": snap.as_ref().map(|s| s.rows.len()).unwrap_or(0),
        }));
        let Some(snap) = snap else { continue };
        for m in snap.metrics() {
            let series = snap.series(&m);
            let agg = if matches!(m.as_str(), "conversion_rate" | "ctr" | "average_position") { Agg::Mean } else { Agg::Sum };
            let (now7, prev7) = window(&series, 7, agg, today);
            let (now30, _) = window(&series, 30, agg, today);
            metrics.push(json!({
                "source": id,
                "metric": m,
                "label": metric_label(&m),
                "lower_is_better": lower_is_better(&m),
                "last7": now7,
                "prev7": prev7,
                "last30": now30,
                "series": series.iter().map(|(d, v)| json!({ "date": d, "value": v })).collect::<Vec<_>>(),
            }));
        }
        for (metric, dim) in [("clicks", "query"), ("clicks", "page"), ("impressions", "query"), ("key_events", "event"), ("sessions", "channel"), ("published", "channel")] {
            let t = snap.top(metric, dim, 25);
            if !t.is_empty() {
                tops.insert(format!("{id}.{metric}.{dim}"), json!(t.iter().map(|(k, v)| json!({ "key": k, "value": v })).collect::<Vec<_>>()));
            }
        }
    }
    // Each objective's KPI against its target, from the source that measures it.
    let kpis: Vec<Value> = plan
        .as_ref()
        .map(|p| {
            p.objectives
                .iter()
                .map(|o| {
                    let bound = kpi_binding(&o.kpi);
                    let (value, prev, series_len, source) = match bound {
                        Some((src, metric, agg)) => {
                            let snap = Snapshot::latest(&app.store, slug, src);
                            match snap {
                                Some(s) => {
                                    let series = s.series(metric);
                                    let (now, prev) = window(&series, 30, agg, today);
                                    (now, prev, series.len(), Some(src))
                                }
                                None => (None, None, 0, Some(src)),
                            }
                        }
                        None => (None, None, 0, None),
                    };
                    json!({
                        "objective": o.id,
                        "title": o.title,
                        "kpi": o.kpi,
                        "label": kpi_label(&o.kpi),
                        "target": o.target,
                        "unit": o.unit,
                        "by": o.by,
                        "source": source,
                        "measured": series_len > 0,
                        "value": value,
                        "prev": prev,
                        "window_days": 30,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(json!({ "sources": sources, "metrics": metrics, "tops": tops, "kpis": kpis, "plan_period": plan.as_ref().map(|p| p.period.clone()), "today": today }))
}

async fn get_performance(State(app): State<App>, Path(slug): Path<String>, Query(q): Query<PerfQuery>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    if q.refresh == Some(true) {
        pull_all(&app, &slug, q.source.as_deref()).await;
    }
    Ok(Json(view(&app, &slug, q.source.as_deref()).await?))
}

/// `POST …/performance/refresh {source?}`: read the sources again now.
async fn refresh(State(app): State<App>, Path(slug): Path<String>, Json(q): Json<PerfQuery>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let results = pull_all(&app, &slug, q.source.as_deref()).await;
    if results.iter().all(|(_, ok)| !ok) && !results.is_empty() {
        return Err(ApiError(StatusCode::BAD_GATEWAY, results.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>().join("; ")));
    }
    let mut v = view(&app, &slug, q.source.as_deref()).await?;
    v["refreshed"] = json!(results.iter().map(|(s, ok)| json!({ "note": s, "ok": ok })).collect::<Vec<_>>());
    Ok(Json(v))
}

/// Pull every source that has a reader (or one), keep what worked, say what
/// failed. Returns one line per source tried.
pub async fn pull_all(app: &App, slug: &str, only: Option<&str>) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let ids: Vec<&str> = SOURCES.iter().map(|(id, _)| *id).chain(std::iter::once("kiqr")).collect();
    for id in ids {
        if only.is_some_and(|o| o != id) {
            continue;
        }
        if id != "kiqr" && crate::metrics_src::provider_for(app, slug, id).await.is_none() {
            continue;
        }
        let key = format!("metrics:{slug}/{id}");
        if !app.jobs.write().await.insert(key.clone()) {
            continue;
        }
        let r = crate::metrics_src::pull(app, slug, id).await;
        app.jobs.write().await.remove(&key);
        match r {
            Ok(s) => {
                let _ = app.store.commit(&format!("[auto] metrics({slug}): {id} via {}", s.provider)).await;
                app.bus.emit(Some(slug), EventKind::MetricsSnapshot { source: id.to_string(), provider: s.provider.clone(), rows: s.rows.len(), taken_at: s.taken_at });
                out.push((format!("{id}: {} rows via {}", s.rows.len(), s.provider), true));
            }
            Err(e) => {
                let msg = format!("{e:#}");
                tracing::warn!(slug, source = id, error = %msg, "reading a metrics source failed");
                app.bus.emit(Some(slug), EventKind::MetricsFailed { source: id.to_string(), provider: crate::metrics_src::provider_for(app, slug, id).await.map(|m| m.id).unwrap_or_default(), error: msg.clone() });
                out.push((format!("{id}: {msg}"), false));
            }
        }
    }
    out
}

/// The daily pull: once a day per project, after 05:00 local, for every
/// source with a reader; `kiqr` always. The scheduler calls this every tick.
pub async fn pull_due(app: &App, slug: &str, tz: chrono_tz::Tz) {
    let now_local = Utc::now().with_timezone(&tz);
    if now_local.format("%H").to_string().parse::<u32>().unwrap_or(0) < 5 {
        return;
    }
    let today = Utc::now().date_naive();
    let ids: Vec<&str> = SOURCES.iter().map(|(id, _)| *id).chain(std::iter::once("kiqr")).collect();
    for id in ids {
        let fresh = Snapshot::latest(&app.store, slug, id).is_some_and(|s| s.taken_at.date_naive() == today);
        if fresh {
            continue;
        }
        if id != "kiqr" && crate::metrics_src::provider_for(app, slug, id).await.is_none() {
            continue;
        }
        // Only pull `kiqr` on its own when something else is measured too, or there is a plan.
        if id == "kiqr" && Plan::load(&app.store, slug).ok().flatten().is_none() {
            continue;
        }
        pull_all(app, slug, Some(id)).await;
    }
}
