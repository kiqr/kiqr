//! The marketing plan and its campaigns: the routes, the cards behind
//! `propose_plan`, `create_campaign` and `update_campaign`, and the views
//! that say what belongs to a campaign (assets with `source.campaign`,
//! tasks with `campaign`).

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::NaiveDate;
use kiqr_core::assets::{self, Asset};
use kiqr_core::events::EventKind;
use kiqr_core::marketing::{kpi_label, Campaign, CampaignStatus, Objective, Plan, KPIS};
use kiqr_core::project::Project;
use kiqr_core::tasks::Task;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/projects/{slug}/plan", get(get_plan).put(put_plan))
        .route("/api/projects/{slug}/campaigns", get(list_campaigns).post(create_campaign))
        .route("/api/projects/{slug}/campaigns/{id}", get(get_campaign).put(put_campaign).delete(delete_campaign))
        .route("/api/projects/{slug}/campaigns/{id}/attach", post(attach))
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

// ------------------------------------------------------------------ views

/// A campaign with what belongs to it.
fn campaign_view(c: &Campaign, assets: &[Asset], tasks: &[Task], plan: Option<&Plan>) -> Value {
    let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
    v["assets"] = json!(assets
        .iter()
        .filter(|a| a.source.get("campaign").and_then(|x| x.as_str()) == Some(c.id.as_str()))
        .map(|a| json!({ "type": a.kind, "id": a.id, "title": a.title, "status": a.status, "updated_at": a.updated_at }))
        .collect::<Vec<_>>());
    v["tasks"] = json!(tasks.iter().filter(|t| t.campaign.as_deref() == Some(c.id.as_str())).map(|t| json!({ "id": t.id, "title": t.title, "status": t.status.id(), "next_run": t.next_run, "runs": t.runs })).collect::<Vec<_>>());
    v["objective_title"] = json!(c.objective.as_deref().and_then(|o| plan.and_then(|p| p.objectives.iter().find(|x| x.id == o)).map(|x| x.title.clone())));
    v
}

/// Everything the Plan page and `get_plan` show.
pub fn plan_view(app: &App, slug: &str) -> Result<Value, ApiError> {
    let plan = Plan::load(&app.store, slug)?;
    let assets = assets::list(&app.store, slug).unwrap_or_default();
    let tasks = Task::list(&app.store, slug).unwrap_or_default();
    let campaigns: Vec<Value> = Campaign::list(&app.store, slug)?.iter().map(|c| campaign_view(c, &assets, &tasks, plan.as_ref())).collect();
    let kpis: Vec<Value> = KPIS.iter().map(|(id, label)| json!({ "id": id, "label": label })).collect();
    Ok(json!({ "plan": plan, "kpis": kpis, "campaigns": campaigns }))
}

async fn get_plan(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    Ok(Json(plan_view(&app, &slug)?))
}

/// The user's own edit of the plan: the whole thing, saved as the next version.
async fn put_plan(State(app): State<App>, Path(slug): Path<String>, Json(mut plan): Json<Plan>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    if let Some(current) = Plan::load(&app.store, &slug)? {
        plan.version = current.version;
    } else {
        plan.version = 0;
    }
    plan.save(&app.store, &slug, "user").map_err(|e| bad(e.to_string()))?;
    app.store.commit(&format!("plan({slug}): v{} by the user", plan.version)).await?;
    emit_plan(&app, &slug, &plan);
    Ok(Json(plan_view(&app, &slug)?))
}

fn emit_plan(app: &App, slug: &str, plan: &Plan) {
    app.bus.emit(Some(slug), EventKind::PlanSaved { version: plan.version, period: plan.period.clone(), objectives: plan.objectives.len(), by: plan.updated_by.clone() });
}

fn emit_campaign(app: &App, slug: &str, c: &Campaign, created: bool) {
    app.bus.emit(Some(slug), EventKind::CampaignSaved { campaign: c.id.clone(), title: c.title.clone(), status: status_id(c.status).into(), created });
}

fn status_id(s: CampaignStatus) -> &'static str {
    match s {
        CampaignStatus::Active => "active",
        CampaignStatus::Done => "done",
        CampaignStatus::Dropped => "dropped",
    }
}

// -------------------------------------------------------------- campaigns

