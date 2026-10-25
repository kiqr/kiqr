//! The readers behind the metrics sources: Google Analytics 4 and Search
//! Console with a service account, Plausible with an API key, and KIQR's own
//! count of what went out. Each reads the last 30 days into a `Snapshot`;
//! the daemon calls them once a day and on "Refresh". Keys come from `.env`
//! by name, as everywhere; the model never sees them.

use crate::App;
use anyhow::{anyhow, bail, Context, Result};
use chrono::{Duration, NaiveDate, Utc};
use kiqr_core::assets;
use kiqr_core::extensions::Manifest;
use kiqr_core::metrics::{Row, Snapshot};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const DAYS: i64 = 30;

/// Which connection reads a source for this project: the configured one
/// (`config.abilities["source:<name>"]`), else the first that is on and ready.
pub async fn provider_for(app: &App, slug: &str, source: &str) -> Option<Manifest> {
    let cfg = app.config.read().await.clone();
    let configured = cfg.abilities.get(&format!("source:{source}")).cloned();
    let reg = app.registry.read().await;
    let off = crate::ext::project_off(app, slug);
    let candidates: Vec<&Manifest> = reg.connections(slug).into_iter().map(|(_, m)| m).filter(|m| m.sources.iter().any(|s| s == source) && !off.contains(&m.id) && crate::ext::ready_in(app, Some(slug), m)).collect();
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
    reg.connections(slug).into_iter().map(|(_, m)| m).filter(|m| m.sources.iter().any(|s| s == source) && !off.contains(&m.id)).map(|m| (m.id.clone(), m.name.clone(), crate::ext::ready_in(app, Some(slug), m))).collect()
}

/// Read one source now and keep the snapshot. `kiqr` needs no connection.
pub async fn pull(app: &App, slug: &str, source: &str) -> Result<Snapshot> {
    let today = Utc::now().date_naive();
    let from = today - Duration::days(DAYS - 1);
    let snap = if source == "kiqr" {
        kiqr_rows(app, slug, from, today)?
    } else {
        let m = provider_for(app, slug, source).await.ok_or_else(|| anyhow!("nothing reads {source} for this project yet; install and connect an analytics ability"))?;
        let env = kiqr_core::secrets::env_fn(app.store.home(), Some(slug));
        let rows = match (m.id.as_str(), source) {
            ("google-analytics", "traffic") => ga4(&env, from, today, false).await?,
            ("google-analytics", "conversions") => ga4(&env, from, today, true).await?,
            ("search-console", "search") => gsc(&env, from, today).await?,
            ("plausible", "traffic") => plausible(&env, from, today).await?,
            (id, s) => bail!("{id} does not know how to read {s}"),
        };
        Snapshot { source: source.to_string(), provider: m.id.clone(), taken_at: Utc::now(), from, to: today, rows }
    };
    snap.save(&app.store, slug)?;
    Ok(snap)
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

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().timeout(std::time::Duration::from_secs(40)).build()?)
}

// ------------------------------------------------------------ google auth

/// An access token for a service account: a signed JWT exchanged at Google's
/// token endpoint. The key file is read at call time and never kept.
async fn google_token(env: &impl Fn(&str) -> Option<String>, scope: &str) -> Result<String> {
    let path = env("GOOGLE_SERVICE_ACCOUNT_FILE").ok_or_else(|| anyhow!("GOOGLE_SERVICE_ACCOUNT_FILE is not set"))?;
    let key: Value = serde_json::from_str(&std::fs::read_to_string(path.trim()).with_context(|| format!("reading the service account file {}", path.trim()))?).context("the service account file is not JSON")?;
    let email = key["client_email"].as_str().ok_or_else(|| anyhow!("the service account file has no client_email"))?;
    let pem = key["private_key"].as_str().ok_or_else(|| anyhow!("the service account file has no private_key"))?;
    let der = pem_to_der(pem)?;
    let pair = ring::signature::RsaKeyPair::from_pkcs8(&der).map_err(|e| anyhow!("the private key is not PKCS#8: {e}"))?;
    let now = Utc::now().timestamp();
    let b64 = |b: &[u8]| base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, b);
    let header = b64(br#"{"alg":"RS256","typ":"JWT"}"#);
    let claims = b64(json!({ "iss": email, "scope": scope, "aud": "https://oauth2.googleapis.com/token", "iat": now, "exp": now + 3600 }).to_string().as_bytes());
    let signing = format!("{header}.{claims}");
    let mut sig = vec![0u8; pair.public().modulus_len()];
    pair.sign(&ring::signature::RSA_PKCS1_SHA256, &ring::rand::SystemRandom::new(), signing.as_bytes(), &mut sig).map_err(|e| anyhow!("signing the token failed: {e}"))?;
    let jwt = format!("{signing}.{}", b64(&sig));
    let resp: Value = client()?
        .post("https://oauth2.googleapis.com/token")
        .form(&[("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"), ("assertion", &jwt)])
        .send()
        .await
        .context("reaching Google's token endpoint")?
        .json()
        .await?;
    resp["access_token"].as_str().map(str::to_string).ok_or_else(|| anyhow!("Google refused the service account: {}", resp["error_description"].as_str().or(resp["error"].as_str()).unwrap_or("no token in the answer")))
}

