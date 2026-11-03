//! Channels over HTTP: everywhere the business's people are, owned or
//! third-party, each with what Seemo may do there and who it publishes as.
//! Also the one place that knows which channel a call means (`spot_of`): the
//! executor and the publish cards use it to decide whether Seemo may act on
//! its own (the channel's mode says `auto`), must ask (`ask`, or a spot that
//! is no channel yet: the yes adds it) or may not (`off`).

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use kiqr_core::vault::Credential;
use kiqr_core::channels::{self, host_of, Channel, Mode, Ownership, LIST, WEBSITE};
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{Kind, Manifest};
use kiqr_core::vault::same_site;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/channels", get(list).post(create_route))
        .route("/api/projects/{slug}/channels/suggest", axum::routing::post(suggest_route))
        .route("/api/projects/{slug}/channels/{id}", get(get_one).put(update_route).delete(delete_route))
        .route("/api/projects/{slug}/channels/{id}/learn", axum::routing::post(learn_now))
        .route("/api/projects/{slug}/channels/{id}/choices/{name}", get(field_choices))
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

/// A kind by name, with the ability that publishes it (or declares it).
pub async fn kind(app: &App, slug: &str, kind: &str) -> Option<(Manifest, Kind)> {
    let all = kinds(app, slug).await;
    all.iter().find(|(_, k)| k.kind == kind && k.publishes()).or_else(|| all.iter().find(|(_, k)| k.kind == kind)).cloned()
}

/// The ability on here that sends email to a list: one that provides `email`.
pub async fn list_sender(app: &App, slug: &str) -> Option<Manifest> {
    let cfg = app.config.read().await.clone();
    let reg = app.registry.read().await;
    let off = crate::ext::project_off(app, slug);
    reg.provider(slug, "email", cfg.abilities.get("email").map(String::as_str), |m| !off.contains(&m.id)).map(|(_, m)| m.clone())
}

/// The ability that publishes to a channel: a list's mail sender, else the ability of its kind.
pub async fn publishing_ability(app: &App, slug: &str, ch: &Channel) -> Option<Manifest> {
    if ch.is_list() {
        return list_sender(app, slug).await;
    }
    kind_for(app, slug, ch).await.filter(|(_, k)| k.publishes()).map(|(m, _)| m)
}

/// The site a kind is on: its link's host, else the ability's first fixed host.
pub fn kind_domain(m: &Manifest, k: &Kind) -> String {
    let h = host_of(&k.url);
    if !h.is_empty() {
        return h;
    }
    m.hosts.iter().filter(|h| !h.contains('$') && !h.contains('{')).find_map(|h| kiqr_core::vault::normalise_host(h)).unwrap_or_default()
}

/// A channel's target fields for a kind: its own, else what its link names
/// through the kind's `target_from` (a subreddit added by link before any
/// ability knew Reddit).
pub fn fields_for(ch: &Channel, k: &Kind) -> BTreeMap<String, Value> {
    if !ch.fields.is_empty() || ch.url.is_empty() {
        return ch.fields.clone();
    }
    k.target_from.keys().find_map(|a| k.target_of_url(a, &ch.url)).and_then(|f| k.check_target(&f).ok()).unwrap_or_default()
}

/// The kind a channel is, when an ability on here knows it: its own `kind`
/// (through the pinned ability when there is one), else a kind on its site
/// whose fields fit it (a profile when it has none). Publishing kinds first.
pub async fn kind_for(app: &App, slug: &str, ch: &Channel) -> Option<(Manifest, Kind)> {
    if ch.is_list() {
        return None;
    }
    let all = kinds(app, slug).await;
    // A website is published through the ability it picked, wherever it is.
    if ch.is_website() {
        return all.into_iter().find(|(m, k)| m.id == ch.ability && k.publishes_site());
    }
    let pinned = |m: &Manifest| ch.ability.is_empty() || m.id == ch.ability;
    let fits = |k: &Kind| {
        let f = fields_for(ch, k);
        if f.is_empty() { k.target.is_empty() || k.account_is_target } else { f.keys().all(|x| k.target.contains_key(x)) }
    };
    let mut found: Vec<&(Manifest, Kind)> = if ch.kind.is_empty() {
        all.iter().filter(|(m, k)| pinned(m) && fits(k) && !ch.domain.is_empty() && same_site(&kind_domain(m, k), &ch.domain)).collect()
    } else {
        all.iter().filter(|(m, k)| k.kind == ch.kind && pinned(m)).collect()
    };
    found.sort_by_key(|(_, k)| !k.publishes());
    found.first().map(|x| (*x).clone())
}

/// A website channel's address, as its calls get it (`${SITE_URL}`): its link, else its domain over https.
pub fn site_url(ch: &Channel) -> String {
    let u = ch.url.trim().trim_end_matches('/');
    if u.is_empty() { format!("https://{}", ch.domain) } else { u.to_string() }
}

/// The credentials that could publish to a channel: those in the Vault that
/// fit what its publishing ability asks for (a login for x.com, a mail server).
/// When the ability asks for one on whatever site it publishes (`${SITE_URL}`),
/// only those for this website's own domain.
pub async fn publishers(app: &App, slug: &str, ch: &Channel) -> Vec<Credential> {
    let Some(m) = publishing_ability(app, slug, ch).await else { return vec![] };
    let Some(need) = m.needs().into_iter().next() else { return vec![] };
    let any_site = !need.hosts.is_empty() && need.hosts.iter().all(|h| h.contains('$'));
    crate::creds::fitting(app, slug, &m, &need).into_iter().filter(|c| !(ch.is_website() && any_site) || c.allows_host(&ch.domain)).collect()
}

/// Where a call happens: the kind, the target fields, how it reads, and the
/// channel when there is one.
#[derive(Clone, Debug)]
pub struct Spot {
    pub kind: Kind,
    pub fields: BTreeMap<String, Value>,
    pub label: String,
    pub channel: Option<Channel>,
}