#[derive(Deserialize, Default)]
#[serde(default)]
struct CampaignBody {
    title: Option<String>,
    goal: Option<String>,
    objective: Option<String>,
    channels: Option<Vec<String>>,
    start: Option<String>,
    end: Option<String>,
    status: Option<CampaignStatus>,
}

fn date(s: &Option<String>, what: &str) -> Result<Option<NaiveDate>, ApiError> {
    match s.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(d) => NaiveDate::parse_from_str(d, "%Y-%m-%d").map(Some).map_err(|_| bad(format!("{what} is not a date (YYYY-MM-DD): {d}"))),
    }
}

/// Apply a body to a campaign; checks the objective exists when the plan has any.
fn apply(c: &mut Campaign, b: &CampaignBody, plan: Option<&Plan>) -> Result<(), ApiError> {
    if let Some(t) = &b.title {
        c.title = t.trim().to_string();
    }
    if let Some(g) = &b.goal {
        c.goal = g.trim().to_string();
    }
    if let Some(o) = &b.objective {
        let o = o.trim();
        if o.is_empty() {
            c.objective = None;
        } else {
            if let Some(p) = plan {
                if !p.objectives.iter().any(|x| x.id == o) {
                    return Err(bad(format!("no objective '{o}' in the plan; objectives: {}", p.objectives.iter().map(|x| x.id.as_str()).collect::<Vec<_>>().join(", "))));
                }
            }
            c.objective = Some(o.to_string());
        }
    }
    if let Some(ch) = &b.channels {
        c.channels = ch.iter().map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect();
    }
    if b.start.is_some() {
        c.start = date(&b.start, "start")?;
    }
    if b.end.is_some() {
        c.end = date(&b.end, "end")?;
    }
    if let Some(s) = b.status {
        c.status = s;
    }
    if c.title.trim().is_empty() {
        return Err(bad("a campaign needs a title"));
    }
    Ok(())
}

async fn list_campaigns(State(app): State<App>, Path(slug): Path<String>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    Ok(Json(plan_view(&app, &slug)?["campaigns"].clone()))
}

async fn get_campaign(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let c = Campaign::load(&app.store, &slug, &id)?;
    let plan = Plan::load(&app.store, &slug)?;
    Ok(Json(campaign_view(&c, &assets::list(&app.store, &slug).unwrap_or_default(), &Task::list(&app.store, &slug).unwrap_or_default(), plan.as_ref())))
}

/// The user's own campaign, no card.
async fn create_campaign(State(app): State<App>, Path(slug): Path<String>, Json(b): Json<CampaignBody>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let _ = Project::load(&app.store, &slug)?;
    let plan = Plan::load(&app.store, &slug)?;
    let title = b.title.clone().unwrap_or_default();
    let mut c = Campaign { id: Campaign::new_id(&app.store, &slug, &title), created_by: "user".into(), ..Default::default() };
    apply(&mut c, &b, plan.as_ref())?;
    c.save(&app.store, &slug).map_err(|e| bad(e.to_string()))?;
    app.store.commit(&format!("campaign({slug}): add {}", c.title)).await?;
    emit_campaign(&app, &slug, &c, true);
    Ok((StatusCode::CREATED, Json(campaign_view(&c, &[], &[], plan.as_ref()))))
}

async fn put_campaign(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(b): Json<CampaignBody>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let plan = Plan::load(&app.store, &slug)?;
    let mut c = Campaign::load(&app.store, &slug, &id)?;
    apply(&mut c, &b, plan.as_ref())?;
    c.save(&app.store, &slug).map_err(|e| bad(e.to_string()))?;
    app.store.commit(&format!("campaign({slug}): {}", c.title)).await?;
    emit_campaign(&app, &slug, &c, false);
    Ok(Json(campaign_view(&c, &assets::list(&app.store, &slug).unwrap_or_default(), &Task::list(&app.store, &slug).unwrap_or_default(), plan.as_ref())))
}

