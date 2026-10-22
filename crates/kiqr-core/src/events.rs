//! The event bus. Everything that happens in the daemon is an [`Event`];
//! the web UI and `kiqr crawl --watch` subscribe over SSE.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub at: DateTime<Utc>,
    pub project: Option<String>,
    #[serde(flatten)]
    pub kind: EventKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    ProjectCreated { slug: String, url: String },
    CrawlStarted { max_pages: usize, concurrency: usize, ai_concurrency: usize },
    PageDiscovered { url: String, from: Option<String> },
    PageFetching { url: String, via: String },
    PageFetched { url: String, status: u16, bytes: usize, ms: u64, via: String },
    PageBlocked { url: String, reason: String },
    /// A page is waiting for, or being read by, the model. `waiting` is the queue depth.
    PageQueued { url: String, waiting: usize },
    PageParsed { url: String, facts: usize, waiting: usize, preview: Vec<FactPreview> },
    PageFailed { url: String, error: String },
    CrawlFinished { pages: usize, blocked: usize, failed: usize },
    ProfileUpdated { section: String, by: String },
    SeemoSays { text: String },
    Committed { message: String },
}

/// A few of the facts a page produced, for the live feed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FactPreview {
    pub section: String,
    pub field: String,
    pub value: String,
    pub quote: String,
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(4096);
        Self { tx }
    }

    pub fn emit(&self, project: Option<&str>, kind: EventKind) {
        let ev = Event { at: Utc::now(), project: project.map(str::to_string), kind };
        tracing::debug!(?ev, "event");
        let _ = self.tx.send(ev);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}
