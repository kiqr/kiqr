//! The tools Seemo can use. Defined once here; `kiqr mcp` serves them to the
//! model over MCP and maps each call to the daemon's HTTP API, the daemon
//! labels them in the chat, and the UI lists the groups a task may use.
//!
//! Tools are grouped. A chat gets every group; a task run gets the groups the
//! user confirmed when the task was created. Two tools ask the user first
//! (`create_task`, `change_config`): they open an approval and wait for it.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A family of tools, confirmed as one unit for a task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    /// Read the profile, brief, brand, assets, tasks, config and skills.
    Read,
    /// Edit profile fields (locking them) and the brief.
    WriteProfile,
    /// Create and change assets and asset types.
    WriteAssets,
    /// Start crawls, profile builds, brief rewrites and brand rebuilds.
    Jobs,
    /// Create, change, pause, resume, delete and run tasks.
    Tasks,
    /// Change `config.yaml` and project settings.
    Config,
    /// Read pages on the web through KIQR's own fetcher.
    Web,
}

impl Group {
    pub const ALL: [Group; 7] = [Group::Read, Group::Web, Group::WriteProfile, Group::WriteAssets, Group::Jobs, Group::Tasks, Group::Config];
    /// What a task run may use unless its creator narrowed it: everything but
    /// making more tasks and changing settings, which need a person.
    pub const TASK_DEFAULT: [Group; 5] = [Group::Read, Group::Web, Group::WriteProfile, Group::WriteAssets, Group::Jobs];

    pub fn id(self) -> &'static str {
        match self {
            Group::Read => "read",
            Group::WriteProfile => "write_profile",
            Group::WriteAssets => "write_assets",
            Group::Jobs => "jobs",
            Group::Tasks => "tasks",
            Group::Config => "config",
            Group::Web => "web",
        }
    }

    pub fn parse(s: &str) -> Option<Group> {
        Group::ALL.into_iter().find(|g| g.id() == s.trim())
    }

    /// Wording for the confirmation card and the Tasks page.
    pub fn label(self) -> &'static str {
        match self {
            Group::Read => "Read the profile, brief, brand, assets and tasks",
            Group::WriteProfile => "Edit the business profile and brief",
            Group::WriteAssets => "Create and edit assets",
            Group::Jobs => "Start crawls and profile builds",
            Group::Tasks => "Create and change tasks",
            Group::Config => "Change KIQR settings",
            Group::Web => "Read pages on the web",
        }
    }

    /// Parse a comma-separated list, as `KIQR_ALLOWED` carries it.
    pub fn parse_list(s: &str) -> Vec<Group> {
        let mut out: Vec<Group> = Vec::new();
        for g in s.split(',').filter_map(Group::parse) {
            if !out.contains(&g) {
                out.push(g);
            }
        }
        out
    }

    pub fn join(groups: &[Group]) -> String {
        groups.iter().map(|g| g.id()).collect::<Vec<_>>().join(",")
    }
}

/// How `kiqr mcp` performs a tool: one request to the daemon, or an approval
/// that waits for the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    /// `path` may contain `{slug}` and `{arg}` placeholders filled from the
    /// project and the tool's arguments; `body` names the arguments sent as
    /// JSON (all of them when `None` and the method has a body).
    Http { method: &'static str, path: &'static str, body: Body },
    /// Open an approval of this kind in the chat and wait for the decision.
    Approval(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    None,
    /// Every argument not used in the path.
    Args,
    /// The arguments go in the query string.
    Query,
}

pub struct Tool {
    pub name: &'static str,
    pub group: Group,
    /// Present-tense wording for the chat ("Reading the business brief").
    pub label: &'static str,
    pub description: &'static str,
    pub call: Call,
    /// Builds the JSON schema of the arguments.
    pub schema: fn() -> Value,
}

/// The MCP server name; tools appear to the model as `mcp__kiqr__<name>`.
pub const SERVER: &str = "kiqr";

