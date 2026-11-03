//! Seemo changing KIQR itself: `change_kiqr` names a request one of KIQR's own pages makes
//! (`SETTINGS.md` in Seemo's guide maps them), the user sees it on a card, and the yes runs it
//! against the daemon's own routes, marked as Seemo's, so a route only a person in the browser
//! may use refuses it as it refuses any agent. What is never Seemo's is refused before the card:
//! secret values, the people on a list, the user's own decisions, other projects, the system.

use crate::api::{ApiError, ApiResult};
use crate::App;
use axum::body::Body;
use axum::extract::Path;
use axum::http::{Method, Request, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use tower::ServiceExt;

pub fn routes() -> Router<App> {
    Router::new().route("/api/projects/{slug}/kiqr/settings-map", get(map))
}

/// `GET …/kiqr/settings-map`: every change a page can make, by route and body.
async fn map(Path(_slug): Path<String>) -> ApiResult<Value> {
    Ok(Json(json!({ "text": kiqr_core::agent::SETTINGS_MAP })))
}

fn bad(m: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, m.into())
}

/// Paths (method, pattern) never run for Seemo; `*` is one segment, a trailing `**` anything below.
const NEVER: &[(&str, &str, &str)] = &[
    ("*", "/api/secrets/**", "secret values are typed by the user under Settings"),
    ("*", "/api/projects/*/secrets/**", "secret values are typed by the user under Settings"),
    ("*", "/api/projects/*/approvals/**", "answering a card is the user's"),
    ("*", "/api/projects/*/assets/*/*/attempt", "whether an uncertain publish went out is the user's to say"),
    ("*", "/api/projects/*/assets/*/*/unpublish", "unpublishing is the user's own move"),
    ("POST", "/api/projects/*/assets/*/*/approve", "approve or publish with publish_asset: its card is the question"),
    ("POST", "/api/projects/*/assets/*/*/publish", "publish with publish_asset: its card is the question"),
    ("POST", "/api/projects/*/assets/*/*/schedule", "schedule with publish_asset and `at`"),
    ("*", "/api/projects/*/email", "send email with send_email"),
    ("*", "/api/projects/*/channels/*/contacts", "the people on a list are the user's to handle"),
    ("*", "/api/projects/*/channels/*/import", "the people on a list are the user's to handle"),
    ("*", "/api/projects/*/channels/*/unsubscribe", "the people on a list are the user's to handle"),
    ("*", "/api/projects/*/credentials/*/hands/**", "only a person takes the controls"),
    ("*", "/api/projects/*/abilities/*/connect", "a sign-in through kiqr.cloud happens in the user's browser"),
    ("POST", "/api/projects/*/abilities/*", "run an ability through its own tools"),
    ("*", "/api/projects/*/chats/*/**", "the chat is where you are"),
    ("POST", "/api/projects/*/chats", "the chat is where you are"),
    ("DELETE", "/api/projects/*/chats/*", "deleting a chat is the user's"),
    ("*", "/api/projects/*/memory/*/keep", "keeping a lesson Seemo learned is the user's check"),
    ("POST", "/api/projects/*/memory", "use remember, in the chat where the user said it"),
    ("*", "/api/projects/*/memory/remember", "use remember"),
    ("*", "/api/abilities/*/programs/system", "system programs need the user's password"),
    ("*", "/api/vault/connect/**", "a sign-in happens in the user's browser"),
    ("*", "/api/signins/**", "a sign-in happens in the user's browser"),
    ("*", "/api/connect/**", "a sign-in happens in the user's browser"),
    ("*", "/api/reset/**", "a reset is the user's"),
    ("*", "/api/reset", "a reset is the user's"),
    ("*", "/api/update/**", "updating KIQR is the user's"),
    ("*", "/api/update", "updating KIQR is the user's"),
    ("*", "/api/telemetry", "telemetry consent is the user's"),
    ("*", "/api/extensions/**", "use the ability routes"),
    ("PUT", "/api/config", "use change_config: it changes one setting at a time"),
];

fn matches(pattern: &str, path: &str) -> bool {
    let p: Vec<&str> = pattern.trim_matches('/').split('/').collect();
    let s: Vec<&str> = path.trim_matches('/').split('/').collect();
    let mut i = 0;
    for (k, seg) in p.iter().enumerate() {
        if *seg == "**" {
            return k == p.len() - 1 && s.len() > i;
        }
        match s.get(i) {
            Some(x) if *seg == "*" || seg == x => i += 1,
            _ => return false,
        }
    }
    i == s.len()
}