impl Spot {
    pub fn view(&self) -> Value {
        json!({ "kind": self.kind.kind, "kind_label": self.kind.label, "fields": self.fields, "label": self.label, "channel": self.channel.as_ref().map(|e| e.id.clone()), "new": self.channel.is_none() })
    }

    /// What Seemo may do here for a role: the channel's mode; somewhere new always asks.
    pub fn mode(&self, role: &str) -> Mode {
        match &self.channel {
            Some(ch) => ch.mode(role),
            None if role == "read" || role == "measure" => Mode::Auto,
            None => Mode::Ask,
        }
    }
}

/// The channel for a spot of a kind reached through an account: the same kind
/// (or a channel on the kind's site that names no kind yet) and fields; when
/// the profile itself is the spot, the one this credential publishes as.
pub fn matching(store: &kiqr_core::Store, slug: &str, publisher: &str, ability: &str, kind: &Kind, domain: &str, fields: &BTreeMap<String, Value>) -> Option<Channel> {
    Channel::all(store, slug).unwrap_or_default().into_iter().find(|ch| {
        !ch.suggested
            && !ch.is_list()
            && (ch.kind == kind.kind || (ch.is_website() && ch.ability == ability && kind.publishes_site()) || (ch.kind.is_empty() && !domain.is_empty() && same_site(&ch.domain, domain)))
            && channels::same_fields(&fields_for(ch, kind), fields)
            && (!fields.is_empty() || ch.publisher == publisher)
    })
}

/// The spot a call as a credential means, out of the ability's kinds: the kind whose publish tool it is, else one whose target fields
/// (or a URL `target_from` reads) the call carries, else the only kind when
/// the tool acts. Its fields come from the arguments, checked. None when no
/// kind is involved (a repository tool).
#[allow(clippy::too_many_arguments)]
pub fn spot_of(store: &kiqr_core::Store, slug: &str, publisher: &str, who: &str, kinds: &[(Manifest, Kind)], tool: &str, args: &Value, acts: bool) -> anyhow::Result<Option<Spot>> {
    let has = |k: &Kind| k.target.keys().any(|f| args.get(f).is_some_and(|v| !v.is_null())) || k.target_from.keys().any(|a| args.get(a).and_then(Value::as_str).is_some_and(|u| k.target_of_url(a, u).is_some()));
    let found = kinds.iter().find(|(_, k)| k.publishes() && k.publish == tool).or_else(|| kinds.iter().find(|(_, k)| has(k))).or_else(|| (acts && kinds.len() == 1).then(|| &kinds[0]));
    let Some((m, k)) = found else { return Ok(None) };
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
    let label = channels::spot_label(k, who, &fields);
    let channel = matching(store, slug, publisher, &m.id, k, &kind_domain(m, k), &fields);
    Ok(Some(Spot { kind: k.clone(), fields, label, channel }))
}

/// A spot the user said yes to becomes a channel, published to as the
/// credential the yes was for; the channel when it is there already.
pub async fn add_spot(app: &App, slug: &str, m: &Manifest, c: &Credential, spot: &Value, by: &str) -> Option<Channel> {
    let kind = spot["kind"].as_str()?;
    let fields: BTreeMap<String, Value> = spot["fields"].as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    let k = m.kinds.iter().find(|k| k.kind == kind)?.clone();
    if let Some(found) = matching(&app.store, slug, &c.id, &m.id, &k, &kind_domain(m, &k), &fields) {
        return Some(found);
    }
    let mut ch = Channel::for_spot(&k, &crate::creds::name_of(c), fields, by).ok()?;
    if ch.domain.is_empty() {
        ch.domain = kind_domain(m, &k);
    }
    ch.publisher = c.id.clone();
    // Suggested once, said yes to now: the suggestion becomes the channel.
    if let Some(s) = Channel::all(&app.store, slug).unwrap_or_default().into_iter().find(|x| x.suggested && x.kind == ch.kind && channels::same_fields(&x.fields, &ch.fields)) {
        ch.id = s.id;
        ch.created_at = s.created_at;
    } else {
        ch.id = Channel::new_id(&app.store, slug, &ch.label);
    }
    ch.save(&app.store, slug).ok()?;
    let _ = app.store.commit(&format!("[auto] channels({slug}): add {} as {}", ch.label, c.label)).await;
    added(app, slug, &ch);
    Some(ch)
}

pub fn emit(app: &App, slug: &str, ch: &Channel, contacts: Option<usize>) {
    app.bus.emit(Some(slug), EventKind::ChannelSaved { channel: ch.id.clone(), label: ch.label.clone(), ownership: if ch.owned() { "owned" } else { "third_party" }.into(), suggested: ch.suggested, contacts });
}

/// A channel was added: the event, and the first posts for its network once.
pub fn added(app: &App, slug: &str, ch: &Channel) {
    emit(app, slug, ch, ch.is_list().then_some(0));
    if !ch.suggested && !ch.is_list() {
        let (app2, slug2, ch2) = (app.clone(), slug.to_string(), ch.clone());
        tokio::spawn(async move {
            if let Some((_, k)) = kind_for(&app2, &slug2, &ch2).await {
                crate::api::posts_for_new_account(&app2, &slug2, &k.kind).await
            }
        });
    }
}

