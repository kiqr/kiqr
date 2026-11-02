//! Accounts over HTTP: who Seemo signs in as, per project and service (two
//! WordPress sites, two Reddit logins), each with its keys, where its
//! secrets are kept. Where content goes and what Seemo may do there is the
//! channel (`channels.rs`), never the account. Whether an account works
//! is one rule (`state`), used by every route, the executor and the UI.

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use kiqr_core::accounts::{self, Account, Facts, Secrets};
use kiqr_core::channels::Channel;
use kiqr_core::events::EventKind;
use kiqr_core::extensions::{Manifest, Transport};
use kiqr_core::project::Project;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/accounts", get(list).post(create_route))
        .route("/api/projects/{slug}/accounts/{id}", get(get_one).put(update_route).delete(delete_route))
        .route("/api/projects/{slug}/accounts/{id}/secrets/{name}", put(put_secret))
        .route("/api/projects/{slug}/accounts/{id}/default", post(make_default))
        .route("/api/projects/{slug}/accounts/{id}/activity", get(activity))
        .route("/api/projects/{slug}/accounts/{id}/choices/{name}", get(choices))
        .route("/api/projects/{slug}/abilities/{ability}/settings", put(put_settings))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}
fn not_found(e: impl std::fmt::Display) -> ApiError {
    ApiError(StatusCode::NOT_FOUND, e.to_string())
}

/// The installed manifest of an ability, as this project sees it.
pub async fn manifest(app: &App, slug: &str, ability: &str) -> Option<Manifest> {
    let reg = app.registry.read().await;
    reg.entry(Some(slug), ability).and_then(|e| e.manifest.clone()).filter(|_| reg.entry(Some(slug), ability).is_some_and(|e| e.error.is_none()))
}

/// The abilities that use a service's accounts here: those on in the project,
/// else (all switched off) every installed one, so an account still reads right.
pub async fn manifests_of(app: &App, slug: &str, service: &str) -> Vec<Manifest> {
    let reg = app.registry.read().await;
    let off = crate::ext::project_off(app, slug);
    let all: Vec<Manifest> = reg.of_service(slug, service).into_iter().cloned().collect();
    let on: Vec<Manifest> = all.iter().filter(|m| !off.contains(&m.id)).cloned().collect();
    if on.is_empty() { all } else { on }
}

/// The manifest an account is shown with: the one of its service that signs
/// in (a sign-in, a login, keys), else its first.
pub async fn manifest_for(app: &App, slug: &str, c: &Account) -> Option<Manifest> {
    let all = manifests_of(app, slug, &c.service).await;
    all.iter().find(|m| !m.connect.is_empty() || m.session.is_some() || !m.auth.is_empty()).or(all.first()).cloned()
}

/// What the Source switch says, when an account of the ability can be a source.
pub fn learn_view(m: &Manifest) -> Value {
    json!(m.learn.as_ref().map(|l| json!({ "label": if l.label.is_empty() { "Learn from it into the profile".to_string() } else { l.label.clone() } })))
}

/// Whether an account works right now: the one rule (`Facts::state`) over the
/// keys every ability of its service needs, the programs they run on, and the
/// sign-in when one of them signs in on a hosted program.
pub async fn state(app: &App, slug: &str, c: &Account) -> accounts::State {
    let ms = manifests_of(app, slug, &c.service).await;
    state_with(app, slug, c, &ms)
}

/// The same rule over manifests the caller already has (one that holds the registry).
pub fn state_with(app: &App, slug: &str, c: &Account, ms: &[Manifest]) -> accounts::State {
    let home = app.store.home();
    let mut facts = Facts { session: c.session(home, slug), ..Default::default() };
    for m in ms {
        for k in c.missing(home, slug, m) {
            if !facts.missing_keys.contains(&k) {
                facts.missing_keys.push(k);
            }
        }
        if m.transport == Transport::Mcp && !crate::ext::program_found(app, Some(slug), m) {
            // The program by the name the user knows it by: the ability it belongs to ("Browse with a real browser").
            let p = crate::ext::program_of_sync(app, Some(slug), m).filter(|_| !m.program.is_empty()).map(|p| p.name).unwrap_or_else(|| if m.program.is_empty() { m.command.clone() } else { m.program.clone() });
            if !facts.missing_programs.contains(&p) {
                facts.missing_programs.push(p);
            }
        }
        facts.has_session |= m.session.is_some();
    }
    if ms.is_empty() {
        facts.missing_programs.push(c.service.clone());
    }
    facts.state()
}