async fn delete_campaign(State(app): State<App>, Path((slug, id)): Path<(String, String)>) -> Result<StatusCode, ApiError> {
    let _ = Project::load(&app.store, &slug)?;
    Campaign::delete(&app.store, &slug, &id)?;
    app.store.commit(&format!("campaign({slug}): remove {id}")).await?;
    app.bus.emit(Some(&slug), EventKind::CampaignDeleted { campaign: id });
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Attach {
    asset: Option<AssetRef>,
    task: Option<String>,
}

#[derive(Deserialize)]
struct AssetRef {
    #[serde(rename = "type")]
    kind: String,
    id: String,
}

/// Put an asset or a task into the campaign. Taking one out is `campaign: ""` on the asset or task itself.
async fn attach(State(app): State<App>, Path((slug, id)): Path<(String, String)>, Json(b): Json<Attach>) -> ApiResult<Value> {
    let _ = Project::load(&app.store, &slug)?;
    let c = Campaign::load(&app.store, &slug, &id)?;
    let mut did = Vec::new();
    if let Some(a) = b.asset {
        let mut asset = assets::load(&app.store, &slug, &a.kind, &a.id)?;
        asset.source.insert("campaign".into(), json!(c.id));
        let saved = assets::save(&app.store, &slug, &asset)?;
        crate::api::emit_saved(&app, &slug, &saved, false);
        did.push(format!("{}/{}", a.kind, a.id));
    }
    if let Some(t) = b.task.filter(|t| !t.trim().is_empty()) {
        let mut task = Task::load(&app.store, &slug, t.trim())?;
        task.campaign = Some(c.id.clone());
        task.save(&app.store, &slug)?;
        app.store.commit(&format!("task({slug}): {} joins campaign {}", task.title, c.title)).await?;
        crate::agent::emit_task(&app, &slug, &task);
        did.push(format!("task {}", task.id));
    }
    if did.is_empty() {
        return Err(bad("name an asset or a task to attach"));
    }
    emit_campaign(&app, &slug, &c, false);
    Ok(Json(json!({ "ok": true, "campaign": c.id, "attached": did })))
}

// -------------------------------------------------------------- approvals

/// What the plan card shows: the proposal, checked, next to what it replaces.
pub fn plan_preview(app: &App, slug: &str, payload: &Value) -> Result<Value, ApiError> {
    let period = payload["period"].as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad("a plan needs a period"))?;
    let summary = payload["summary"].as_str().map(str::trim).unwrap_or("").to_string();
    let mut objectives: Vec<Objective> = Vec::new();
    for o in payload["objectives"].as_array().cloned().unwrap_or_default() {
        let title = o["title"].as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad("an objective needs a title"))?.to_string();
        let by = o["by"].as_str().map(str::to_string);
        objectives.push(Objective {
            id: kiqr_core::marketing::short_id(&title),
            title,
            why: o["why"].as_str().unwrap_or("").trim().to_string(),
            kpi: o["kpi"].as_str().unwrap_or("").trim().to_string(),
            target: o["target"].as_f64(),
            unit: o["unit"].as_str().unwrap_or("").trim().to_string(),
            by: date(&by, "by")?,
            ..Default::default()
        });
    }
    let channels: Vec<String> = payload["channels"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    let plan = Plan { period: period.to_string(), summary, objectives, channels, ..Default::default() };
    plan.validate().map_err(|e| bad(e.to_string()))?;
    let current = Plan::load(&app.store, slug)?;
    let mut v = serde_json::to_value(&plan).unwrap_or(Value::Null);
    v["objectives"] = json!(plan.objectives.iter().map(|o| { let mut x = serde_json::to_value(o).unwrap_or(Value::Null); x["kpi_label"] = json!(kpi_label(&o.kpi)); x }).collect::<Vec<_>>());
    v["reason"] = json!(payload["reason"].as_str().unwrap_or(""));
    v["replaces"] = json!(current.as_ref().map(|c| json!({ "version": c.version, "period": c.period, "objectives": c.objectives.iter().map(|o| o.title.clone()).collect::<Vec<_>>() })));
    Ok(v)
}

/// A yes to a plan: it becomes the plan, the next version.
pub async fn approve_plan(app: &App, slug: &str, approval: &crate::seemo::Approval) -> Result<Value, ApiError> {
    let p = &approval.payload;
    let mut plan: Plan = serde_json::from_value(json!({
        "period": p["period"], "summary": p["summary"], "channels": p["channels"],
        "objectives": p["objectives"].as_array().cloned().unwrap_or_default().into_iter().map(|mut o| { if let Some(m) = o.as_object_mut() { m.remove("kpi_label"); } o }).collect::<Vec<_>>(),
    }))
    .map_err(|e| bad(e.to_string()))?;
    if let Some(current) = Plan::load(&app.store, slug)? {
        plan.version = current.version;
    }
    let by = approval.chat.as_ref().map(|c| format!("chat:{c}")).unwrap_or_else(|| "seemo".into());
    plan.save(&app.store, slug, &by).map_err(|e| bad(e.to_string()))?;
    app.store.commit(&format!("[auto] plan({slug}): v{} {}", plan.version, plan.period)).await?;
    emit_plan(app, slug, &plan);
    Ok(json!({
        "version": plan.version,
        "period": plan.period,
        "objectives": plan.objectives.iter().map(|o| json!({ "id": o.id, "title": o.title })).collect::<Vec<_>>(),
        "note": "Approved. This is the plan you work from now; tie what you make to its objectives.",
    }))
}

