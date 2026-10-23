//! The event bus. Everything that happens in the daemon is an [`Event`];
//! the web UI and `kiqr crawl --watch` subscribe over SSE.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
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
    /// The page limit of the running crawl was changed.
    CrawlLimitChanged { max_pages: usize },
    PageDiscovered { url: String, from: Option<String> },
    PageFetching { url: String, via: String },
    /// `title` is the page's `<title>` or first `<h1>` (may be empty); `words`
    /// is the word count of the extracted Markdown.
    PageFetched { url: String, status: u16, bytes: usize, ms: u64, via: String, title: String, words: usize },
    PageBlocked { url: String, reason: String },
    /// Not read: it redirects to another website (an affiliate or outbound
    /// link), sits under a path that does, or isn't a web page.
    PageSkipped { url: String, reason: String },
    /// A page is waiting for, or being read by, the model. `waiting` is the queue depth.
    PageQueued { url: String, waiting: usize },
    PageParsed { url: String, facts: usize, waiting: usize, preview: Vec<FactPreview> },
    PageFailed { url: String, error: String },
    CrawlFinished { pages: usize, blocked: usize, failed: usize },
    /// Profile synthesis began. `total` is the number of sections that will be
    /// synthesized plus one for the business brief.
    SynthesisStarted { total: usize },
    /// One section (or `"brief"`) was saved. `by` is `seemo` or a person
    /// (`you`/`user`). `items` are up to six short display strings.
    ProfileUpdated {
        section: String,
        by: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        items: Vec<String>,
    },
    /// Synthesis ended, successfully or not. `sections` counts what was saved,
    /// the brief included.
    SynthesisFinished { sections: usize },
    /// Brand assets were collected from the crawl cache and saved.
    /// `images` is how many images the brand holds.
    BrandUpdated { logo: Option<String>, colors: Vec<String>, images: usize },
    /// A background job on the project ended and its slot is free again.
    /// `job` is `crawl` (crawl plus synthesis), `synthesize` or `brief`.
    JobFinished { job: String },
    /// An asset was made (`created`) or changed. `asset_type` is its type id.
    AssetSaved { asset_type: String, id: String, title: String, status: String, created: bool },
    AssetDeleted { asset_type: String, id: String },
    /// An asset type was added or changed.
    AssetTypeSaved { id: String, name: String },
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

/// The most events one crawl log keeps; a crawl of a few hundred pages emits a few thousand.
const MAX_LOG: usize = 50_000;

/// Every event of a project's latest crawl job, so a client that connects
/// mid-crawl (or after it) can replay it before following live events.
#[derive(Clone, Debug, Default)]
pub struct CrawlLog {
    pub events: Vec<Event>,
    /// True while the crawl job (crawl plus synthesis) is still running.
    pub open: bool,
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
    logs: Arc<Mutex<HashMap<String, CrawlLog>>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(4096);
        Self { tx, logs: Default::default() }
    }

    pub fn emit(&self, project: Option<&str>, kind: EventKind) {
        let ev = Event { at: Utc::now(), project: project.map(str::to_string), kind };
        tracing::debug!(?ev, "event");
        // Record and send under one lock, so `subscribe_with_log` sees each event
        // exactly once: either in the log it copies or on the channel.
        let mut logs = self.logs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(log) = project.and_then(|p| logs.get_mut(p)) {
            if log.open && log.events.len() < MAX_LOG {
                log.events.push(ev.clone());
            }
        }
        let _ = self.tx.send(ev);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    /// Start a fresh log for a project's crawl job; everything it emits is recorded.
    pub fn open_log(&self, project: &str) {
        let mut logs = self.logs.lock().unwrap_or_else(|e| e.into_inner());
        logs.insert(project.to_string(), CrawlLog { events: Vec::new(), open: true });
    }

    /// Stop recording and return the finished log (to be kept on disk).
    pub fn close_log(&self, project: &str) -> Option<Vec<Event>> {
        let mut logs = self.logs.lock().unwrap_or_else(|e| e.into_inner());
        let log = logs.get_mut(project)?;
        log.open = false;
        Some(log.events.clone())
    }

    /// Restore a finished log read from disk, unless one is already in memory.
    pub fn restore_log(&self, project: &str, events: Vec<Event>) {
        let mut logs = self.logs.lock().unwrap_or_else(|e| e.into_inner());
        logs.entry(project.to_string()).or_insert(CrawlLog { events, open: false });
    }

    /// Drop a project's log, e.g. when the project is removed.
    pub fn forget_log(&self, project: &str) {
        self.logs.lock().unwrap_or_else(|e| e.into_inner()).remove(project);
    }

    pub fn has_log(&self, project: &str) -> bool {
        self.logs.lock().unwrap_or_else(|e| e.into_inner()).contains_key(project)
    }

    /// The project's crawl log so far plus a receiver for everything after it,
    /// with no gap and no overlap between the two.
    pub fn subscribe_with_log(&self, project: &str) -> (CrawlLog, broadcast::Receiver<Event>) {
        let logs = self.logs.lock().unwrap_or_else(|e| e.into_inner());
        let log = logs.get(project).cloned().unwrap_or_default();
        let rx = self.tx.subscribe();
        (log, rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_replays_without_gap_or_overlap() {
        let bus = EventBus::new();
        bus.emit(Some("a"), EventKind::SeemoSays { text: "before".into() });
        bus.open_log("a");
        bus.emit(Some("a"), EventKind::CrawlStarted { max_pages: 5, concurrency: 1, ai_concurrency: 1 });
        bus.emit(Some("b"), EventKind::SeemoSays { text: "other project".into() });
        let (log, mut rx) = bus.subscribe_with_log("a");
        assert_eq!(log.events.len(), 1);
        assert!(log.open);
        bus.emit(Some("a"), EventKind::CrawlFinished { pages: 1, blocked: 0, failed: 0 });
        assert!(matches!(rx.try_recv().unwrap().kind, EventKind::CrawlFinished { .. }));
        let done = bus.close_log("a").unwrap();
        assert_eq!(done.len(), 2);
        bus.emit(Some("a"), EventKind::SeemoSays { text: "after".into() });
        let (log, _) = bus.subscribe_with_log("a");
        assert_eq!(log.events.len(), 2);
        assert!(!log.open);
    }
}