/// Whether a channel can be published to, in one word, a line, and the one fix.
pub async fn readiness(app: &App, slug: &str, ch: &Channel, kind: Option<&(Manifest, Kind)>, publisher: Option<&Credential>) -> Value {
    let ability = publishing_ability(app, slug, ch).await;
    let needs_publisher = ability.as_ref().is_none_or(|m| !m.needs().is_empty());
    let (state, text, fix) = if ch.suggested {
        ("suggested", "Suggested by Seemo: add it, or dismiss it".to_string(), json!({ "action": "add", "label": "Add" }))
    } else if ch.is_website() && kind.is_none() {
        let site = if ch.domain.is_empty() { "this site".to_string() } else { ch.domain.clone() };
        let text = if ch.ability.is_empty() { format!("Pick how {site} is published") } else { format!("What published {site} is not on here; pick how it is published") };
        ("pick_ability", text, json!({ "action": "pick_ability", "label": "Pick how it is published" }))
    } else if !ch.is_list() && kind.is_none_or(|(_, k)| !k.publishes()) {
        let site = if ch.domain.is_empty() { "this site".to_string() } else { ch.domain.clone() };
        // The marketplace ability that publishes there, when one does.
        let fix = match crate::networks::ability_for_site(app, &ch.domain).await {
            Some((id, name)) => json!({ "action": "install", "label": format!("Install {name}"), "ability": id }),
            None => json!({ "action": "abilities", "label": "Find one" }),
        };
        ("no_ability", format!("No ability here publishes to {site}"), fix)
    } else if ch.publish == Mode::Off {
        ("publish_off", "Publishing is off".to_string(), Value::Null)
    } else if ch.is_list() && ability.is_none() {
        ("no_ability", "Nothing here sends email yet".to_string(), json!({ "action": "abilities", "label": "Find one" }))
    } else if ch.publisher.is_empty() && !needs_publisher {
        ("ready", String::new(), Value::Null)
    } else if ch.publisher.is_empty() {
        let who = if ch.is_list() { "No mail server to send from".to_string() } else { format!("No publisher: pick who Seemo posts as on {}", if ch.domain.is_empty() { "it".to_string() } else { ch.domain.clone() }) };
        ("no_publisher", who, json!({ "action": "publisher", "label": "Pick a publisher" }))
    } else if let Some(c) = publisher {
        let st = crate::creds::state(app, slug, c).await;
        if st.ready() {
            ("ready", String::new(), Value::Null)
        } else {
            ("publisher_not_ready", crate::creds::state_text(&c.label, &st), json!({ "action": "credential", "label": "See to it", "credential": c.id }))
        }
    } else {
        ("publisher_gone", "The credential it published as is gone".to_string(), json!({ "action": "publisher", "label": "Pick another" }))
    };
    // Who it publishes as comes first (a repository list is read through it), then where exactly.
    let missing = kind.filter(|(_, k)| ch.is_website() && !k.target.is_empty()).and_then(|(_, k)| {
        let f = fields_for(ch, k);
        k.required().into_iter().find(|n| f.get(*n).is_none_or(|v| v.as_str().is_some_and(|s| s.trim().is_empty()))).map(|n| k.target[n]["label"].as_str().unwrap_or(n).to_lowercase())
    });
    let (state, text, fix) = match missing {
        Some(m) if state == "ready" => ("needs_fields", format!("Pick the {m}"), json!({ "action": "fields", "label": format!("Pick the {m}") })),
        _ => (state, text, fix),
    };
    json!({ "state": state, "ready": state == "ready", "text": text, "fix": fix })
}

/// One channel for the UI and Seemo: the channel, its kind's name and icon,
/// who publishes and whether that works, who could, and a list's counts.
pub async fn view(app: &App, slug: &str, ch: &Channel) -> Value {
    let mut v = json!(ch);
    v["suggested"] = json!(ch.suggested);
    let kind = kind_for(app, slug, ch).await;
    let publisher = (!ch.publisher.is_empty()).then(|| app.vault.get(&ch.publisher).ok().flatten()).flatten();
    v["publisher_view"] = match &publisher {
        Some(c) => publisher_view(app, slug, c).await,
        None => Value::Null,
    };
    // What a publisher must be, for the Add panel: the publishing ability's first need.
    let ability = publishing_ability(app, slug, ch).await;
    v["publisher_need"] = ability.as_ref().and_then(|m| m.needs().into_iter().next().map(|n| json!({ "ability": m.id, "ability_name": m.name, "describe": crate::vault::describe(app, &n), "provider_name": crate::vault::provider_name(app, &n.provider), "template": n.template, "hosts": n.hosts.iter().filter(|h| !h.contains('$')).collect::<Vec<_>>(), "provider": n.provider, "scopes": n.scopes, "connect": n.connect.as_ref().filter(|c| c.needs.iter().all(|x| x == kiqr_core::conn::SITE_URL && ch.is_website())).map(|c| json!({ "need": n.id, "label": if c.label.is_empty() { format!("Connect with {}", m.name) } else { c.label.clone() } })) }))).unwrap_or(Value::Null);
    // The profile itself is the channel and kept the name it was made with: who it is now.
    if let Some(p) = v["publisher_view"]["label"].as_str().map(str::to_string) {
        if ch.fields.is_empty() && !ch.is_list() && ch.added_by == "account" {
            v["label"] = json!(p);
        }
    }
    let mut candidates = Vec::new();
    for c in publishers(app, slug, ch).await {
        candidates.push(publisher_view(app, slug, &c).await);
    }
    v["publishers"] = json!(candidates);
    v["readiness"] = readiness(app, slug, ch, kind.as_ref(), publisher.as_ref()).await;
    if ch.is_list() {
        let lv = kiqr_core::lists::view(ch, app.store.home(), slug);
        v["contacts"] = json!(lv.contacts);
        v["unsubscribed"] = json!(lv.unsubscribed);
        v["kind_name"] = json!("Mailing list");
        v["icon"] = json!("mail");
        v["publishes"] = json!(true);
    }
    if ch.is_website() {
        // How it can be published: the abilities here that publish a site, and the marketplace's.
        v["site_ways"] = json!(site_ways(app, slug).await);
        v["kind_name"] = json!("Website");
        v["icon"] = json!("globe");
    }
    if let Some((m, k)) = kind.as_ref().filter(|_| !ch.is_list()) {
        if !ch.is_website() {
            v["kind"] = json!(k.kind);
        }
        v["kind_name"] = json!(if ch.is_website() { format!("Website through {}", k.display_name()) } else { k.display_name().to_string() });
        v["kind_label"] = json!(k.label);
        v["icon"] = json!(k.icon);
        v["publishes"] = json!(k.publishes());
        v["network"] = json!(k.is_network());
        v["ability_name"] = json!(m.name);
        v["target"] = json!(k.target);
        if k.publishes() {
            v["options"] = json!(crate::publish::publish_options(m, k));
            v["prepared"] = json!(crate::publish::prepared_inputs(m, k));
        }
        v["can_learn"] = json!(ch.owned() && m.learn.is_some());
    }
    v
}