/// The account's state as the UI reads it: the word, the details, and a
/// sentence that stands under the account's name ("Signed out").
pub fn state_view(_c: &Account, s: &accounts::State) -> Value {
    let say = match s {
        accounts::State::MissingKeys { keys } => format!("Missing {}", keys.join(", ")),
        accounts::State::NeedsProgram { programs } => format!("Needs a program installed first: {}", programs.join(", ")),
        accounts::State::SignedOut => "Signed out".to_string(),
        accounts::State::NeedsHand => "Needs a hand to sign in".to_string(),
        accounts::State::Checking => "Signing in".to_string(),
        accounts::State::Ready => "Ready".to_string(),
    };
    let mut v = serde_json::to_value(s).unwrap_or(Value::Null);
    v["ready"] = json!(s.ready());
    v["says"] = json!(say);
    v
}

/// The state in a sentence that names the account, for errors and Seemo: "u/acme_dev: signed out".
pub fn state_text(c: &Account, s: &accounts::State) -> String {
    let says = state_view(c, s)["says"].as_str().unwrap_or("not ready").to_string();
    let mut ch = says.chars();
    let lower = ch.next().map(|f| f.to_lowercase().collect::<String>() + ch.as_str()).unwrap_or_default();
    format!("{}: {lower}", c.label)
}

/// One account for the UI and for Seemo: never its values, only which are set.
pub async fn view(app: &App, slug: &str, c: &Account) -> Value {
    let home = app.store.home();
    let ms = manifests_of(app, slug, &c.service).await;
    let m = manifest_for(app, slug, c).await;
    let keys = c.keys(home, slug);
    let st = state(app, slug, c).await;
    let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
    v["default"] = json!(c.default);
    v["state"] = state_view(c, &st);
    v["ready"] = json!(st.ready());
    // Every ability of its service that is on here uses it, with what each does through it.
    let off = crate::ext::project_off(app, slug);
    v["used_by"] = json!(ms.iter().filter(|o| !off.contains(&o.id)).map(|o| {
        let mut does: Vec<&str> = o.tools.iter().map(|t| if t.role.is_empty() { "read" } else { t.role.as_str() }).collect();
        if o.kinds.iter().any(|k| k.publishes()) { does.push("publish"); }
        if !o.sources.is_empty() { does.push("measure"); }
        does.sort();
        does.dedup();
        json!({ "id": o.id, "name": o.name, "does": does })
    }).collect::<Vec<_>>());
    // The channels it publishes to.
    let channels: Vec<Value> = Channel::all(&app.store, slug).unwrap_or_default().into_iter().filter(|e| e.publisher == c.id && !e.suggested).map(|e| json!({ "id": e.id, "label": e.label, "domain": e.domain, "ownership": e.ownership })).collect();
    v["channels"] = json!(channels);
    v["can_publish"] = json!(ms.iter().any(|m| m.kinds.iter().any(|k| k.publishes())));
    v["can_learn"] = json!(ms.iter().any(|m| m.learn.is_some()));
    // Signing in on a hosted program: whether this account does, how it stands.
    if let Some(o) = crate::session::view(app, slug, c).await.as_object() {
        for (k, val) in o {
            v[k] = val.clone();
        }
    }
    // Where its secrets are kept, for the card: in KIQR, or a password manager's account by name.
    let store_label = if c.secrets.in_kiqr() { "KIQR".to_string() } else { Account::load(&app.store, slug, &c.secrets.store).map(|s| s.label).unwrap_or_else(|_| c.secrets.store.clone()) };
    v["secrets"] = json!({ "store": c.secrets.store, "item": c.secrets.item, "store_label": store_label, "in_kiqr": c.secrets.in_kiqr() });
    v["login_set"] = json!(Secrets::LOGIN.iter().filter(|k| keys.contains_key(**k)).collect::<Vec<_>>());
    v["identity"] = json!(identity(&ms, &keys));
    match &m {
        Some(m) => {
            let card = m.service_card();
            // The label was the user's, not the one the daemon gave it after the service.
            let s = &card.name;
            v["label_set"] = json!(own_label(&c.label, s));
            v["service_card"] = json!(card);
            v["avatar"] = json!(app.registry.read().await.entry(Some(slug), &m.id).and_then(|e| crate::market::folder_avatar(e, &m.avatar)));
            // The keys of every ability of the service, once each: which are set, which a sign-in writes.
            let mut auth: Vec<Value> = Vec::new();
            let mut seen: Vec<String> = Vec::new();
            for o in &ms {
                let written: Vec<String> = kiqr_core::connect::written_tokens(&o.connect).into_iter().filter(|w| !o.auth.iter().any(|a| &a.env == w && a.typed)).collect();
                for a in &o.auth {
                    if seen.contains(&a.env) {
                        continue;
                    }
                    seen.push(a.env.clone());
                    auth.push(json!({ "env": a.env, "label": a.label, "help": a.help, "optional": a.optional, "options": a.options, "placeholder": a.placeholder, "set": keys.contains_key(&a.env), "written": written.contains(&a.env), "choices": a.choices.is_some() }));
                }
            }
            v["auth"] = json!(auth);
            v["signed_in"] = json!(m.connect.iter().any(|w| w.satisfied(&|k: &str| keys.contains_key(k))));
            v["connect"] = kiqr_core::connect::view(&m.connect);
        }
        None => {
            v["label_set"] = json!(true);
            v["auth"] = json!([]);
            v["error"] = json!("nothing installed uses this account");
        }
    }
    v
}