/// The card for `change_kiqr`: the request checked, `{slug}` filled in.
pub fn preview(slug: &str, payload: &Value) -> Result<Value, ApiError> {
    let method = payload["method"].as_str().unwrap_or("").trim().to_ascii_uppercase();
    if !["POST", "PUT", "PATCH", "DELETE"].contains(&method.as_str()) {
        return Err(bad("method is POST, PUT, PATCH or DELETE; reading needs no card"));
    }
    let raw = payload["path"].as_str().unwrap_or("").trim().replace("{slug}", slug);
    let (path, query) = raw.split_once('?').map(|(p, q)| (p.to_string(), Some(q.to_string()))).unwrap_or((raw.clone(), None));
    if !path.starts_with("/api/") || path.contains("..") || path.contains("//") {
        return Err(bad("path is a KIQR route, starting /api/ (see kiqr_settings_map)"));
    }
    let summary = payload["summary"].as_str().unwrap_or("").trim().to_string();
    if summary.is_empty() {
        return Err(bad("say in `summary`, in plain words, what this changes"));
    }
    let segs: Vec<&str> = path.trim_matches('/').split('/').collect();
    if segs.get(1) == Some(&"projects") {
        match segs.get(2) {
            Some(s) if *s == slug && segs.len() > 3 => {}
            Some(s) if *s == slug => return Err(bad("the project itself is not changed or deleted from here")),
            _ => return Err(bad("only this project")),
        }
    }
    if let Some(q) = &query {
        if q.split('&').any(|kv| kv.split_once('=').is_some_and(|(k, v)| k == "project" && v != slug)) {
            return Err(bad("only this project"));
        }
    }
    for (m, pat, why) in NEVER {
        if (*m == "*" || *m == method) && matches(pat, &path) {
            return Err(bad(format!("not through change_kiqr: {why}")));
        }
    }
    let body = payload.get("body").cloned().filter(|b| !b.is_null()).unwrap_or(json!({}));
    if !body.is_object() {
        return Err(bad("body is a JSON object"));
    }
    if body.to_string().len() > 200_000 {
        return Err(bad("that body is too large"));
    }
    // A credential's values are never Seemo's: typed by the user in the Vault.
    if path.starts_with("/api/vault") {
        let has_values = body["values"].as_object().is_some_and(|v| v.values().any(|x| x.as_str().is_some_and(|s| !s.is_empty())));
        let has_field_values = body["fields"].as_array().is_some_and(|f| f.iter().any(|x| x.get("value").is_some_and(|v| !v.is_null())));
        if has_values || has_field_values {
            return Err(bad("a credential's values are typed by the user in the Vault; leave values out and tell them where"));
        }
    }
    Ok(json!({ "method": method, "path": raw, "body": body, "summary": summary }))
}

/// After the yes: the request, as Seemo's, against KIQR's own routes.
pub async fn apply(app: &App, preview: &Value) -> Result<Value, ApiError> {
    let method = Method::from_bytes(preview["method"].as_str().unwrap_or("").as_bytes()).map_err(|_| bad("no method"))?;
    let path = preview["path"].as_str().unwrap_or("").to_string();
    let body = preview["body"].clone();
    let req = Request::builder()
        .method(method)
        .uri(&path)
        .header("content-type", "application/json")
        .header(crate::reset::AGENT_HEADER, "seemo")
        .body(Body::from(body.to_string()))
        .map_err(|e| bad(e.to_string()))?;
    let res = crate::api::router(app.clone()).oneshot(req).await.map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1_000_000).await.unwrap_or_default();
    let text = crate::redact(app, &String::from_utf8_lossy(&bytes));
    let said: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!(kiqr_core::profile::truncate_chars(text.trim(), 2000)));
    if !status.is_success() {
        let msg = said["error"].as_str().map(str::to_string).unwrap_or_else(|| format!("{status}: {}", kiqr_core::profile::truncate_chars(&said.to_string(), 400)));
        return Err(ApiError(status, msg));
    }
    let short = if said.to_string().len() > 3000 { json!("done (the answer was long; read the state again to see it)") } else { said };
    Ok(json!({ "done": true, "answer": short, "note": "Done. Read the state again before saying how it is now." }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_never_seemos_is_refused_before_the_card() {
        let ok = |m: &str, p: &str, b: Value| preview("acme", &json!({ "method": m, "path": p, "body": b, "summary": "x" }));
        assert!(ok("POST", "/api/projects/{slug}/access/grant", json!({ "ability": "x-browser", "credential": "c1" })).is_ok());
        assert!(ok("PUT", "/api/projects/acme/memory/m_1", json!({ "text": "t" })).is_ok());
        assert!(ok("POST", "/api/projects/acme/abilities/ga4/pause", json!({})).is_ok());
        assert!(ok("PUT", "/api/vault/c1", json!({ "label": "X login", "domains": ["x.com"] })).is_ok());
        assert!(ok("PUT", "/api/vault/c1", json!({ "values": { "password": "hunter2" } })).is_err(), "values are the user's");
        assert!(ok("POST", "/api/vault", json!({ "label": "x", "fields": [{ "name": "key", "value": "s" }] })).is_err());
        assert!(ok("PUT", "/api/secrets/ANTHROPIC_API_KEY", json!({})).is_err());
        assert!(ok("POST", "/api/projects/other/channels", json!({})).is_err(), "only this project");
        assert!(ok("DELETE", "/api/projects/acme", json!({})).is_err());
        assert!(ok("POST", "/api/projects", json!({})).is_err());
        assert!(ok("POST", "/api/projects/acme/approvals/a1", json!({})).is_err());
        assert!(ok("POST", "/api/projects/acme/channels/news/contacts", json!({})).is_err());
        assert!(ok("POST", "/api/projects/acme/memory/m_1/keep", json!({})).is_err());
        assert!(ok("POST", "/api/projects/acme/abilities/x-browser", json!({})).is_err(), "running an ability is its own tools");
        assert!(ok("POST", "/api/projects/acme/assets/post/p1/publish", json!({})).is_err());
        assert!(ok("POST", "/api/abilities/x/update?project=other", json!({})).is_err());
        assert!(ok("GET", "/api/projects/acme/channels", json!({})).is_err());
        assert!(preview("acme", &json!({ "method": "PUT", "path": "/api/projects/acme/memory/learning", "body": { "on": false } })).is_err(), "a summary is required");
    }
}