fn no_args() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
}

fn asset_ref() -> Value {
    json!({
        "type": { "type": "string", "description": "Asset type id, e.g. post, article" },
        "id": { "type": "string", "description": "Asset id (the folder name)" },
    })
}

/// Every tool, in the order the model sees them.
pub fn catalogue() -> Vec<Tool> {
    use Group::*;
    vec![
        // ------------------------------------------------------------ read
        Tool {
            name: "get_project",
            group: Read,
            label: "Checking the project",
            description: "The project: name, website URL, crawl settings, when it was last crawled and which jobs run now.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}", body: Body::None },
            schema: no_args,
        },
        Tool {
            name: "get_profile",
            group: Read,
            label: "Reading the business profile",
            description: "The whole business profile as Markdown: brief, brand, and every section with its fields, evidence quotes and source URLs. Long; prefer get_brief or get_section unless you need the evidence.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/profile.md", body: Body::None },
            schema: no_args,
        },
        Tool {
            name: "get_brief",
            group: Read,
            label: "Reading the business brief",
            description: "The business brief: summary, language, geographies, offerings, audiences, tone of voice, brand terms, differentiators, competitors, pricing, topic clusters and content gaps.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/brief", body: Body::None },
            schema: no_args,
        },
        Tool {
            name: "get_brand",
            group: Read,
            label: "Looking at the brand",
            description: "Brand assets collected from the website: name, tagline, logo, icon, cover, colours, fonts, images and social profiles.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/brand", body: Body::None },
            schema: no_args,
        },
        Tool {
            name: "get_section",
            group: Read,
            label: "Reading a profile section",
            description: "One profile section with its fields, evidence and locks. Sections: company, offerings, pricing, audience, positioning, voice, proof, team, contact, legal, channels, seo.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/profile/{section}", body: Body::None },
            schema: || obj(json!({ "section": { "type": "string" } }), &["section"]),
        },
        Tool {
            name: "list_assets",
            group: Read,
            label: "Looking through the library",
            description: "Assets in the library (title, type, status, tags, who made them), optionally filtered by type, status (draft, ready, scheduled, published, archived) or tag.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/assets", body: Body::Query },
            schema: || obj(json!({ "type": { "type": "string" }, "status": { "type": "string" }, "tag": { "type": "string" } }), &[]),
        },
        Tool {
            name: "get_asset",
            group: Read,
            label: "Opening an asset",
            description: "One asset's metadata and file list. Read its text with read_asset_file.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/assets/{type}/{id}", body: Body::None },
            schema: || obj(asset_ref(), &["type", "id"]),
        },
        Tool {
            name: "read_asset_file",
            group: Read,
            label: "Reading an asset's text",
            description: "The text of one of an asset's files, e.g. post.md.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/assets/{type}/{id}/files/{name}", body: Body::None },
            schema: || {
                let mut p = asset_ref();
                p["name"] = json!({ "type": "string", "description": "File name as listed by get_asset" });
                obj(p, &["type", "id", "name"])
            },
        },
        Tool {
            name: "list_asset_types",
            group: Read,
            label: "Checking the asset types",
            description: "The kinds of asset the library knows: id, name, icon, the files each holds and how it is previewed.",
            call: Call::Http { method: "GET", path: "/api/asset-types", body: Body::None },
            schema: no_args,
        },
        Tool {
            name: "list_tasks",
            group: Read,
            label: "Checking the tasks",
            description: "The project's scheduled tasks with their schedule, status, next and last run.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/tasks", body: Body::None },
            schema: no_args,
        },
        Tool {
            name: "get_task",
            group: Read,
            label: "Checking a task",
            description: "One task.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/tasks/{task}", body: Body::None },
            schema: || obj(json!({ "task": { "type": "string" } }), &["task"]),
        },
        Tool {
            name: "list_runs",
            group: Read,
            label: "Reading a task's runs",
            description: "The runs of a task, newest first: when, status, summary, the tools used and the assets made.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/tasks/{task}/runs", body: Body::None },
            schema: || obj(json!({ "task": { "type": "string" } }), &["task"]),
        },
        Tool {
            name: "get_run",
            group: Read,
            label: "Reading a task run",
            description: "One run of a task in full.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/tasks/{task}/runs/{run}", body: Body::None },
            schema: || obj(json!({ "task": { "type": "string" }, "run": { "type": "string" } }), &["task", "run"]),
        },
        Tool {
            name: "get_config",
            group: Read,
            label: "Reading KIQR's settings",
            description: "config.yaml: daemon host and port, crawl defaults (max_pages, concurrency, ai_concurrency, escalate_to_unblockingapi), the model per task (extract, synthesize, chat) and agent settings. Never contains secrets.",
            call: Call::Http { method: "GET", path: "/api/config", body: Body::None },
            schema: no_args,
        },
        Tool {
            name: "read_skill",
            group: Read,
            label: "Reading a skill",
            description: "The text of a skill, i.e. the standing instructions for a kind of work. Skills: social-posts (how to write posts), business-brief, build-profile, extract-facts, crawl-site, seemo-agent.",
            call: Call::Http { method: "GET", path: "/api/skills/{name}", body: Body::None },
            schema: || obj(json!({ "name": { "type": "string" } }), &["name"]),
        },
        Tool {
            name: "list_chats",
            group: Read,
            label: "Looking at earlier chats",
            description: "Earlier conversations in this project: title, when, and the last message.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/chats", body: Body::None },
            schema: no_args,
        },
        Tool {
            name: "get_history",
            group: Read,
            label: "Reading the change history",
            description: "The project's change history (git log): what changed and when.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/history", body: Body::None },
            schema: no_args,
        },
        // ------------------------------------------------------------- web
        Tool {
            name: "read_url",
            group: Web,
            label: "Reading a web page",
            description: "Read one web page as Markdown through KIQR's own fetcher (a real browser and proxies take over when a site blocks plain requests). For competitor pages, articles, sources to quote, anything the user links. Long pages are cut at `max_chars` (default 20000); `obstacle` is set when the page could not be read properly. There is no web search: you need a URL.",
            call: Call::Http { method: "GET", path: "/api/projects/{slug}/web", body: Body::Query },
            schema: || obj(json!({
                "url": { "type": "string", "description": "Full http(s) URL" },
                "max_chars": { "type": "integer", "description": "Cut the Markdown after this many characters (default 20000, max 100000)" },
            }), &["url"]),
        },
        // --------------------------------------------------- write_profile
        Tool {
            name: "update_section_field",
            group: WriteProfile,
            label: "Updating the business profile",
            description: "Set one field of a profile section to a value the user gave you. The field is locked afterwards (it is ground truth, so the next crawl keeps it). Only for facts the user stated.",
            call: Call::Http { method: "PUT", path: "/api/projects/{slug}/profile/{section}/fields/{field}", body: Body::Args },
            schema: || obj(json!({ "section": { "type": "string" }, "field": { "type": "string", "description": "Field name, e.g. tagline" }, "value": { "type": "string" } }), &["section", "field", "value"]),
        },
        Tool {
            name: "update_brief",
            group: WriteProfile,
            label: "Updating the business brief",
            description: "Change parts of the business brief. Pass only the top-level keys to replace (summary, offerings, audiences, tone_of_voice, brand_terms, differentiators, competitors, pricing_summary, content_gaps…); the rest stays. The brief is marked as edited.",
            call: Call::Http { method: "PATCH", path: "/api/projects/{slug}/brief", body: Body::Args },
            schema: || obj(json!({ "changes": { "type": "object", "description": "Brief keys and their new values", "additionalProperties": true } }), &["changes"]),
        },
        // ---------------------------------------------------- write_assets
        Tool {
            name: "create_asset",
            group: WriteAssets,
            label: "Adding to the library",
            description: "Save a new asset in the library as a draft: a post, an article, a reply, an email… `text` becomes its body file (Markdown). Use `summary` for a one-line description (for a post: the network, e.g. 'For LinkedIn') and `tags` for the network or topic.",
            call: Call::Http { method: "POST", path: "/api/projects/{slug}/assets", body: Body::Args },
            schema: || obj(
                json!({
                    "type": { "type": "string", "description": "Asset type id: post, article, reply, email…" },
                    "title": { "type": "string", "description": "Short internal label, 3 to 7 words" },
                    "text": { "type": "string", "description": "The content, exactly as it would be published" },
                    "summary": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "status": { "type": "string", "enum": ["draft", "ready"] },
                }),
                &["type", "title", "text"],
            ),
        },
        Tool {
            name: "update_asset",
            group: WriteAssets,
            label: "Changing an asset",
            description: "Change an asset's title, status (draft, ready, archived), tags, summary or text. Pass only what changes.",
            call: Call::Http { method: "PATCH", path: "/api/projects/{slug}/assets/{type}/{id}", body: Body::Args },
            schema: || {
                let mut p = asset_ref();
                p["title"] = json!({ "type": "string" });
                p["status"] = json!({ "type": "string", "enum": ["draft", "ready", "archived"] });
                p["tags"] = json!({ "type": "array", "items": { "type": "string" } });
                p["summary"] = json!({ "type": "string" });
                p["text"] = json!({ "type": "string", "description": "New body text" });
                obj(p, &["type", "id"])
            },
        },
        Tool {
            name: "add_asset_type",
            group: WriteAssets,
            label: "Adding an asset type",
            description: "Define a new kind of asset for the library when none fits (e.g. 'newsletter'). Icons are lucide names; preview is article, post, image, video, audio, campaign or files.",
            call: Call::Http { method: "POST", path: "/api/asset-types", body: Body::Args },
            schema: || obj(
                json!({
                    "id": { "type": "string", "description": "kebab-case id" },
                    "name": { "type": "string" },
                    "plural": { "type": "string" },
                    "icon": { "type": "string" },
                    "description": { "type": "string" },
                    "preview": { "type": "string", "enum": ["article", "post", "image", "video", "audio", "campaign", "files"] },
                }),
                &["id", "name", "plural"],
            ),
        },
        // ------------------------------------------------------------ jobs
        Tool {
            name: "start_crawl",
            group: Jobs,
            label: "Starting a crawl",
            description: "Crawl the website again and rebuild the profile afterwards. Runs in the background; progress arrives as events. Fails with 409 if a crawl is already running.",
            call: Call::Http { method: "POST", path: "/api/projects/{slug}/crawl", body: Body::Query },
            schema: || obj(json!({ "max_pages": { "type": "integer", "minimum": 1 } }), &[]),
        },
        Tool {
            name: "start_synthesis",
            group: Jobs,
            label: "Rebuilding the profile",
            description: "Rebuild the profile sections and the brief from the facts already extracted, without crawling. Background job.",
            call: Call::Http { method: "POST", path: "/api/projects/{slug}/synthesize", body: Body::None },
            schema: no_args,
        },
        Tool {
            name: "rewrite_brief",
            group: Jobs,
            label: "Rewriting the brief",
            description: "Rewrite only the business brief from the current sections. Background job.",
            call: Call::Http { method: "POST", path: "/api/projects/{slug}/brief", body: Body::None },
            schema: no_args,
        },
        Tool {
            name: "rebuild_brand",
            group: Jobs,
            label: "Rebuilding the brand",
            description: "Collect the brand assets again from the crawled pages.",
            call: Call::Http { method: "POST", path: "/api/projects/{slug}/brand", body: Body::None },
            schema: no_args,
        },
        // ----------------------------------------------------------- tasks
        Tool {
            name: "create_task",
            group: Tasks,
            label: "Proposing a task",
            description: "Create a task: an instruction you will carry out later, once or on a schedule, as a session of your own with the allowed tools. The user sees a card and confirms before it exists; this call waits for that answer. A one-off task runs right away once confirmed. Schedule: {kind: 'once', at: RFC 3339} or {kind: 'rrule', rrule: 'FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR=10;BYMINUTE=0', tz: 'Europe/Stockholm'} plus `text`, the schedule in the user's words.",
            call: Call::Approval("task"),
            schema: || obj(
                json!({
                    "title": { "type": "string", "description": "3 to 7 words" },
                    "instruction": { "type": "string", "description": "What to do on each run, written to yourself, complete enough to act on without this conversation" },
                    "schedule": {
                        "type": "object",
                        "properties": {
                            "kind": { "type": "string", "enum": ["once", "rrule"] },
                            "at": { "type": "string", "description": "For once: RFC 3339 time, or omit to run now" },
                            "rrule": { "type": "string", "description": "For rrule: the RRULE value without DTSTART" },
                            "tz": { "type": "string", "description": "IANA timezone the rule is in" },
                            "text": { "type": "string", "description": "The schedule in words, e.g. 'every weekday at 10:00'" },
                        },
                        "required": ["kind"],
                    },
                    "allowed": { "type": "array", "items": { "type": "string", "enum": ["read", "write_profile", "write_assets", "jobs"] }, "description": "Tool groups the runs may use; default read, write_profile, write_assets, jobs" },
                    "follow_up": { "type": "boolean", "description": "Tell this chat when the first run finishes (default true for one-off tasks)" },
                }),
                &["title", "instruction", "schedule"],
            ),
        },
        Tool {
            name: "update_task",
            group: Tasks,
            label: "Changing a task",
            description: "Change a task's title, instruction or schedule. Pass only what changes.",
            call: Call::Http { method: "PATCH", path: "/api/projects/{slug}/tasks/{task}", body: Body::Args },
            schema: || obj(
                json!({
                    "task": { "type": "string" },
                    "title": { "type": "string" },
                    "instruction": { "type": "string" },
                    "schedule": { "type": "object", "additionalProperties": true },
                }),
                &["task"],
            ),
        },
        Tool {
            name: "pause_task",
            group: Tasks,
            label: "Pausing a task",
            description: "Pause a task; it keeps its schedule but does not run.",
            call: Call::Http { method: "POST", path: "/api/projects/{slug}/tasks/{task}/pause", body: Body::None },
            schema: || obj(json!({ "task": { "type": "string" } }), &["task"]),
        },
        Tool {
            name: "resume_task",
            group: Tasks,
            label: "Resuming a task",
            description: "Resume a paused task.",
            call: Call::Http { method: "POST", path: "/api/projects/{slug}/tasks/{task}/resume", body: Body::None },
            schema: || obj(json!({ "task": { "type": "string" } }), &["task"]),
        },
        Tool {
            name: "delete_task",
            group: Tasks,
            label: "Removing a task",
            description: "Delete a task and its run records.",
            call: Call::Http { method: "DELETE", path: "/api/projects/{slug}/tasks/{task}", body: Body::None },
            schema: || obj(json!({ "task": { "type": "string" } }), &["task"]),
        },
        Tool {
            name: "run_task_now",
            group: Tasks,
            label: "Running a task now",
            description: "Run a task right away, outside its schedule.",
            call: Call::Http { method: "POST", path: "/api/projects/{slug}/tasks/{task}/run", body: Body::None },
            schema: || obj(json!({ "task": { "type": "string" } }), &["task"]),
        },
        // ---------------------------------------------------------- config
        Tool {
            name: "change_config",
            group: Config,
            label: "Proposing a settings change",
            description: "Change KIQR settings (config.yaml). Keys are dotted paths: models.extract, models.synthesize, models.chat (provider/model, e.g. claude-cli/sonnet, anthropic/claude-haiku-5-5), crawl.max_pages, crawl.concurrency (1-10), crawl.ai_concurrency (1-10), crawl.escalate_to_unblockingapi, daemon.host, daemon.port, agent.max_budget_usd, agent.max_concurrent, agent.timezone. The user sees the change and confirms; this call waits for that answer. Applied at once, except host and port, which need a restart. Secrets cannot be set here; they go in .env via Settings.",
            call: Call::Approval("config"),
            schema: || obj(
                json!({
                    "changes": { "type": "object", "description": "Dotted key → new value", "additionalProperties": true },
                    "reason": { "type": "string", "description": "One sentence on why" },
                }),
                &["changes"],
            ),
        },
        Tool {
            name: "change_project_settings",
            group: Config,
            label: "Changing the project's crawl settings",
            description: "Set this project's own crawl settings: max_pages, concurrency (1-10), ai_concurrency (1-10). Null clears a value back to the global default.",
            call: Call::Http { method: "PUT", path: "/api/projects/{slug}/settings", body: Body::Args },
            schema: || obj(
                json!({
                    "max_pages": { "type": ["integer", "null"] },
                    "concurrency": { "type": ["integer", "null"] },
                    "ai_concurrency": { "type": ["integer", "null"] },
                }),
                &[],
            ),
        },
    ]
}