/// Whether the label is one somebody gave the account, not the service's name the daemon fell back on ("X", "X 2").
pub fn own_label(label: &str, service: &str) -> bool {
    label != service && !(label.starts_with(service) && label[service.len()..].trim().chars().all(|ch| ch.is_ascii_digit()))
}

/// The account's name where it is shown beside the channels it publishes to: the name the user gave it, else
/// who it is (a repository, a site, an address), else the user name it signs in with, else its label.
pub async fn name_of(app: &App, slug: &str, c: &Account) -> String {
    let service = manifest_for(app, slug, c).await.map(|m| m.service_card().name).unwrap_or_else(|| c.service.clone());
    if own_label(&c.label, &service) {
        return c.label.clone();
    }
    let keys = c.keys(app.store.home(), slug);
    identity(&manifests_of(app, slug, &c.service).await, &keys)
        .or_else(|| keys.get(Secrets::USERNAME).map(|u| u.trim().to_string()).filter(|u| !u.is_empty()))
        .unwrap_or_else(|| c.label.clone())
}

/// Who an account is, for its name and card: the first key that is an
/// identity, not a secret (an address, a site, a repository), a URL reduced
/// to its host. Secrets never leave the daemon.
pub fn identity(ms: &[Manifest], keys: &BTreeMap<String, String>) -> Option<String> {
    let secret = |n: &str| {
        let u = n.to_ascii_uppercase();
        ["TOKEN", "SECRET", "PASSWORD", "PASS", "KEY", "FILE", "LOGIN_"].iter().any(|w| u.contains(w))
    };
    ms.iter().find_map(|m| m.auth.iter().filter(|a| !secret(&a.env)).find_map(|a| keys.get(&a.env).map(|v| v.trim()).filter(|v| !v.is_empty())).map(|v| url::Url::parse(v).ok().and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string())).unwrap_or_else(|| v.to_string())))
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct NewAccount {
    /// The service; or an ability of it (`ability`), for callers that know only that.
    pub service: String,
    pub ability: String,
    pub label: String,
    /// Keys to save right away, by name.
    pub secrets: BTreeMap<String, String>,
    /// Another account of the same service whose keys this one starts with (the same mail login).
    pub same_as: Option<String>,
    /// Where its secrets are kept, when not in KIQR.
    pub keep: Option<Secrets>,
}

