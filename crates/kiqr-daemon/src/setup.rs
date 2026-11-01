//! What to set up for a new project, from what the owner told Seemo in the
//! welcome conversation (the intake) and what the crawl saw on the site. Each
//! ability says when it fits in its own manifest (`fits`: goals, and text in a
//! page's HTML); a network the owner named matches the kinds an ability
//! publishes to. Nothing here names a service.

use crate::api::ApiResult;
use crate::App;
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use kiqr_core::intake::Intake;
use serde_json::{json, Value};

/// Pages read for the HTML checks, and how much of each.
const MAX_PAGES: usize = 60;
const MAX_BYTES: usize = 400_000;

pub fn routes() -> Router<App> {
    Router::new().route("/api/projects/{slug}/setup", get(suggest))
}

/// The raw HTML the crawl kept, lowercased, a bounded amount.
fn site_html(app: &App, slug: &str) -> Vec<String> {
    // Where the crawler keeps each page as fetched (`crawl.rs`).
    let Ok(rd) = std::fs::read_dir(app.store.home().cache_dir(slug).join("pages")) else { return vec![] };
    let mut files: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "html")).collect();
    files.sort();
    files
        .into_iter()
        .take(MAX_PAGES)
        .filter_map(|p| std::fs::read(&p).ok())
        .map(|b| String::from_utf8_lossy(&b[..b.len().min(MAX_BYTES)]).to_lowercase())
        .collect()
}

/// Why one ability fits: the reasons, in the order the UI says them. An
/// ability that names what to look for in the site's HTML works with something
/// the site has to run on (its CMS, say), so without it nothing else counts.
pub fn reasons(fits: &kiqr_core::extensions::Fits, channels: &[(String, String)], intake: &Intake, html: &[String]) -> Vec<Value> {
    let mut out = vec![];
    let seen = fits.html.iter().map(|h| h.to_lowercase()).filter(|h| !h.is_empty()).any(|h| html.iter().any(|page| page.contains(&h)));
    if seen {
        out.push(json!({ "kind": "site" }));
    } else if !fits.html.is_empty() {
        return out;
    }
    for named in &intake.networks {
        if let Some((_, name)) = channels.iter().find(|(k, n)| k.eq_ignore_ascii_case(named) || n.eq_ignore_ascii_case(named)) {
            out.push(json!({ "kind": "network", "name": name }));
        }
    }
    if !intake.goal.is_empty() && fits.goals.iter().any(|g| g == &intake.goal) {
        out.push(json!({ "kind": "goal", "goal": intake.goal }));
    }
    out
}

async fn suggest(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    kiqr_core::project::Project::load(&app.store, &slug)?;
    Ok(Json(suggestions(&app, &slug).await?))
}

/// What Seemo would set up, for the project's lines in Seemo's prompt: the suggestions by name, and what the owner said no to.
pub async fn prompt_lines(app: &App, slug: &str) -> Vec<String> {
    let Ok(v) = suggestions(app, slug).await else { return vec![] };
    let mut out = vec![];
    let names: Vec<String> = v["suggestions"].as_array().into_iter().flatten().map(|s| format!("{} (`{}`{})", s["name"].as_str().unwrap_or(""), s["id"].as_str().unwrap_or(""), if s["ready"].as_bool() == Some(true) { ", set up" } else { "" })).collect();
    if !names.is_empty() {
        out.push(format!("In your first conversation you suggested setting up: {}. Each has Add and an × to drop it in that list and on To do", names.join(", ")));
    }
    let dismissed = Intake::load(&app.store, slug).ok().flatten().map(|i| i.dismissed).unwrap_or_default();
    if !dismissed.is_empty() {
        out.push(format!("They dropped these suggestions; don't suggest them again: {}", dismissed.join(", ")));
    }
    out
}

pub async fn suggestions(app: &App, slug: &str) -> anyhow::Result<Value> {
    let intake = Intake::load(&app.store, slug)?.unwrap_or_default();
    let html = site_html(app, slug);
    let items = crate::market::catalogue(app).await;
    let listing = crate::market::listing(app, Some(slug)).await;
    let state = |id: &str| listing.abilities.iter().chain(listing.market.iter()).find(|v| v["id"] == id).cloned().unwrap_or(Value::Null);

    let mut out = vec![];
    for item in items.iter().filter(|i| !intake.dismissed.contains(&i.id)) {
        let channels: Vec<(String, String)> = item.channels.iter().map(|c| (c.kind.clone(), c.display_name().to_string())).collect();
        let why = reasons(&item.fits, &channels, &intake, &html);
        if why.is_empty() {
            continue;
        }
        let s = state(&item.id);
        let installed = s["installed"].as_bool().unwrap_or(false);
        let on = installed && !s["paused"].as_bool().unwrap_or(false) && !s["machine_off"].as_bool().unwrap_or(false);
        out.push(json!({
            "id": item.id,
            "name": item.name,
            "summary": item.summary,
            "icon": item.icon,
            "avatar": s["avatar"].as_str().map(str::to_string).or_else(|| (!item.avatar.is_empty()).then(|| item.avatar.clone())),
            "why": why,
            "installed": installed,
            "on": on,
            "ready": on && s["ready"].as_bool().unwrap_or(false),
        }));
    }
    // What the site shows first, then what the owner named, then the goal.
    let rank = |v: &Value| match v["why"][0]["kind"].as_str() {
        Some("site") => 0,
        Some("network") => 1,
        _ => 2,
    };
    out.sort_by_key(rank);

    // Networks the owner named that no ability publishes to yet.
    let known: Vec<(String, String)> = items.iter().flat_map(|i| i.channels.iter().map(|c| (c.kind.clone(), c.display_name().to_string()))).collect();
    let waiting: Vec<&String> = intake.networks.iter().filter(|n| !known.iter().any(|(k, name)| k.eq_ignore_ascii_case(n) || name.eq_ignore_ascii_case(n))).collect();
    Ok(json!({ "suggestions": out, "waiting": waiting }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiqr_core::extensions::Fits;

    #[test]
    fn fits_from_answers_and_the_site() {
        let fits = Fits { goals: vec!["customers".into()], html: vec!["Analytics.Example/JS".into()] };
        let intake = Intake { goal: "customers".into(), networks: vec!["Blog".into()], ..Default::default() };
        let channels = vec![("blog".to_string(), "Blog".to_string())];
        let html = vec!["<script src=\"https://analytics.example/js/x.js\">".to_string()];
        let why = reasons(&fits, &channels, &intake, &html);
        let kinds: Vec<&str> = why.iter().filter_map(|w| w["kind"].as_str()).collect();
        assert_eq!(kinds, ["site", "network", "goal"]);

        let other = Intake { goal: "repeat".into(), ..Default::default() };
        assert!(reasons(&fits, &[], &other, &[]).is_empty(), "nothing said, nothing seen");
        assert!(reasons(&fits, &channels, &intake, &["<p>a plain site</p>".to_string()]).is_empty(), "named the goal and the network, but the site doesn't run on it");

        let any_site = Fits { goals: vec!["customers".into()], html: vec![] };
        assert_eq!(reasons(&any_site, &[], &intake, &[]).len(), 1, "no HTML to look for: the goal alone is enough");
    }
}