/// A credential as a channel shows it: who it is and whether it works.
async fn publisher_view(app: &App, slug: &str, c: &Credential) -> Value {
    let st = crate::creds::state(app, slug, c).await;
    json!({ "credential": c.id, "label": crate::creds::name_of(c), "domain": c.default_domain(), "template": c.template, "state": st.word(), "ready": st.ready() })
}

/// How a website can be published: the installed abilities with a kind that
/// publishes a site, then the marketplace's not installed here.
pub async fn site_ways(app: &App, slug: &str) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for (m, k) in kinds(app, slug).await.into_iter().filter(|(_, k)| k.publishes_site()) {
        if !out.iter().any(|o| o["ability"] == m.id.as_str()) {
            out.push(json!({ "ability": m.id, "ability_name": m.name, "kind": k.kind, "name": k.display_name(), "label": k.label, "icon": k.icon, "installed": true }));
        }
    }
    let here: Vec<String> = app.registry.read().await.entries.iter().map(|e| e.id().to_string()).collect();
    for item in crate::market::catalogue(app).await {
        if here.contains(&item.id) || out.iter().any(|o| o["ability"] == item.id.as_str()) {
            continue;
        }
        if let Some(k) = item.kinds.iter().find(|k| k.site) {
            out.push(json!({ "ability": item.id, "ability_name": item.name, "kind": k.kind, "name": k.display_name(), "label": "", "icon": k.icon, "installed": false }));
        }
    }
    out
}

/// What can be added: each kind with its site and target fields, and a list.
async fn addable(app: &App, slug: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (m, k) in kinds(app, slug).await {
        if seen.contains(&k.kind) || k.section() == "list" {
            continue;
        }
        seen.push(k.kind.clone());
        out.push(json!({ "kind": k.kind, "label": k.label, "name": k.display_name(), "icon": k.icon, "domain": kind_domain(&m, &k), "ownership": if Ownership::of_kind(&k) == Ownership::Owned { "owned" } else { "third_party" }, "target": k.target, "account_is_target": k.account_is_target, "publishes": k.publishes(), "ability": m.id, "ability_name": m.name }));
    }
    out.push(json!({ "kind": LIST, "label": "List", "name": "Mailing list", "icon": "mail", "domain": "", "ownership": "owned", "target": {}, "account_is_target": false, "publishes": true }));
    out
}

async fn list(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    let _ = kiqr_core::project::Project::load(&app.store, &slug)?;
    let mut out = Vec::new();
    for ch in Channel::all(&app.store, &slug)? {
        out.push(view(&app, &slug, &ch).await);
    }
    Ok(Json(json!({ "channels": out, "kinds": addable(&app, &slug).await })))
}

async fn get_one(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let ch = Channel::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    Ok(Json(view(&app, &slug, &ch).await))
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct NewChannel {
    /// A kind an ability declares, or `list`; empty: from `url` or `domain`.
    pub kind: String,
    /// A link to it (`https://x.com/acme`, `https://reddit.com/r/selfhosted`): the site, the kind and its fields are read from it.
    pub url: String,
    /// The site, when there is no link.
    pub domain: String,
    pub fields: BTreeMap<String, Value>,
    pub label: String,
    pub description: String,
    /// `owned` or `third_party`; empty: what the kind usually is.
    pub ownership: String,
    /// Who Seemo publishes as (an account id); empty: nobody yet.
    pub publisher: String,
    pub read: Option<bool>,
    pub publish: Option<String>,
    pub reply: Option<String>,
    /// Seemo's: a suggestion for the user's Review, with why.
    pub suggested: bool,
    pub reason: String,
}

/// A kind declared by an ability installed here, on or off: adding a channel is wanting it.
async fn kind_any(app: &App, slug: &str, kind: &str) -> Option<(Manifest, Kind)> {
    if let Some(found) = self::kind(app, slug, kind).await {
        return Some(found);
    }
    let reg = app.registry.read().await;
    reg.visible(slug).into_iter().filter(|e| e.error.is_none()).filter_map(|e| e.manifest.as_ref()).find_map(|m| m.kinds.iter().find(|k| k.kind == kind).map(|k| (m.clone(), k.clone())))
}

/// The kind and fields a link names, out of the kinds installed here: a spot's
/// fields read by `target_from`, else a profile kind on the link's site.
async fn kind_of_url(app: &App, slug: &str, url: &str) -> Option<(Kind, BTreeMap<String, Value>)> {
    let host = host_of(url);
    let all: Vec<(Manifest, Kind)> = {
        let reg = app.registry.read().await;
        reg.visible(slug).into_iter().filter(|e| e.error.is_none()).filter_map(|e| e.manifest.as_ref()).flat_map(|m| m.kinds.iter().map(move |k| (m.clone(), k.clone()))).collect()
    };
    for (_, k) in &all {
        if let Some(f) = k.target_from.keys().find_map(|a| k.target_of_url(a, url)) {
            if k.check_target(&f).is_ok() {
                return Some((k.clone(), f));
            }
        }
    }
    let mut on_site: Vec<&(Manifest, Kind)> = all.iter().filter(|(m, k)| (k.account_is_target || k.target.is_empty()) && !host.is_empty() && same_site(&kind_domain(m, k), &host)).collect();
    on_site.sort_by_key(|(_, k)| !k.publishes());
    on_site.first().map(|(_, k)| (k.clone(), BTreeMap::new()))
}

/// A readable name for a link: `x.com/acme`.
fn label_of_url(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    rest.trim_start_matches("www.").trim_end_matches('/').to_string()
}

fn mode(s: Option<&String>, default: Mode, what: &str) -> Result<Mode, ApiError> {
    match s.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        None => Ok(default),
        Some(s) => Mode::parse(s).ok_or_else(|| bad(format!("{what} is off, ask or auto"))),
    }
}