/// The service a request means: `service`, else the service of `ability`.
pub async fn service_of(app: &App, slug: &str, service: &str, ability: &str) -> Result<(String, Manifest), ApiError> {
    if !service.trim().is_empty() {
        let s = service.trim().to_string();
        let ms = manifests_of(app, slug, &s).await;
        let m = ms.into_iter().next().ok_or_else(|| bad(format!("nothing installed signs in to '{s}'")))?;
        return Ok((s, m));
    }
    let m = manifest(app, slug, ability.trim()).await.ok_or_else(|| bad(format!("'{}' is not installed", ability.trim())))?;
    Ok((m.service().to_string(), m))
}

/// Add an account: its record, then its keys. The first of a service is its
/// default. Kinds whose account is the spot (an X profile) get their channel
/// (or become the publisher of the one waiting on that site) at once.
pub async fn create(app: &App, slug: &str, b: NewAccount, by: &str) -> Result<Account, ApiError> {
    let _ = Project::load(&app.store, slug)?;
    let (service, m) = service_of(app, slug, &b.service, &b.ability).await?;
    let same = match b.same_as.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(Account::find(&app.store, slug, &service, s).ok_or_else(|| bad(format!("there is no account '{s}' of {} here to share", m.service_card().name)))?),
        None => None,
    };
    let secrets = match b.keep {
        Some(k) if !k.in_kiqr() => {
            check_store(app, slug, &k)?;
            k
        }
        _ => Secrets::default(),
    };
    let siblings = Account::of(&app.store, slug, &service);
    let base = b.label.trim().to_string();
    // Unnamed, an account is who it is (a site's host, an address), else its service ("Google", "Reddit").
    let typed: BTreeMap<String, String> = b.secrets.iter().filter(|(_, v)| !v.trim().is_empty()).map(|(k, v)| (k.clone(), v.clone())).collect();
    let base = if !base.is_empty() { base } else { identity(&manifests_of(app, slug, &service).await, &typed).unwrap_or_else(|| m.service_card().name) };
    let mut label = base.clone();
    let mut n = 2;
    while siblings.iter().any(|c| c.label.eq_ignore_ascii_case(&label)) {
        label = format!("{base} {n}");
        n += 1;
    }
    let c = Account { id: accounts::new_id(&app.store, slug, &label), label, service: service.clone(), default: siblings.is_empty(), created_by: by.to_string(), secrets, ..Default::default() };
    c.save(&app.store, slug)?;
    let home = app.store.home();
    if let Some(s) = &same {
        for (k, v) in s.keys(home, slug) {
            kiqr_core::secrets::write_env(&c.env_file(home, slug), &k, &v).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }
    for (name, value) in b.secrets.iter().filter(|(_, v)| !v.trim().is_empty()) {
        if let Err(e) = crate::ext::set_account_secret(app, slug, &c.id, name, value).await {
            let _ = Account::delete(&app.store, slug, &c.id);
            return Err(e);
        }
    }
    app.store.commit(&format!("account({slug}): add {} ({})", c.label, m.service_card().name)).await?;
    app.bus.emit(Some(slug), EventKind::AccountSaved { account: c.id.clone(), label: c.label.clone(), service: c.service.clone() });
    crate::channels::channels_for_account(app, slug, &c).await;
    // Connecting is wanting it: the service's abilities switched off for this project come back on.
    let off = crate::ext::project_off(app, slug);
    let ids: Vec<String> = { let reg = app.registry.read().await; reg.of_service(slug, &service).into_iter().map(|o| o.id.clone()).collect() };
    for id in ids.into_iter().filter(|id| off.contains(id)) {
        if let Err(e) = crate::market::set_project_paused(app, slug, &id, false).await {
            tracing::warn!(slug, %id, "could not switch the ability back on: {}", e.1);
        }
    }
    Ok(c)
}

