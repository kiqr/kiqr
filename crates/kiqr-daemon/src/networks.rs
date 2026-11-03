//! Networks are kinds (`extensions::Kind`). Everything KIQR knows about one comes from
//! the ability that declares it: the installed abilities a project has on,
//! and, for recognising the business's profiles and suggesting what to
//! install, the abilities listed in the marketplace. Nothing here names a
//! network.

use crate::api::ApiResult;
use crate::App;
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use kiqr_core::brand::Brand;
use kiqr_core::extensions::Kind;
use kiqr_core::project::Project;
use serde_json::{json, Value};

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/networks", get(list))
}

/// The kinds of the abilities this project has on.
pub async fn installed(app: &App, slug: &str) -> Vec<Kind> {
    crate::channels::kinds(app, slug).await.into_iter().map(|(_, k)| k).collect()
}

/// The networks this project can write posts for: the kinds an ability on
/// here publishes to that are networks (not an inbox or a blog) and that a
/// channel is, once each. With no channels, every network an ability on here
/// publishes to, so the first posts can be written before anything is set up.
pub async fn for_posts(app: &App, slug: &str) -> Vec<Kind> {
    let mut kinds: Vec<String> = Vec::new();
    for ch in kiqr_core::channels::Channel::all(&app.store, slug).unwrap_or_default().into_iter().filter(|e| !e.suggested && !e.is_list()) {
        if let Some((_, k)) = crate::channels::kind_for(app, slug, &ch).await {
            kinds.push(k.kind);
        }
    }
    let mut out: Vec<Kind> = Vec::new();
    for k in installed(app, slug).await.into_iter().filter(|k| k.is_network() && k.publishes()) {
        if !out.iter().any(|o| o.kind == k.kind) && (kinds.is_empty() || kinds.contains(&k.kind)) {
            out.push(k);
        }
    }
    out
}

/// (host, kind) for every place a profile may live: installed abilities
/// first, then whatever the marketplace lists.
pub async fn profile_hosts(app: &App, slug: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut add = |h: &str, k: &str| {
        let h = h.trim().trim_start_matches("www.").to_ascii_lowercase();
        if !h.is_empty() && !out.iter().any(|(x, _)| *x == h) {
            out.push((h, k.to_string()));
        }
    };
    // The hosts a kind's profiles live on: its link's, else its ability's.
    for (m, k) in crate::channels::kinds(app, slug).await {
        for h in m.service_card().profile_hosts {
            add(&h, &k.kind);
        }
    }
    for item in crate::market::catalogue(app).await {
        for c in &item.kinds {
            for h in &c.profile_hosts {
                add(h, &c.kind);
            }
        }
    }
    out
}

/// Every network the project knows of: installed ones in full (one entry per
/// kind, with what the abilities that declare it do there), listed ones by name.
pub async fn view(app: &App, slug: &str) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let on = crate::channels::kinds(app, slug).await;
    for (_, k) in &on {
        if seen.contains(&k.kind) {
            continue;
        }
        seen.push(k.kind.clone());
        let same: Vec<&(kiqr_core::extensions::Manifest, Kind)> = on.iter().filter(|(_, o)| o.kind == k.kind).collect();
        out.push(json!({ "kind": k.kind, "name": k.display_name(), "label": k.label, "icon": k.icon, "section": k.section(), "network": k.is_network(), "text_limit": k.text_limit, "image": k.image, "preview": k.preview, "installed": true, "publishes": same.iter().any(|(_, o)| o.publishes()), "abilities": same.iter().map(|(m, o)| json!({ "id": m.id, "name": m.name, "publishes": o.publishes() })).collect::<Vec<_>>() }));
    }
    for item in crate::market::catalogue(app).await {
        for c in &item.kinds {
            if seen.contains(&c.kind) {
                continue;
            }
            seen.push(c.kind.clone());
            out.push(json!({ "kind": c.kind, "name": c.display_name(), "icon": c.icon, "network": !c.profile_hosts.is_empty(), "installed": false, "ability": item.id }));
        }
    }
    out
}