/// The credential a channel publishes as, checked: it fits what the
/// publishing ability asks for. Picking it is the one grant made for the
/// user: that ability may use it in this project from now on.
async fn check_publisher(app: &App, slug: &str, ch: &Channel, id: &str) -> Result<String, ApiError> {
    let id = id.trim();
    if id.is_empty() {
        return Ok(String::new());
    }
    let all = publishers(app, slug, ch).await;
    // By id, label or who it is (`identity`, e.g. the account's handle): what a person or Seemo would call it.
    let said = id.trim_start_matches('@');
    let c = all.iter().find(|c| c.id == id || c.label.eq_ignore_ascii_case(id) || c.identity().is_some_and(|i| i.trim_start_matches('@').eq_ignore_ascii_case(said))).cloned().ok_or_else(|| {
        let what = app.vault.get(id).ok().flatten().map(|c| c.label).unwrap_or_else(|| format!("'{id}'"));
        let can: Vec<String> = all.iter().map(|c| format!("{} ({})", c.label, c.id)).collect();
        let fits = if can.is_empty() { String::new() } else { format!("; it can be {}", can.join(", ")) };
        bad(if ch.is_list() { format!("{what} is not a mail server{fits}") } else { format!("{what} cannot publish to {}{fits}", ch.label) })
    })?;
    Ok(c.id)
}

/// Add a channel: a list, a spot of a kind, or any link or site (an ability that knows it can come later).
pub async fn create(app: &App, slug: &str, b: NewChannel, by: &str) -> Result<Channel, ApiError> {
    let _ = kiqr_core::project::Project::load(&app.store, slug)?;
    let kind = b.kind.trim().to_ascii_lowercase();
    let url = b.url.trim();
    let mut ch = if kind == WEBSITE {
        // The site is the channel; how it is published is picked on it.
        let domain = if url.is_empty() { kiqr_core::vault::normalise_host(&b.domain).unwrap_or_default() } else { host_of(url) };
        if domain.is_empty() {
            return Err(bad("name the website's address"));
        }
        let label = if b.label.trim().is_empty() { domain.clone() } else { b.label.trim().to_string() };
        Channel { label, domain, kind: WEBSITE.into(), ownership: Ownership::Owned, url: url.to_string(), added_by: by.to_string(), ..Default::default() }
    } else if kind == LIST {
        if b.label.trim().is_empty() {
            return Err(bad("a list needs a name"));
        }
        if kiqr_core::lists::find(&app.store, slug, b.label.trim()).is_some() {
            return Err(bad(format!("there is a list called '{}' already", b.label.trim())));
        }
        kiqr_core::lists::new_list(&app.store, slug, &b.label, &b.description, "", by)
    } else {
        let found = if !kind.is_empty() {
            let (_, k) = kind_any(app, slug, &kind).await.ok_or_else(|| bad(format!("nothing installed here declares '{kind}'")))?;
            Some((k, b.fields.clone()))
        } else if !url.is_empty() {
            kind_of_url(app, slug, url).await
        } else {
            None
        };
        let mut ch = match found {
            Some((k, f)) => Channel::for_spot(&k, b.label.trim(), f, by).map_err(|e| bad(e.to_string()))?,
            None => {
                let domain = if url.is_empty() { kiqr_core::vault::normalise_host(&b.domain).unwrap_or_default() } else { host_of(url) };
                if domain.is_empty() {
                    return Err(bad("name the channel's site or give a link to it"));
                }
                let label = if b.label.trim().is_empty() { if url.is_empty() { domain.clone() } else { label_of_url(url) } } else { b.label.trim().to_string() };
                Channel { label, domain, added_by: by.to_string(), ..Default::default() }
            }
        };
        if !url.is_empty() {
            ch.url = url.to_string();
            if ch.domain.is_empty() {
                ch.domain = host_of(url);
            }
        }
        if !b.label.trim().is_empty() {
            ch.label = b.label.trim().to_string();
        } else if ch.label.is_empty() || ch.label == ch.kind || (ch.fields.is_empty() && !url.is_empty()) {
            ch.label = if url.is_empty() { ch.domain.clone() } else { label_of_url(url) };
        }
        ch.description = b.description.trim().to_string();
        ch
    };
    if !b.ownership.trim().is_empty() {
        ch.ownership = Ownership::parse(&b.ownership).ok_or_else(|| bad("ownership is owned or third_party"))?;
    }
    if ch.is_list() || ch.is_website() {
        ch.ownership = Ownership::Owned;
    }
    ch.read = b.read.unwrap_or(true);
    ch.publish = mode(b.publish.as_ref(), Mode::Ask, "publish")?;
    ch.reply = mode(b.reply.as_ref(), Mode::Ask, "reply")?;
    ch.publisher = check_publisher(app, slug, &ch, &b.publisher).await?;
    // A list with nobody to send from yet takes the mail server, when there is exactly one.
    if ch.is_list() && ch.publisher.is_empty() {
        if let [c] = &publishers(app, slug, &ch).await[..] {
            ch.publisher = check_publisher(app, slug, &ch, &c.id).await?;
        }
    }
    ch.suggested = b.suggested;
    ch.reason = b.reason.trim().to_string();
    ch.id = Channel::new_id(&app.store, slug, &ch.label);
    ch.save(&app.store, slug).map_err(|x| bad(x.to_string()))?;
    let what = if ch.suggested { "suggest" } else { "add" };
    app.store.commit(&format!("channels({slug}): {what} {}", ch.label)).await?;
    added(app, slug, &ch);
    Ok(ch)
}