fn pem_to_der(pem: &str) -> Result<Vec<u8>> {
    let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect::<Vec<_>>().join("");
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, body.trim()).context("the private key is not base64")
}

fn day(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y%m%d").or_else(|_| NaiveDate::parse_from_str(s, "%Y-%m-%d")).ok()
}

// ------------------------------------------------------------------- ga4

/// Sessions, users, page views and key events per day, organic sessions from
/// the default channel group; with `conversions`, key events by event name.
async fn ga4(env: &impl Fn(&str) -> Option<String>, from: NaiveDate, to: NaiveDate, conversions: bool) -> Result<Vec<Row>> {
    let property = env("GA4_PROPERTY_ID").ok_or_else(|| anyhow!("GA4_PROPERTY_ID is not set"))?;
    let property = property.trim().trim_start_matches("properties/").to_string();
    let token = google_token(env, "https://www.googleapis.com/auth/analytics.readonly").await?;
    let url = format!("https://analyticsdata.googleapis.com/v1beta/properties/{property}:runReport");
    let range = json!([{ "startDate": from.format("%Y-%m-%d").to_string(), "endDate": to.format("%Y-%m-%d").to_string() }]);
    let body = if conversions {
        json!({ "dateRanges": range, "dimensions": [{ "name": "date" }, { "name": "eventName" }], "metrics": [{ "name": "keyEvents" }], "limit": 10000 })
    } else {
        json!({ "dateRanges": range, "dimensions": [{ "name": "date" }, { "name": "sessionDefaultChannelGroup" }], "metrics": [{ "name": "sessions" }, { "name": "totalUsers" }, { "name": "screenPageViews" }, { "name": "keyEvents" }], "limit": 10000 })
    };
    let resp = client()?.post(&url).bearer_auth(&token).json(&body).send().await.context("reaching the Analytics Data API")?;
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("Google Analytics answered {}: {}", status.as_u16(), v["error"]["message"].as_str().unwrap_or("no detail"));
    }
    let mut totals: BTreeMap<(NaiveDate, &str), f64> = BTreeMap::new();
    let mut rows: Vec<Row> = Vec::new();
    for r in v["rows"].as_array().cloned().unwrap_or_default() {
        let dims: Vec<String> = r["dimensionValues"].as_array().map(|a| a.iter().map(|d| d["value"].as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
        let mets: Vec<f64> = r["metricValues"].as_array().map(|a| a.iter().map(|m| m["value"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0)).collect()).unwrap_or_default();
        let Some(date) = dims.first().and_then(|d| day(d)) else { continue };
        if conversions {
            let event = dims.get(1).cloned().unwrap_or_default();
            let n = mets.first().copied().unwrap_or(0.0);
            *totals.entry((date, "key_events")).or_default() += n;
            if n > 0.0 {
                rows.push(Row { date, metric: "key_events".into(), value: n, dims: BTreeMap::from([("event".to_string(), event)]) });
            }
        } else {
            let group = dims.get(1).cloned().unwrap_or_default();
            let (sessions, users, views, key) = (mets.first().copied().unwrap_or(0.0), mets.get(1).copied().unwrap_or(0.0), mets.get(2).copied().unwrap_or(0.0), mets.get(3).copied().unwrap_or(0.0));
            *totals.entry((date, "sessions")).or_default() += sessions;
            *totals.entry((date, "users")).or_default() += users;
            *totals.entry((date, "pageviews")).or_default() += views;
            *totals.entry((date, "key_events")).or_default() += key;
            if group == "Organic Search" {
                *totals.entry((date, "organic_sessions")).or_default() += sessions;
            } else {
                totals.entry((date, "organic_sessions")).or_default();
            }
            if sessions > 0.0 {
                rows.push(Row { date, metric: "sessions".into(), value: sessions, dims: BTreeMap::from([("channel".to_string(), group)]) });
            }
        }
    }
    if !conversions {
        // Conversion rate per day from the totals.
        let days: Vec<NaiveDate> = totals.keys().map(|(d, _)| *d).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        for d in days {
            let s = totals.get(&(d, "sessions")).copied().unwrap_or(0.0);
            let k = totals.get(&(d, "key_events")).copied().unwrap_or(0.0);
            totals.insert((d, "conversion_rate"), if s > 0.0 { k / s * 100.0 } else { 0.0 });
        }
    }
    rows.extend(totals.into_iter().map(|((date, metric), value)| Row { date, metric: metric.to_string(), value, dims: BTreeMap::new() }));
    Ok(rows)
}

// ------------------------------------------------------------------- gsc

/// Clicks, impressions, CTR and position per day, plus the top 25 queries and pages over the window.
async fn gsc(env: &impl Fn(&str) -> Option<String>, from: NaiveDate, to: NaiveDate) -> Result<Vec<Row>> {
    let site = env("GSC_SITE_URL").ok_or_else(|| anyhow!("GSC_SITE_URL is not set"))?;
    let token = google_token(env, "https://www.googleapis.com/auth/webmasters.readonly").await?;
    let url = format!("https://searchconsole.googleapis.com/webmasters/v3/sites/{}/searchAnalytics/query", urlencoding(site.trim()));
    let c = client()?;
    let ask = |dims: Vec<&str>, limit: u32| {
        let c = c.clone();
        let url = url.clone();
        let token = token.clone();
        let dims: Vec<String> = dims.into_iter().map(str::to_string).collect();
        async move {
            let body = json!({ "startDate": from.format("%Y-%m-%d").to_string(), "endDate": to.format("%Y-%m-%d").to_string(), "dimensions": dims, "rowLimit": limit });
            let resp = c.post(&url).bearer_auth(&token).json(&body).send().await.context("reaching the Search Console API")?;
            let status = resp.status();
            let v: Value = resp.json().await.unwrap_or(Value::Null);
            if !status.is_success() {
                bail!("Search Console answered {}: {}", status.as_u16(), v["error"]["message"].as_str().unwrap_or("no detail"));
            }
            Ok::<Vec<Value>, anyhow::Error>(v["rows"].as_array().cloned().unwrap_or_default())
        }
    };
    let mut rows = Vec::new();
    for r in ask(vec!["date"], 1000).await? {
        let Some(date) = r["keys"][0].as_str().and_then(day) else { continue };
        for (k, m) in [("clicks", "clicks"), ("impressions", "impressions"), ("ctr", "ctr"), ("position", "average_position")] {
            let mut v = r[k].as_f64().unwrap_or(0.0);
            if k == "ctr" {
                v *= 100.0;
            }
            rows.push(Row { date, metric: m.into(), value: v, dims: BTreeMap::new() });
        }
    }
    for (dim, name) in [("query", "query"), ("page", "page")] {
        for r in ask(vec![dim], 25).await? {
            let Some(key) = r["keys"][0].as_str() else { continue };
            rows.push(Row { date: to, metric: "clicks".into(), value: r["clicks"].as_f64().unwrap_or(0.0), dims: BTreeMap::from([(name.to_string(), key.to_string())]) });
            rows.push(Row { date: to, metric: "impressions".into(), value: r["impressions"].as_f64().unwrap_or(0.0), dims: BTreeMap::from([(name.to_string(), key.to_string())]) });
            rows.push(Row { date: to, metric: "average_position".into(), value: r["position"].as_f64().unwrap_or(0.0), dims: BTreeMap::from([(name.to_string(), key.to_string())]) });
        }
    }
    Ok(rows)
}

fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

// ------------------------------------------------------------- plausible

async fn plausible(env: &impl Fn(&str) -> Option<String>, from: NaiveDate, to: NaiveDate) -> Result<Vec<Row>> {
    let site = env("PLAUSIBLE_SITE_ID").ok_or_else(|| anyhow!("PLAUSIBLE_SITE_ID is not set"))?;
    let key = env("PLAUSIBLE_API_KEY").ok_or_else(|| anyhow!("PLAUSIBLE_API_KEY is not set"))?;
    let host = env("PLAUSIBLE_HOST").map(|h| h.trim().trim_end_matches('/').to_string()).filter(|h| !h.is_empty()).unwrap_or_else(|| "https://plausible.io".into());
    let url = format!("{host}/api/v1/stats/timeseries");
    let resp = client()?
        .get(&url)
        .bearer_auth(key.trim())
        .query(&[("site_id", site.trim()), ("period", "custom"), ("date", &format!("{},{}", from.format("%Y-%m-%d"), to.format("%Y-%m-%d"))), ("metrics", "visitors,visits,pageviews")])
        .send()
        .await
        .context("reaching Plausible")?;
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("Plausible answered {}: {}", status.as_u16(), v["error"].as_str().unwrap_or("no detail"));
    }
    let mut rows = Vec::new();
    for r in v["results"].as_array().cloned().unwrap_or_default() {
        let Some(date) = r["date"].as_str().and_then(day) else { continue };
        rows.push(Row { date, metric: "sessions".into(), value: r["visits"].as_f64().unwrap_or(0.0), dims: BTreeMap::new() });
        rows.push(Row { date, metric: "users".into(), value: r["visitors"].as_f64().unwrap_or(0.0), dims: BTreeMap::new() });
        rows.push(Row { date, metric: "pageviews".into(), value: r["pageviews"].as_f64().unwrap_or(0.0), dims: BTreeMap::new() });
    }
    Ok(rows)
}
