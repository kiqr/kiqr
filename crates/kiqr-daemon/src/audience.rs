//! The Audience over HTTP: everywhere the business's people are (pages,
//! communities, lists), each reached through an account. Also the one place
//! that knows which spot a call means (`spot_of`): the executor and the
//! publish cards use it to decide whether Seemo may act on its own there
//! (only in the Audience) and to add a spot the user approved.

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use kiqr_core::accounts::Account;
use kiqr_core::audience::{self, Entry, LIST};
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{Kind, Manifest};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/audience", get(list).post(create_route))
        .route("/api/projects/{slug}/audience/suggest", axum::routing::post(suggest_route))
        .route("/api/projects/{slug}/audience/{id}", get(get_one).put(update_route).delete(delete_route))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

/// Every kind the project's abilities declare (those on), with the ability that declares it.
pub async fn kinds(app: &App, slug: &str) -> Vec<(Manifest, Kind)> {
    let reg = app.registry.read().await;
    let off = crate::ext::project_off(app, slug);
    reg.connections(slug).into_iter().filter(|(_, m)| !off.contains(&m.id)).flat_map(|(_, m)| m.kinds.iter().map(move |k| (m.clone(), k.clone()))).collect()
}

/// The kinds of one service, the ones that publish first.
pub async fn kinds_of(app: &App, slug: &str, service: &str) -> Vec<(Manifest, Kind)> {
    let mut v: Vec<(Manifest, Kind)> = crate::accounts::manifests_of(app, slug, service).await.into_iter().flat_map(|m| m.kinds.clone().into_iter().map(move |k| (m.clone(), k))).collect();
    v.sort_by_key(|(_, k)| !k.publishes());
    v
}

/// A kind by name, with the ability that publishes it (or declares it).
pub async fn kind(app: &App, slug: &str, kind: &str) -> Option<(Manifest, Kind)> {
    let all = kinds(app, slug).await;
    all.iter().find(|(_, k)| k.kind == kind && k.publishes()).or_else(|| all.iter().find(|(_, k)| k.kind == kind)).cloned()
}

/// Whether accounts of a service send email to lists: one of its kinds lists people.
pub async fn sends_to_lists(app: &App, slug: &str, service: &str) -> bool {
    kinds_of(app, slug, service).await.iter().any(|(_, k)| k.section() == "list")
}

/// Where a call happens: the kind, the target fields, how it reads, and the
/// Audience entry when the Audience has it.
#[derive(Clone, Debug)]
pub struct Spot {
    pub kind: Kind,
    pub fields: BTreeMap<String, Value>,
    pub label: String,
    pub entry: Option<Entry>,
}

impl Spot {
    pub fn view(&self) -> Value {
        json!({ "kind": self.kind.kind, "kind_label": self.kind.label, "fields": self.fields, "label": self.label, "audience": self.entry.as_ref().map(|e| e.id.clone()), "new": self.entry.is_none() })
    }
}

/// The spot a call through an account means, out of the kinds of its
/// service: the kind whose publish tool it is, else one whose target fields
/// (or a URL `target_from` reads) the call carries, else the only kind when
/// the tool acts. Its fields come from the arguments, checked. None when no
/// kind is involved (a repository tool).
pub fn spot_of(store: &kiqr_core::Store, slug: &str, c: &Account, kinds: &[Kind], tool: &str, args: &Value, acts: bool) -> anyhow::Result<Option<Spot>> {
    let has = |k: &Kind| k.target.keys().any(|f| args.get(f).is_some_and(|v| !v.is_null())) || k.target_from.keys().any(|a| args.get(a).and_then(Value::as_str).is_some_and(|u| k.target_of_url(a, u).is_some()));
    let k = kinds.iter().find(|k| k.publishes() && k.publish == tool).or_else(|| kinds.iter().find(|k| has(k))).or_else(|| (acts && kinds.len() == 1).then(|| &kinds[0]));
    let Some(k) = k else { return Ok(None) };
    let fields = if k.account_is_target {
        BTreeMap::new()
    } else {
        let mut f: BTreeMap<String, Value> = k.target.keys().filter_map(|n| args.get(n).filter(|v| !v.is_null()).map(|v| (n.clone(), v.clone()))).collect();
        if f.is_empty() {
            if let Some(found) = k.target_from.keys().find_map(|a| args.get(a).and_then(Value::as_str).and_then(|u| k.target_of_url(a, u))) {
                f = found;
            }
        }
        k.check_target(&f)?
    };
    let label = audience::spot_label(k, &c.label, &fields);
    let entry = Entry::matching(store, slug, &c.id, &k.kind, &fields);
    Ok(Some(Spot { kind: k.clone(), fields, label, entry }))
}