/// What the campaign card shows: a new campaign, or an existing one with the changes applied.
pub fn campaign_preview(app: &App, slug: &str, payload: &Value) -> Result<Value, ApiError> {
    let plan = Plan::load(&app.store, slug)?;
    let b: CampaignBody = serde_json::from_value(payload.clone()).map_err(|e| bad(format!("the campaign is not in the expected shape: {e}")))?;
    let (mut c, update, before) = match payload["id"].as_str().map(str::trim).filter(|s| !s.is_empty()) {
        Some(id) => {
            let c = Campaign::load(&app.store, slug, id)?;
            (c.clone(), true, Some(c))
        }
        None => (Campaign { created_by: "seemo".into(), ..Default::default() }, false, None),
    };
    apply(&mut c, &b, plan.as_ref())?;
    if !update {
        c.id = Campaign::new_id(&app.store, slug, &c.title);
    }
    let mut v = serde_json::to_value(&c).unwrap_or(Value::Null);
    v["update"] = json!(update);
    v["objective_title"] = json!(c.objective.as_deref().and_then(|o| plan.as_ref().and_then(|p| p.objectives.iter().find(|x| x.id == o)).map(|x| x.title.clone())));
    if let Some(b) = before {
        let mut changes = Vec::new();
        if b.title != c.title { changes.push(json!({ "key": "title", "from": b.title, "to": c.title })); }
        if b.goal != c.goal { changes.push(json!({ "key": "goal", "from": b.goal, "to": c.goal })); }
        if b.objective != c.objective { changes.push(json!({ "key": "objective", "from": b.objective, "to": c.objective })); }
        if b.channels != c.channels { changes.push(json!({ "key": "channels", "from": b.channels.join(", "), "to": c.channels.join(", ") })); }
        if b.start != c.start { changes.push(json!({ "key": "start", "from": b.start, "to": c.start })); }
        if b.end != c.end { changes.push(json!({ "key": "end", "from": b.end, "to": c.end })); }
        if b.status != c.status { changes.push(json!({ "key": "status", "from": status_id(b.status), "to": status_id(c.status) })); }
        if changes.is_empty() {
            return Err(bad("nothing changes"));
        }
        v["changes"] = json!(changes);
    }
    Ok(v)
}

/// A yes to a campaign: saved as proposed.
pub async fn approve_campaign(app: &App, slug: &str, approval: &crate::seemo::Approval) -> Result<Value, ApiError> {
    let p = &approval.payload;
    let update = p["update"].as_bool().unwrap_or(false);
    let mut c: Campaign = serde_json::from_value(json!({
        "id": p["id"], "title": p["title"], "goal": p["goal"], "objective": p["objective"], "channels": p["channels"], "start": p["start"], "end": p["end"], "status": p["status"],
        "created_by": approval.chat.as_ref().map(|c| format!("chat:{c}")).unwrap_or_else(|| "seemo".into()),
    }))
    .map_err(|e| bad(e.to_string()))?;
    if update {
        let before = Campaign::load(&app.store, slug, &c.id)?;
        c.created_by = before.created_by;
        c.created_at = before.created_at;
    } else if app.store.home().campaign_dir(slug, &c.id).exists() {
        c.id = Campaign::new_id(&app.store, slug, &c.title);
    }
    c.save(&app.store, slug).map_err(|e| bad(e.to_string()))?;
    app.store.commit(&format!("[auto] campaign({slug}): {} {}", if update { "change" } else { "add" }, c.title)).await?;
    emit_campaign(app, slug, &c, !update);
    Ok(json!({
        "campaign": c.id,
        "title": c.title,
        "status": status_id(c.status),
        "note": if update { "Changed as proposed." } else { "Approved. Put what you make for it into it with `campaign` on create_asset and create_task." },
    }))
}