/// A password manager to keep secrets in: an account of this project, and an item named.
fn check_store(app: &App, slug: &str, k: &Secrets) -> Result<(), ApiError> {
    let store = Account::load(&app.store, slug, &k.store).map_err(|_| bad(format!("no account '{}' to keep the secrets in", k.store)))?;
    if k.item.trim().is_empty() {
        return Err(bad(format!("which item in {}? name it", store.label)));
    }
    Ok(())
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Patch {
    pub label: Option<String>,
    /// Where its secrets are kept: `{store: kiqr}`, or a password manager account with `item`.
    pub secrets: Option<Secrets>,
}

pub async fn update(app: &App, slug: &str, id: &str, p: Patch) -> Result<Account, ApiError> {
    let mut c = Account::load(&app.store, slug, id).map_err(not_found)?;
    let old_label = c.label.clone();
    if let Some(l) = p.label.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        if Account::of(&app.store, slug, &c.service).iter().any(|o| o.id != c.id && o.label.eq_ignore_ascii_case(&l)) {
            return Err(bad(format!("another account is already called '{l}'")));
        }
        c.label = l;
    }
    if let Some(k) = p.secrets {
        if !k.in_kiqr() {
            if k.store == c.id {
                return Err(bad("an account cannot keep its own secrets"));
            }
            check_store(app, slug, &k)?;
        }
        c.secrets = if k.in_kiqr() { Secrets::default() } else { k };
    }
    c.save(&app.store, slug)?;
    // The channels that are the account itself (an X profile) carry its name.
    if c.label != old_label {
        for mut e in Channel::all(&app.store, slug).unwrap_or_default().into_iter().filter(|e| e.publisher == c.id && !e.is_list() && e.fields.is_empty() && e.label == old_label) {
            e.label = c.label.clone();
            if e.save(&app.store, slug).is_ok() {
                crate::channels::emit(app, slug, &e, None);
            }
        }
    }
    app.store.commit(&format!("account({slug}): {}", c.label)).await?;
    app.bus.emit(Some(slug), EventKind::AccountSaved { account: c.id.clone(), label: c.label.clone(), service: c.service.clone() });
    Ok(c)
}

/// Remove an account, its keys and its program. Channels it published to
/// stay (they are the business's people) and say they need a publisher.
pub async fn delete(app: &App, slug: &str, id: &str) -> Result<(), ApiError> {
    let c = Account::delete(&app.store, slug, id).map_err(not_found)?;
    app.mcp.stop_account(slug, &c.id).await;
    app.store.commit(&format!("account({slug}): remove {}", c.label)).await?;
    app.bus.emit(Some(slug), EventKind::AccountDeleted { account: c.id, service: c.service });
    Ok(())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ListQuery {
    service: Option<String>,
    ability: Option<String>,
}

async fn list(State(app): State<App>, Path(slug): Path<String>, Query(q): Query<ListQuery>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    // An ability's accounts are its service's, shared with the other abilities of it.
    let service = match (q.service, q.ability.as_deref()) {
        (Some(s), _) => Some(s),
        (None, Some(a)) => Some(manifest(&app, &slug, a).await.map(|m| m.service().to_string()).unwrap_or_else(|| a.to_string())),
        _ => None,
    };
    let mut out = Vec::new();
    for c in Account::list(&app.store, &slug)?.iter().filter(|c| service.as_deref().is_none_or(|s| c.service == s)) {
        out.push(view(&app, &slug, c).await);
    }
    Ok(Json(json!({ "accounts": out })))
}

async fn create_route(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<NewAccount>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let c = create(&app, &slug, b, "user").await?;
    Ok((StatusCode::CREATED, Json(view(&app, &slug, &c).await)))
}

async fn get_one(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let c = Account::load(&app.store, &slug, &id).map_err(not_found)?;
    Ok(Json(view(&app, &slug, &c).await))
}

async fn update_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(p): Json<Patch>) -> ApiResult<Value> {
    let c = update(&app, &slug, &id, p).await?;
    Ok(Json(view(&app, &slug, &c).await))
}