/// Seemo's: a spot it found goes in as a suggestion, for the user's Review; nothing goes there on its own.
async fn suggest_route(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<NewChannel>) -> Result<(StatusCode, Json<Value>), ApiError> {
    if b.kind.trim() == LIST {
        return Err(bad("a list is proposed with create_channel"));
    }
    if b.reason.trim().is_empty() {
        return Err(bad("say in one line why the business's people are there"));
    }
    let ch = create(&app, &slug, NewChannel { suggested: true, ..b }, "seemo").await?;
    let mut v = view(&app, &slug, &ch).await;
    v["note"] = json!(format!("{} is suggested; it waits for the user's Review on the Channels page.", ch.label));
    Ok((StatusCode::CREATED, Json(v)))
}

async fn create_route(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<NewChannel>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let ch = create(&app, &slug, b, "user").await?;
    Ok((StatusCode::CREATED, Json(view(&app, &slug, &ch).await)))
}

#[derive(Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct Patch {
    pub label: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub ownership: Option<String>,
    /// Who Seemo publishes as; empty: nobody.
    pub publisher: Option<String>,
    /// The ability that publishes here, when several could; empty: any. For a
    /// website, how it is published: an ability with a kind that publishes a site.
    pub ability: Option<String>,
    /// The spot's target fields (a website's repository), checked against its kind.
    pub fields: Option<BTreeMap<String, Value>>,
    /// `website`: a channel on a site, with no kind yet, is the business's website.
    pub kind: Option<String>,
    pub read: Option<bool>,
    pub publish: Option<String>,
    pub reply: Option<String>,
    pub learn: Option<bool>,
    /// A suggestion the user added: it is a channel now.
    pub add: bool,
}

async fn update_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(p): Json<Patch>) -> ApiResult<Value> {
    let ch = update(&app, &slug, &id, p, "user").await?;
    Ok(Json(view(&app, &slug, &ch).await))
}

/// Change a channel (from its page, or after the user's yes to Seemo's card) and save it.
pub async fn update(app: &App, slug: &str, id: &str, p: Patch, by: &str) -> Result<Channel, ApiError> {
    let mut ch = Channel::load(&app.store, slug, id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    let was_learning = ch.learn;
    let was_publisher = ch.publisher.clone();
    apply(app, slug, &mut ch, &p).await?;
    // Picking a publisher is the one grant made for the user: the publishing ability may use it here from now on.
    if !ch.publisher.is_empty() && ch.publisher != was_publisher {
        if let Some(m) = publishing_ability(app, slug, &ch).await {
            crate::access::grant(app, slug, &m.id, &ch.publisher, "publisher").await?;
        }
    }
    let accepted = p.add && ch.suggested;
    if p.add {
        ch.suggested = false;
    }
    ch.save(&app.store, slug).map_err(|x| bad(x.to_string()))?;
    let who = if by == "user" { String::new() } else { format!(" ({by})") };
    app.store.commit(&format!("channels({slug}): {}{who}", ch.label)).await?;
    if accepted {
        added(app, slug, &ch);
    } else {
        emit(app, slug, &ch, None);
    }
    if ch.learn && !was_learning {
        let _ = crate::learn::start(app, slug, Some(ch.id.clone())).await;
    }
    Ok(ch)
}

/// A patch applied to a channel in memory, every rule checked; nothing saved.
async fn apply(app: &App, slug: &str, ch: &mut Channel, p: &Patch) -> Result<(), ApiError> {
    let (app, slug, p) = (app, slug.to_string(), p.clone());
    if let Some(l) = p.label.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        ch.label = l;
    }
    if let Some(d) = p.description {
        ch.description = d.trim().to_string();
    }
    if let Some(u) = p.url {
        ch.url = u.trim().to_string();
        let h = host_of(&ch.url);
        if !h.is_empty() && ch.kind.is_empty() {
            ch.domain = h;
        }
    }
    if let Some(o) = p.ownership.filter(|_| !ch.is_list()) {
        ch.ownership = Ownership::parse(&o).ok_or_else(|| bad("ownership is owned or third_party"))?;
    }
    if let Some(k) = p.kind.map(|k| k.trim().to_ascii_lowercase()).filter(|k| *k != ch.kind) {
        if k != WEBSITE || !ch.kind.is_empty() || !ch.owned() || ch.domain.is_empty() {
            return Err(bad("only an owned channel on a site, with no kind, can become the website"));
        }
        ch.kind = k;
        ch.ability.clear();
    }
    if let Some(a) = p.ability {
        let a = a.trim().to_string();
        if ch.is_website() && !a.is_empty() && !kinds(&app, &slug).await.iter().any(|(m, k)| m.id == a && k.publishes_site()) {
            return Err(bad(format!("'{a}' does not publish websites here; install it, or pick one that does")));
        }
        if a != ch.ability && ch.is_website() {
            // Another way of publishing: its fields and publisher were the old one's.
            ch.fields.clear();
            ch.publisher.clear();
        }
        ch.ability = a;
    }
    if let Some(f) = p.fields {
        let (_, k) = kind_for(&app, &slug, &ch).await.ok_or_else(|| bad(format!("pick how {} is published first", ch.label)))?;
        ch.fields = k.check_target(&f).map_err(|e| bad(e.to_string()))?;
    }
    if let Some(r) = p.read {
        ch.read = r;
    }
    ch.publish = mode(p.publish.as_ref(), ch.publish, "publish")?;
    ch.reply = mode(p.reply.as_ref(), ch.reply, "reply")?;
    if let Some(pb) = p.publisher {
        ch.publisher = check_publisher(&app, &slug, &ch, &pb).await?;
    }
    if let Some(l) = p.learn {
        if l && !ch.owned() {
            return Err(bad("Seemo learns only from the business's own channels"));
        }
        if l && !kind_for(&app, &slug, &ch).await.is_some_and(|(m, _)| m.learn.is_some()) {
            return Err(bad(format!("nothing to learn from on {}", ch.label)));
        }
        ch.learn = l;
    }
    Ok(())
}