/// Put an approved spot into the Audience (a community Seemo found and the
/// user said yes to); the entry when it is there already.
pub async fn add_spot(app: &App, slug: &str, c: &Account, spot: &Value, by: &str) -> Option<Entry> {
    let kind = spot["kind"].as_str()?;
    let fields: BTreeMap<String, Value> = spot["fields"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    if let Some(e) = Entry::matching(&app.store, slug, &c.id, kind, &fields) {
        return Some(e);
    }
    let (_, k) = kinds_of(app, slug, &c.service).await.into_iter().find(|(_, k)| k.kind == kind)?;
    let mut e = Entry::for_spot(&k, &c.id, &c.label, fields, by).ok()?;
    // A suggestion of the same spot becomes the entry.
    if let Some(s) = Entry::all(&app.store, slug).unwrap_or_default().into_iter().find(|x| x.suggested && x.is(&c.id, &e.kind, &e.fields)) {
        e.id = s.id;
    } else {
        e.id = Entry::new_id(&app.store, slug, &e.label);
    }
    e.save(&app.store, slug).ok()?;
    app.store.commit(&format!("audience({slug}): add {} via {}", e.label, c.label)).await.ok()?;
    added(app, slug, &e);
    Some(e)
}

/// Something joined the Audience: the event, and the first posts for its network once.
pub fn added(app: &App, slug: &str, e: &Entry) {
    app.bus.emit(Some(slug), EventKind::AudienceSaved { entry: e.id.clone(), label: e.label.clone(), section: e.section.clone(), suggested: e.suggested, contacts: e.is_list().then_some(0) });
    if !e.suggested && !e.is_list() {
        let (app2, slug2, kind) = (app.clone(), slug.to_string(), e.kind.clone());
        tokio::spawn(async move { crate::api::posts_for_new_account(&app2, &slug2, &kind).await });
    }
}

/// An account whose kind makes the account itself the spot (an X profile)
/// gets its Audience entry when it is made.
pub async fn entries_for_account(app: &App, slug: &str, c: &Account) {
    for (_, k) in kinds_of(app, slug, &c.service).await.into_iter().filter(|(_, k)| k.account_is_target) {
        if Entry::matching(&app.store, slug, &c.id, &k.kind, &BTreeMap::new()).is_some() {
            continue;
        }
        let Ok(mut e) = Entry::for_spot(&k, &c.id, &c.label, BTreeMap::new(), "account") else { continue };
        e.id = Entry::new_id(&app.store, slug, &format!("{} {}", c.label, k.kind));
        if e.save(&app.store, slug).is_ok() {
            let _ = app.store.commit(&format!("[auto] audience({slug}): {} via {}", e.label, c.label)).await;
            added(app, slug, &e);
        }
    }
}

/// One entry for the UI and Seemo: the entry, the account that reaches it and
/// whether that works, the kind's name and icon, a list's counts.
pub async fn view(app: &App, slug: &str, e: &Entry) -> Value {
    let mut v = json!(e);
    v["suggested"] = json!(e.suggested);
    let acct = (!e.account.is_empty()).then(|| Account::load(&app.store, slug, &e.account).ok()).flatten();
    match &acct {
        Some(c) => {
            let st = crate::accounts::state(app, slug, c).await;
            let service = crate::accounts::manifest_for(app, slug, c).await.map(|m| m.service_card().name).unwrap_or_else(|| c.service.clone());
            let name = crate::accounts::name_of(app, slug, c).await;
            // The account itself is the spot (an X profile, a repository) and the entry kept the name it was made with: who it is now.
            if e.fields.is_empty() && e.label == c.label {
                v["label"] = json!(name);
            }
            v["via"] = json!({ "account": c.id, "label": name, "service": c.service, "service_name": service, "state": st.word(), "ready": st.ready(), "allow": { "publish": c.allow("publish"), "reply": c.allow("reply") } });
        }
        None => {
            v["via"] = Value::Null;
            v["needs_account"] = json!(true);
        }
    }
    if e.is_list() {
        let lv = kiqr_core::lists::view(e, app.store.home(), slug);
        v["contacts"] = json!(lv.contacts);
        v["unsubscribed"] = json!(lv.unsubscribed);
        v["kind_name"] = json!("Email list");
        v["icon"] = json!("mail");
    } else if let Some((_, k)) = kind(app, slug, &e.kind).await {
        v["kind_name"] = json!(k.display_name());
        v["kind_label"] = json!(k.label);
        v["icon"] = json!(k.icon);
        v["publishes"] = json!(k.publishes());
        v["network"] = json!(k.is_network());
    }
    // What a publish there offers (a draft or published post) and what Seemo prepares for each one.
    if let Some(c) = &acct {
        if let Ok((m, k)) = crate::publish::publisher_of(app, slug, c, e).await {
            v["options"] = json!(crate::publish::publish_options(&m, &k));
            v["prepared"] = json!(crate::publish::prepared_inputs(&m, &k));
            v["publishes"] = json!(true);
        }
    }
    v
}

/// What can be added: each kind with a target (or a list), with the accounts that reach it.
async fn addable(app: &App, slug: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let accounts = Account::list(&app.store, slug).unwrap_or_default();
    let mut seen: Vec<String> = Vec::new();
    for (m, k) in kinds(app, slug).await {
        if seen.contains(&k.kind) || k.section() == "list" {
            continue;
        }
        seen.push(k.kind.clone());
        let via: Vec<Value> = accounts.iter().filter(|c| c.service == m.service()).map(|c| json!({ "id": c.id, "label": c.label })).collect();
        out.push(json!({ "kind": k.kind, "label": k.label, "name": k.display_name(), "icon": k.icon, "section": k.section(), "target": k.target, "account_is_target": k.account_is_target, "publishes": k.publishes(), "service": m.service(), "service_name": m.service_card().name, "accounts": via }));
    }
    let mut senders: Vec<Value> = Vec::new();
    for c in &accounts {
        if sends_to_lists(app, slug, &c.service).await {
            senders.push(json!({ "id": c.id, "label": c.label }));
        }
    }
    out.push(json!({ "kind": LIST, "label": "List", "name": "Email list", "icon": "mail", "section": "list", "target": {}, "account_is_target": false, "accounts": senders }));
    out
}

async fn list(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    let _ = kiqr_core::project::Project::load(&app.store, &slug)?;
    let mut entries = Vec::new();
    for e in Entry::all(&app.store, &slug)? {
        entries.push(view(&app, &slug, &e).await);
    }
    Ok(Json(json!({ "entries": entries, "kinds": addable(&app, &slug).await, "sections": kiqr_core::extensions::SECTIONS })))
}

async fn get_one(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let e = Entry::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    Ok(Json(view(&app, &slug, &e).await))
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct NewEntry {
    /// A kind an ability declares, or `list`.
    pub kind: String,
    /// The account that reaches it (a list: the one it is sent from); empty: the kind's service's default.
    pub account: String,
    pub fields: BTreeMap<String, Value>,
    pub label: String,
    pub description: String,
    pub url: String,
    /// Seemo's: a suggestion for the user's Review, with why.
    pub suggested: bool,
    pub reason: String,
}

/// Add to the Audience: a list, or a spot of a kind through an account of its service.
pub async fn create(app: &App, slug: &str, b: NewEntry, by: &str) -> Result<Entry, ApiError> {
    let _ = kiqr_core::project::Project::load(&app.store, slug)?;
    let kind = b.kind.trim().to_ascii_lowercase();
    let mut e = if kind == LIST {
        if b.label.trim().is_empty() {
            return Err(bad("a list needs a name"));
        }
        let account = b.account.trim();
        if !account.is_empty() {
            let c = Account::load(&app.store, slug, account).map_err(|_| bad(format!("no account '{account}'")))?;
            if !sends_to_lists(app, slug, &c.service).await {
                return Err(bad(format!("{} does not send email", c.label)));
            }
        }
        kiqr_core::lists::new_list(&app.store, slug, &b.label, &b.description, account, by)
    } else {
        let (m, k) = kind_any(app, slug, &kind).await.ok_or_else(|| bad(format!("nothing installed here declares '{kind}'")))?;
        let c = match b.account.trim() {
            "" => Account::default_of(&app.store, slug, m.service()).ok_or_else(|| bad(format!("connect a {} account first", m.service_card().name)))?,
            a => Account::find(&app.store, slug, m.service(), a).ok_or_else(|| bad(format!("no {} account '{a}' here", m.service_card().name)))?,
        };
        let mut e = Entry::for_spot(&k, &c.id, &c.label, b.fields.clone(), by).map_err(|e| bad(e.to_string()))?;
        if !b.label.trim().is_empty() {
            e.label = b.label.trim().to_string();
        }
        if !b.url.trim().is_empty() {
            e.url = b.url.trim().to_string();
        }
        e.description = b.description.trim().to_string();
        e.id = Entry::new_id(&app.store, slug, &e.label);
        e
    };
    e.suggested = b.suggested;
    e.reason = b.reason.trim().to_string();
    e.save(&app.store, slug).map_err(|x| bad(x.to_string()))?;
    let what = if e.suggested { "suggest" } else { "add" };
    app.store.commit(&format!("audience({slug}): {what} {}", e.label)).await?;
    added(app, slug, &e);
    Ok(e)
}

/// A kind declared by an ability that is installed here, on or off: adding to the Audience is wanting it.
async fn kind_any(app: &App, slug: &str, kind: &str) -> Option<(Manifest, Kind)> {
    if let Some(found) = self::kind(app, slug, kind).await {
        return Some(found);
    }
    let reg = app.registry.read().await;
    reg.visible(slug).into_iter().filter(|e| e.error.is_none()).filter_map(|e| e.manifest.as_ref()).find_map(|m| m.kinds.iter().find(|k| k.kind == kind).map(|k| (m.clone(), k.clone())))
}

/// Seemo's: a spot it found goes in as a suggestion, for the user's Review; nothing goes there on its own.
async fn suggest_route(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<NewEntry>) -> Result<(StatusCode, Json<Value>), ApiError> {
    if b.kind.trim() == LIST {
        return Err(bad("a list is proposed with create_list"));
    }
    if b.reason.trim().is_empty() {
        return Err(bad("say in one line why the business's people are there"));
    }
    let e = create(&app, &slug, NewEntry { suggested: true, ..b }, "seemo").await?;
    let mut v = view(&app, &slug, &e).await;
    v["note"] = json!(format!("{} is suggested; it waits for the user's Review on the Audience page.", e.label));
    Ok((StatusCode::CREATED, Json(v)))
}

async fn create_route(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<NewEntry>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let e = create(&app, &slug, b, "user").await?;
    Ok((StatusCode::CREATED, Json(view(&app, &slug, &e).await)))
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Patch {
    pub label: Option<String>,
    pub description: Option<String>,
    /// Another account of the same service reaches it now (a list: another mail account).
    pub account: Option<String>,
    /// A suggestion the user added: it is in the Audience now.
    pub add: bool,
}

async fn update_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(p): Json<Patch>) -> ApiResult<Value> {
    let mut e = Entry::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    if let Some(l) = p.label.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        e.label = l;
    }
    if let Some(d) = p.description {
        e.description = d.trim().to_string();
    }
    if let Some(a) = p.account.map(|s| s.trim().to_string()) {
        let c = Account::load(&app.store, &slug, &a).map_err(|_| bad(format!("no account '{a}'")))?;
        if e.is_list() {
            if !sends_to_lists(&app, &slug, &c.service).await {
                return Err(bad(format!("{} does not send email", c.label)));
            }
        } else if !kinds_of(&app, &slug, &c.service).await.iter().any(|(_, k)| k.kind == e.kind) {
            return Err(bad(format!("{} cannot reach {}", c.label, e.label)));
        }
        e.account = c.id;
    }
    let accepted = p.add && e.suggested;
    if p.add {
        e.suggested = false;
    }
    e.save(&app.store, &slug).map_err(|x| bad(x.to_string()))?;
    app.store.commit(&format!("audience({slug}): {}", e.label)).await?;
    if accepted {
        added(&app, &slug, &e);
    } else {
        app.bus.emit(Some(&slug), EventKind::AudienceSaved { entry: e.id.clone(), label: e.label.clone(), section: e.section.clone(), suggested: e.suggested, contacts: None });
    }
    Ok(Json(view(&app, &slug, &e).await))
}

async fn delete_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> Result<StatusCode, ApiError> {
    let e = Entry::delete(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    let what = if e.suggested { "dismiss" } else { "remove" };
    app.store.commit(&format!("audience({slug}): {what} {}", e.label)).await?;
    app.bus.emit(Some(&slug), EventKind::AudienceDeleted { entry: e.id });
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_names_its_spot_and_the_audience_decides() {
        let dir = std::env::temp_dir().join(format!("kiqr-spot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = kiqr_core::Store::open(kiqr_core::Home::at(&dir)).unwrap();
        let m = kiqr_core::extensions::parse_manifest("id: rb\nname: R\ntransport: http\nhosts: [r.test]\ntools:\n  - { name: post, label: Posting, description: Post., role: publish, request: { method: POST, url: \"https://r.test/{{subreddit}}\" } }\n  - { name: reply, label: Replying, description: Reply., role: reply, request: { method: POST, url: \"https://r.test/x\" } }\n  - { name: search, label: Searching, description: Search., request: { method: GET, url: \"https://r.test/s\" } }\nkinds:\n  - kind: reddit\n    label: Subreddit\n    section: community\n    publish: post\n    url: \"https://reddit.com/r/{{subreddit}}\"\n    target: { subreddit: { label: Subreddit, pattern: \"^[A-Za-z0-9_]{2,21}$\" } }\n    target_from: { post_url: \"reddit.com/r/{subreddit}/\" }\n").unwrap();
        let c = Account { id: "u-acme".into(), label: "u/acme".into(), service: "rb".into(), ..Default::default() };
        let kinds = m.kinds.clone();
        // A reply by its post's link is in that subreddit; not in the Audience yet.
        let s = spot_of(&store, "acme", &c, &kinds, "reply", &json!({ "post_url": "https://old.reddit.com/r/homelab/comments/abc/x/", "text": "hi" }), true).unwrap().unwrap();
        assert_eq!((s.label.as_str(), s.entry.is_none()), ("r/homelab", true));
        // In the Audience, the same spot is found, whatever the case.
        let mut e = Entry::for_spot(&kinds[0], "u-acme", "u/acme", [("subreddit".to_string(), json!("homelab"))].into_iter().collect(), "user").unwrap();
        e.id = "r-homelab".into();
        e.save(&store, "acme").unwrap();
        let s = spot_of(&store, "acme", &c, &kinds, "post", &json!({ "subreddit": "HomeLab", "title": "t", "text": "x" }), true).unwrap().unwrap();
        assert_eq!(s.entry.map(|e| e.id), Some("r-homelab".into()));
        // A post with no subreddit is refused; a value that is not a subreddit too.
        assert!(spot_of(&store, "acme", &c, &kinds, "post", &json!({ "title": "t" }), true).unwrap_err().to_string().contains("needs subreddit"));
        assert!(spot_of(&store, "acme", &c, &kinds, "post", &json!({ "subreddit": "a/../b" }), true).is_err());
        // Reading names no spot.
        assert!(spot_of(&store, "acme", &c, &kinds, "search", &json!({ "query": "backups" }), false).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