async fn delete_route(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> Result<StatusCode, ApiError> {
    delete(&app, &slug, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SecretBody {
    #[serde(default)]
    value: String,
}

async fn put_secret(State(app): State<App>, Path((slug, id, name)): Path<(String, String, String)>, Json(b): Json<SecretBody>) -> ApiResult<Value> {
    let set = crate::ext::set_account_secret(&app, &slug, &id, &name, &b.value).await?;
    Ok(Json(json!({ "name": name, "set": set, "account": id })))
}

/// What a key or a setting can be set to, read through the account (`choices`).
async fn choices(State(app): State<App>, Path((slug, id, name)): Path<(String, String, String)>) -> ApiResult<Value> {
    let c = Account::load(&app.store, &slug, &id).map_err(not_found)?;
    let list = crate::ext::key_choices(&app, &slug, &c, &name).await.map_err(|e| bad(format!("{e:#}")))?;
    Ok(Json(json!({ "name": name, "choices": list })))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ActivityQuery {
    limit: Option<usize>,
}

/// What the abilities of its service did through an account, newest first, with today's count and the cap.
async fn activity(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Query(q): Query<ActivityQuery>) -> ApiResult<Value> {
    let c = Account::load(&app.store, &slug, &id).map_err(not_found)?;
    let home = app.store.home();
    let mut all = c.activity(home, &slug);
    all.reverse();
    all.truncate(q.limit.unwrap_or(50).clamp(1, 500));
    let cap = app.registry.read().await.of_service(&slug, &c.service).iter().filter_map(|m| m.daily_cap).min();
    Ok(Json(json!({ "account": c.id, "service": c.service, "today": c.actions_today(home, &slug), "daily_cap": cap, "activity": all })))
}

async fn make_default(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    Account::make_default(&app.store, &slug, &id).map_err(not_found)?;
    let c = Account::load(&app.store, &slug, &id)?;
    app.store.commit(&format!("account({slug}): {} is the default for {}", c.label, c.service)).await?;
    app.bus.emit(Some(&slug), EventKind::AccountSaved { account: c.id.clone(), label: c.label.clone(), service: c.service.clone() });
    Ok(Json(view(&app, &slug, &c).await))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SettingsBody {
    /// The account the values were picked through; the service's default when empty.
    account: String,
    values: BTreeMap<String, Value>,
}

/// `PUT …/abilities/{ability}/settings {account, values}`: what the project
/// picks once for an ability (a GA4 property, a Search Console site), through
/// one of its service's accounts. Kept in `project.yaml`, never on the account.
async fn put_settings(State(app): State<App>, Path((slug, ability)): Path<(String, String)>, Json(b): Json<SettingsBody>) -> ApiResult<Value> {
    let mut p = Project::load(&app.store, &slug)?;
    let m = manifest(&app, &slug, &ability).await.ok_or_else(|| bad(format!("'{ability}' is not installed")))?;
    if m.settings.is_empty() {
        return Err(bad(format!("{} has nothing to pick", m.name)));
    }
    let c = match b.account.trim() {
        "" => Account::default_of(&app.store, &slug, m.service()).ok_or_else(|| bad(format!("connect a {} account first", m.service_card().name)))?,
        a => Account::find(&app.store, &slug, m.service(), a).ok_or_else(|| bad(format!("no {} account '{a}' here", m.service_card().name)))?,
    };
    let mut values = BTreeMap::new();
    for (name, spec) in &m.settings {
        let label = spec["label"].as_str().unwrap_or(name);
        match b.values.get(name).map(|v| v.as_str().map(|s| s.trim().to_string()).unwrap_or_else(|| v.to_string())).filter(|v| !v.is_empty()) {
            Some(v) => {
                if let Some(opts) = spec["enum"].as_array().filter(|o| !o.is_empty()) {
                    if !opts.iter().any(|o| o.as_str() == Some(v.as_str())) {
                        return Err(bad(format!("{label} is one of {}", opts.iter().filter_map(|o| o.as_str()).collect::<Vec<_>>().join(", "))));
                    }
                }
                values.insert(name.clone(), Value::String(v));
            }
            None if !spec["optional"].as_bool().unwrap_or(false) => return Err(bad(format!("pick the {}", label.to_lowercase()))),
            None => {}
        }
    }
    if let Some(n) = b.values.keys().find(|k| !m.settings.contains_key(*k)) {
        return Err(bad(format!("{} has no setting '{n}'", m.name)));
    }
    p.ability_settings.insert(m.id.clone(), kiqr_core::project::AbilitySetting { account: c.id.clone(), values });
    p.save(&app.store)?;
    let what: Vec<String> = p.ability_settings[&m.id].values.values().filter_map(|v| v.as_str().map(str::to_string)).collect();
    app.store.commit(&format!("project({slug}): {} reads {} through {}", m.name, what.join(", "), c.label)).await?;
    app.bus.emit(Some(&slug), EventKind::AbilitySettingsSaved { ability: m.id.clone(), account: c.id.clone() });
    Ok(Json(json!({ "ability": m.id, "setting": p.ability_settings[&m.id] })))
}