/// Seemo's `update_channels`: each change checked as the Channels page would, shown as before → after.
pub async fn changes_preview(app: &App, slug: &str, payload: &Value) -> Result<Value, ApiError> {
    let list = payload["changes"].as_array().filter(|l| !l.is_empty()).ok_or_else(|| bad("name at least one channel and what changes on it"))?;
    let mut out = Vec::new();
    for c in list {
        let id = c["channel"].as_str().unwrap_or("").trim();
        let mut ch = Channel::find(&app.store, slug, id).ok_or_else(|| bad(format!("there is no channel '{id}'; list_channels has their ids")))?;
        let before = ch.clone();
        let mut patch: Patch = serde_json::from_value(c.clone()).map_err(|e| bad(format!("{}: {e}", before.label)))?;
        apply(app, slug, &mut ch, &patch).await.map_err(|ApiError(s, m)| ApiError(s, format!("{}: {m}", before.label)))?;
        // What is applied on the yes is the checked result: a publisher by its id, whatever it was called.
        if patch.publisher.is_some() {
            patch.publisher = Some(ch.publisher.clone());
        }
        let cred = |id: &str| if id.is_empty() { "nobody".to_string() } else { app.vault.view(id).ok().flatten().map(|c| c.label).unwrap_or_else(|| id.to_string()) };
        let mut diff = Vec::new();
        let mut row = |key: &str, from: String, to: String| {
            if from != to {
                diff.push(json!({ "key": key, "from": from, "to": to }));
            }
        };
        row("label", before.label.clone(), ch.label.clone());
        row("description", before.description.clone(), ch.description.clone());
        row("publisher", cred(&before.publisher), cred(&ch.publisher));
        row("publish", word(&before.publish), word(&ch.publish));
        row("reply", word(&before.reply), word(&ch.reply));
        row("read", before.read.to_string(), ch.read.to_string());
        row("learn", before.learn.to_string(), ch.learn.to_string());
        row("ability", before.ability.clone(), ch.ability.clone());
        row("fields", serde_json::to_string(&before.fields).unwrap_or_default(), serde_json::to_string(&ch.fields).unwrap_or_default());
        row("url", before.url.clone(), ch.url.clone());
        row("ownership", word(&before.ownership), word(&ch.ownership));
        if patch.add && before.suggested {
            row("added", "suggested".into(), "a channel".into());
        }
        if diff.is_empty() {
            continue;
        }
        out.push(json!({ "channel": before.id, "label": before.label, "patch": patch, "diff": diff }));
    }
    if out.is_empty() {
        return Err(bad("that changes nothing: the channels are already like that"));
    }
    Ok(json!({ "changes": out, "reason": payload["reason"].as_str().unwrap_or("") }))
}

/// After the yes: every change, in order.
pub async fn apply_changes(app: &App, slug: &str, preview: &Value, by: &str) -> Result<Value, ApiError> {
    let mut done = Vec::new();
    for c in preview["changes"].as_array().cloned().unwrap_or_default() {
        let patch: Patch = serde_json::from_value(c["patch"].clone()).map_err(|e| bad(e.to_string()))?;
        let id = c["channel"].as_str().unwrap_or("");
        let ch = update(app, slug, id, patch, by).await?;
        done.push(json!({ "channel": ch.id, "label": ch.label, "readiness": view(app, slug, &ch).await["readiness"] }));
    }
    Ok(json!({ "changed": done, "note": "Changed. Each channel's readiness says whether it can be published to now." }))
}

/// `GET …/channels/{id}/choices/{name}`: what a target field can be (a
/// website's repository), read through the channel's publisher.
async fn field_choices(State(app): State<App>, Path((slug, id, name)): Path<(String, String, String)>) -> ApiResult<Value> {
    let ch = Channel::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    let (m, k) = kind_for(&app, &slug, &ch).await.ok_or_else(|| bad(format!("pick how {} is published first", ch.label)))?;
    if !k.target.contains_key(&name) {
        return Err(bad(format!("{} has no field '{name}'", k.display_name())));
    }
    if ch.publisher.is_empty() {
        return Err(bad("pick who it publishes as first; the list is read through them"));
    }
    let conn = crate::creds::pick(&app, &slug, &m, Some(&ch.publisher)).await.map_err(|e| bad(e.to_string()))?.ok_or_else(|| bad("pick who it publishes as first"))?;
    let found = crate::ext::key_choices(&app, &slug, &m, &conn, &name).await.map_err(|e| bad(format!("{e:#}")))?;
    Ok(Json(json!({ "name": name, "choices": found })))
}