pub fn find(name: &str) -> Option<Tool> {
    let name = name.strip_prefix("mcp__kiqr__").unwrap_or(name);
    catalogue().into_iter().find(|t| t.name == name)
}

/// The tools in these groups, as the model may see them.
pub fn allowed(groups: &[Group]) -> Vec<Tool> {
    catalogue().into_iter().filter(|t| groups.contains(&t.group)).collect()
}

/// The chat wording for a tool call, with the argument that identifies the
/// target when there is an obvious one ("Reading a profile section: pricing").
pub fn label_for(name: &str, args: &Value) -> String {
    let Some(t) = find(name) else { return format!("Using {name}") };
    let target = ["section", "name", "title", "task", "type", "url"].iter().find_map(|k| args.get(*k).and_then(|v| v.as_str()).filter(|s| !s.is_empty()));
    match target {
        Some(x) if t.name != "create_task" && t.name != "create_asset" => format!("{}: {x}", t.label),
        _ => t.label.to_string(),
    }
}

/// An MCP `tools/list` entry.
pub fn mcp_entry(t: &Tool) -> Value {
    json!({ "name": t.name, "description": t.description, "inputSchema": (t.schema)() })
}

/// The tool groups and their labels, for the UI.
pub fn groups_json() -> Value {
    json!(Group::ALL.iter().map(|g| json!({ "id": g.id(), "label": g.label(), "tools": catalogue().iter().filter(|t| t.group == *g).map(|t| t.name).collect::<Vec<_>>() })).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_schemas_build() {
        let all = catalogue();
        let mut names: Vec<&str> = all.iter().map(|t| t.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all.len());
        for t in &all {
            let s = (t.schema)();
            assert_eq!(s["type"], "object", "{}", t.name);
        }
    }

    #[test]
    fn groups_round_trip() {
        let g = Group::parse_list("read, write_assets,bogus,read");
        assert_eq!(g, vec![Group::Read, Group::WriteAssets]);
        assert_eq!(Group::join(&g), "read,write_assets");
        assert!(allowed(&[Group::Read]).iter().all(|t| t.group == Group::Read));
        assert!(find("mcp__kiqr__get_brief").is_some());
    }

    #[test]
    fn labels_name_the_target() {
        assert_eq!(label_for("get_section", &json!({ "section": "pricing" })), "Reading a profile section: pricing");
        assert_eq!(label_for("create_asset", &json!({ "type": "post", "title": "x" })), "Adding to the library");
        assert_eq!(label_for("nope", &json!({})), "Using nope");
    }
}