/// The business's own profiles the crawl found (the brand's social links)
/// become owned channels, needing nothing: no ability, no credential. A
/// channel the user removed is never made again. Returns the ones made.
pub async fn channels_from_brand(app: &App, slug: &str) -> Vec<kiqr_core::channels::Channel> {
    use kiqr_core::channels::{host_of, Channel};
    let Ok(Some(brand)) = Brand::load(&app.store, slug) else { return vec![] };
    let dismissed = Project::load(&app.store, slug).map(|p| p.suggestions_dismissed).unwrap_or_default();
    let bare = |u: &str| u.trim().trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.").trim_end_matches('/').to_ascii_lowercase();
    let mut made = Vec::new();
    // The site itself is a channel too: a website, published through the ability the user picks.
    if let Some(site) = website_channel(app, slug, &dismissed).await {
        made.push(site);
    }
    for s in &brand.social {
        let url = s.url.trim();
        if url.is_empty() || host_of(url).is_empty() || dismissed.iter().any(|d| *d == format!("channel:{}", bare(url))) {
            continue;
        }
        let have = Channel::all(&app.store, slug).unwrap_or_default();
        if have.iter().any(|c| !c.url.is_empty() && bare(&c.url) == bare(url)) {
            continue;
        }
        let b = crate::channels::NewChannel { url: url.to_string(), ownership: "owned".into(), ..Default::default() };
        match crate::channels::create(app, slug, b, "crawl").await {
            Ok(c) => made.push(c),
            Err(e) => tracing::info!(slug, url, error = %e.1, "a profile the crawl found is not a channel"),
        }
    }
    made
}

/// The project's own site as a website channel, once: a channel already on
/// its site with no kind (added by hand before websites were channels)
/// becomes it. None when it is there already or the user removed it.
async fn website_channel(app: &App, slug: &str, dismissed: &[String]) -> Option<kiqr_core::channels::Channel> {
    use kiqr_core::channels::{host_of, Channel, WEBSITE};
    let url = Project::load(&app.store, slug).ok()?.url;
    let domain = host_of(&url);
    let bare = url.trim().trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.").trim_end_matches('/').to_ascii_lowercase();
    if domain.is_empty() || dismissed.iter().any(|d| *d == format!("channel:{bare}")) {
        return None;
    }
    let have = Channel::all(&app.store, slug).unwrap_or_default();
    if have.iter().any(|c| c.is_website() && kiqr_core::vault::same_site(&c.domain, &domain)) {
        return None;
    }
    if let Some(mut c) = have.into_iter().find(|c| c.kind.is_empty() && c.owned() && !c.suggested && c.domain == domain) {
        c.kind = WEBSITE.into();
        c.save(&app.store, slug).ok()?;
        let _ = app.store.commit(&format!("[auto] channels({slug}): {} is the website", c.label)).await;
        crate::channels::emit(app, slug, &c, None);
        return None;
    }
    let b = crate::channels::NewChannel { kind: WEBSITE.into(), url: url.trim().to_string(), ownership: "owned".into(), ..Default::default() };
    match crate::channels::create(app, slug, b, "crawl").await {
        Ok(c) => Some(c),
        Err(e) => {
            tracing::info!(slug, error = %e.1, "the site is not a channel");
            None
        }
    }
}

/// The marketplace ability that publishes to a site, when one does and is not installed: (id, name).
pub async fn ability_for_site(app: &App, domain: &str) -> Option<(String, String)> {
    if domain.is_empty() {
        return None;
    }
    let reg_ids: Vec<String> = app.registry.read().await.entries.iter().map(|e| e.id().to_string()).collect();
    crate::market::catalogue(app).await.into_iter().filter(|i| !reg_ids.contains(&i.id)).find(|i| i.kinds.iter().any(|k| k.profile_hosts.iter().any(|h| kiqr_core::vault::same_site(h, domain)))).map(|i| (i.id, i.name))
}

/// Seemo's line after a crawl: the profiles that became channels, and what would publish there.
pub async fn channels_line(app: &App, made: &[kiqr_core::channels::Channel]) -> Option<String> {
    if made.is_empty() {
        return None;
    }
    let names: Vec<String> = made.iter().map(|c| c.label.clone()).collect();
    let mut offers = Vec::new();
    for c in made {
        if let Some((_, name)) = ability_for_site(app, &c.domain).await {
            if !offers.contains(&format!("\"{name}\"")) {
                offers.push(format!("\"{name}\""));
            }
        }
    }
    let what = if names.len() == 1 { "it as a channel" } else { "them as channels" };
    let tail = if offers.is_empty() { " To post there, pick who I post as on each, under Marketing → Channels.".to_string() } else { format!(" To post there, install {} from the marketplace and pick who I post as; Channels and To do say what is missing.", join_and(&offers)) };
    Some(format!("I found {} on your site and added {what}.{tail}", join_and(&names)))
}

fn join_and(parts: &[String]) -> String {
    match parts.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        Some((last, _)) => last.clone(),
        None => String::new(),
    }
}

async fn list(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    Ok(Json(json!({ "networks": view(&app, &slug).await })))
}