/// "Learn now": read this source again and write the profile from it.
async fn learn_now(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let ch = Channel::load(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    if !ch.learn {
        return Err(bad(format!("Seemo does not learn from {}; switch learning on first", ch.label)));
    }
    crate::learn::start(&app, &slug, Some(ch.id.clone())).await?;
    Ok(Json(json!({ "started": true })))
}

async fn delete_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> Result<StatusCode, ApiError> {
    let ch = Channel::delete(&app.store, &slug, &id).map_err(|e| ApiError(StatusCode::NOT_FOUND, e.to_string()))?;
    // A profile the crawl added and the user removed is never added again.
    if ch.added_by == "crawl" && !ch.url.is_empty() {
        if let Ok(mut p) = kiqr_core::project::Project::load(&app.store, &slug) {
            let bare = ch.url.trim().trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.").trim_end_matches('/').to_ascii_lowercase();
            p.suggestions_dismissed.push(format!("channel:{bare}"));
            let _ = p.save(&app.store);
        }
    }
    let what = if ch.suggested { "dismiss" } else { "remove" };
    app.store.commit(&format!("channels({slug}): {what} {}", ch.label)).await?;
    app.bus.emit(Some(&slug), EventKind::ChannelDeleted { channel: ch.id });
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_names_its_spot_and_the_channel_decides() {
        let dir = std::env::temp_dir().join(format!("kiqr-spot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = kiqr_core::Store::open(kiqr_core::Home::at(&dir)).unwrap();
        let m = kiqr_core::extensions::parse_manifest("id: rb\nname: R\ntransport: http\nhosts: [r.test]\ntools:\n  - { name: post, label: Posting, description: Post., role: publish, request: { method: POST, url: \"https://r.test/{{subreddit}}\" } }\n  - { name: reply, label: Replying, description: Reply., role: reply, request: { method: POST, url: \"https://r.test/x\" } }\n  - { name: search, label: Searching, description: Search., request: { method: GET, url: \"https://r.test/s\" } }\nkinds:\n  - kind: reddit\n    label: Subreddit\n    section: community\n    publish: post\n    url: \"https://reddit.com/r/{{subreddit}}\"\n    target: { subreddit: { label: Subreddit, pattern: \"^[A-Za-z0-9_]{2,21}$\" } }\n    target_from: { post_url: \"reddit.com/r/{subreddit}/\" }\n").unwrap();
        let kinds: Vec<(Manifest, Kind)> = m.kinds.iter().map(|k| (m.clone(), k.clone())).collect();
        // A reply by its post's link is in that subreddit; no channel yet, so it asks.
        let s = spot_of(&store, "acme", "u-acme", "u/acme", &kinds, "reply", &json!({ "post_url": "https://old.reddit.com/r/homelab/comments/abc/x/", "text": "hi" }), true).unwrap().unwrap();
        assert_eq!((s.label.as_str(), s.channel.is_none(), s.mode("reply")), ("r/homelab", true, Mode::Ask));
        // A channel, whatever the case; its modes decide.
        let mut ch = Channel::for_spot(&kinds[0].1, "", [("subreddit".to_string(), json!("homelab"))].into_iter().collect(), "user").unwrap();
        ch.id = "r-homelab".into();
        ch.publish = Mode::Auto;
        ch.reply = Mode::Off;
        ch.save(&store, "acme").unwrap();
        let s = spot_of(&store, "acme", "u-acme", "u/acme", &kinds, "post", &json!({ "subreddit": "HomeLab", "title": "t", "text": "x" }), true).unwrap().unwrap();
        assert_eq!((s.channel.as_ref().map(|e| e.id.as_str()), s.mode("publish"), s.mode("reply")), (Some("r-homelab"), Mode::Auto, Mode::Off));
        // A channel added before any ability knew the site (no kind yet) is found by its site.
        let mut bare = Channel { id: "r-rust".into(), label: "r/rust".into(), domain: "reddit.com".into(), fields: [("subreddit".to_string(), json!("rust"))].into_iter().collect(), ..Default::default() };
        bare.publish = Mode::Off;
        bare.save(&store, "acme").unwrap();
        let s = spot_of(&store, "acme", "u-acme", "u/acme", &kinds, "post", &json!({ "subreddit": "rust" }), true).unwrap().unwrap();
        assert_eq!(s.mode("publish"), Mode::Off);
        // Added by its link alone: the link names the subreddit.
        Channel { id: "r-golang".into(), label: "r/golang".into(), domain: "reddit.com".into(), url: "https://www.reddit.com/r/golang/".into(), reply: Mode::Auto, ..Default::default() }.save(&store, "acme").unwrap();
        let s = spot_of(&store, "acme", "u-acme", "u/acme", &kinds, "reply", &json!({ "post_url": "https://reddit.com/r/golang/comments/x/y/" }), true).unwrap().unwrap();
        assert_eq!((s.mode("reply"), s.channel.map(|c| c.id)), (Mode::Auto, Some("r-golang".to_string())));
        // A post with no subreddit is refused; a value that is not a subreddit too.
        assert!(spot_of(&store, "acme", "u-acme", "u/acme", &kinds, "post", &json!({ "title": "t" }), true).unwrap_err().to_string().contains("needs subreddit"));
        assert!(spot_of(&store, "acme", "u-acme", "u/acme", &kinds, "post", &json!({ "subreddit": "a/../b" }), true).is_err());
        // Reading names no spot.
        assert!(spot_of(&store, "acme", "u-acme", "u/acme", &kinds, "search", &json!({ "query": "backups" }), false).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn a_website_publishes_through_the_ability_it_picked() {
        let dir = std::env::temp_dir().join(format!("kiqr-site-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = kiqr_core::Store::open(kiqr_core::Home::at(&dir)).unwrap();
        let m = kiqr_core::extensions::parse_manifest("id: gh\nname: G\ntransport: http\nhosts: [api.github.test]\ntools:\n  - { name: pr, label: Opening a pull request, description: PR., role: publish, request: { method: POST, url: \"https://api.github.test/repos/{{repo}}/pulls\" } }\nkinds:\n  - kind: gh-site\n    label: Website from a repository\n    publish: pr\n    target: { repo: { label: Site repository, pattern: \"^[a-z]+/[a-z]+$\" } }\n").unwrap();
        assert!(m.kinds[0].publishes_site());
        let kinds: Vec<(Manifest, Kind)> = m.kinds.iter().map(|k| (m.clone(), k.clone())).collect();
        // The business's site, on its own domain, publishing through gh to acme/site on its own.
        Channel { id: "acme-com".into(), label: "acme.com".into(), domain: "acme.com".into(), kind: WEBSITE.into(), ability: "gh".into(), fields: [("repo".to_string(), json!("acme/site"))].into_iter().collect(), publish: Mode::Auto, ..Default::default() }.save(&store, "acme").unwrap();
        let s = spot_of(&store, "acme", "", "", &kinds, "pr", &json!({ "repo": "acme/site" }), true).unwrap().unwrap();
        assert_eq!((s.channel.as_ref().map(|c| c.id.as_str()), s.mode("publish")), (Some("acme-com"), Mode::Auto));
        // Another repository is another site: no channel, so it asks.
        let s = spot_of(&store, "acme", "", "", &kinds, "pr", &json!({ "repo": "acme/docs" }), true).unwrap().unwrap();
        assert_eq!((s.channel.is_none(), s.mode("publish")), (true, Mode::Ask));
        // Through another ability, the website is not this spot.
        assert!(matching(&store, "acme", "", "other", &kinds[0].1, "api.github.test", &[("repo".to_string(), json!("acme/site"))].into_iter().collect()).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A serialised enum's word (`auto`, `third_party`), for showing a change.
fn word<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}
