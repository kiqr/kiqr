//! A project's first build: crawl, read, write the profile and the brief.
//! Until it is done the project is not ready, and the UI keeps the user in
//! the welcome flow while plans, posts and tasks wait (`require_ready`).
//!
//! The state is folded from the events the crawl job emits, so it needs no
//! hooks in the crawler: one overall percentage, the stage in plain words and
//! an estimate of the time left. `build_progress` carries it to the UI at most
//! once a second, `build_stage` on every change of stage. A project whose
//! brief exists is ready for good; crawling it again does not lock it.

use crate::api::ApiError;
use crate::App;
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use kiqr_core::events::{Event, EventKind};
use kiqr_core::profile::Brief;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Shares of the overall percentage: finding and fetching pages, the model reading them, writing the profile.
const FETCH: f64 = 30.0;
const READ: f64 = 35.0;
const WRITE: f64 = 35.0;
/// Before anything has been timed: seconds the model takes to read a page, and to write one profile section.
const READ_SECS: f64 = 4.0;
const SECTION_SECS: f64 = 9.0;
/// Sections a profile usually has (the brief included), until synthesis says.
const SECTIONS: f64 = 10.0;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Never crawled.
    New,
    /// Fetching pages.
    Crawling,
    /// Every page fetched; the model is still reading.
    Reading,
    /// Writing the profile sections and the brief.
    Writing,
    Ready,
    /// The build ended without a brief; `problem` says why.
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct BuildState {
    pub stage: Stage,
    /// 0–100 over the whole build; never goes back.
    pub pct: u8,
    /// Seconds left, once there is enough to estimate from.
    pub eta_secs: Option<u64>,
    pub max_pages: usize,
    pub found: usize,
    pub fetched: usize,
    pub queued: usize,
    pub read: usize,
    pub facts: usize,
    pub sections_done: usize,
    pub sections_total: usize,
    /// Whether a build is running now.
    pub running: bool,
    pub started_at: Option<DateTime<Utc>>,
    /// The last event, so the UI can tell slow from stuck.
    pub last_at: Option<DateTime<Utc>>,
    pub problem: Option<String>,
    #[serde(skip)]
    seen: HashSet<String>,
    #[serde(skip)]
    queued_urls: HashSet<String>,
    #[serde(skip)]
    extractor: bool,
    #[serde(skip)]
    sent_at: Option<DateTime<Utc>>,
    /// When the first page was queued for the model and the last one read, for its pace.
    #[serde(skip)]
    read_from: Option<DateTime<Utc>>,
    #[serde(skip)]
    read_last: Option<DateTime<Utc>>,
    /// When writing began and the last section was saved.
    #[serde(skip)]
    write_from: Option<DateTime<Utc>>,
    #[serde(skip)]
    write_last: Option<DateTime<Utc>>,
}

impl BuildState {
    fn new(max_pages: usize, at: DateTime<Utc>) -> Self {
        Self {
            stage: Stage::Crawling,
            pct: 0,
            eta_secs: None,
            max_pages: max_pages.max(1),
            found: 0,
            fetched: 0,
            queued: 0,
            read: 0,
            facts: 0,
            sections_done: 0,
            sections_total: 0,
            running: true,
            started_at: Some(at),
            last_at: Some(at),
            problem: None,
            seen: HashSet::new(),
            queued_urls: HashSet::new(),
            extractor: false,
            sent_at: None,
            read_from: None,
            read_last: None,
            write_from: None,
            write_last: None,
        }
    }

    fn settled(stage: Stage) -> Self {
        let mut s = Self::new(1, Utc::now());
        s.stage = stage;
        s.running = false;
        s.started_at = None;
        s.last_at = None;
        if s.stage == Stage::Ready {
            s.pct = 100;
        }
        s
    }

    /// Fold one event in. Returns whether the stage changed.
    fn apply(&mut self, ev: &EventKind, at: DateTime<Utc>, brief_exists: impl Fn() -> bool) -> bool {
        let before = self.stage.clone();
        self.last_at = Some(at);
        match ev {
            EventKind::CrawlLimitChanged { max_pages } => self.max_pages = (*max_pages).max(1),
            EventKind::PageDiscovered { url, .. } => {
                if self.seen.insert(url.clone()) {
                    self.found += 1;
                }
            }
            EventKind::PageFetched { .. } | EventKind::PageBlocked { .. } | EventKind::PageSkipped { .. } => self.fetched += 1,
            EventKind::PageQueued { url, .. } => {
                self.extractor = true;
                self.read_from.get_or_insert(at);
                if self.queued_urls.insert(url.clone()) {
                    self.queued += 1;
                }
            }
            EventKind::PageParsed { facts, .. } => {
                self.read += 1;
                self.read_last = Some(at);
                self.facts += facts;
            }
            EventKind::PageFailed { url, .. } => {
                if self.queued_urls.contains(url) {
                    self.read += 1;
                }
            }
            // Not one page could be read: the site is down or the address is wrong. Nothing to write from.
            EventKind::CrawlFinished { pages: 0, .. } => {
                self.stage = Stage::Failed;
                self.problem.get_or_insert_with(|| "I couldn't open a single page of the site. Check the address, and that the site is up.".into());
            }
            EventKind::CrawlFinished { .. } => {
                self.fetched = self.fetched.max(self.target());
                self.read = self.read.max(self.queued);
                self.stage = Stage::Writing;
            }
            EventKind::SynthesisStarted { total } => {
                self.sections_total = *total;
                self.sections_done = 0;
                self.write_from = Some(at);
                self.stage = Stage::Writing;
            }
            EventKind::ProfileUpdated { by, .. } if by == "seemo" && self.stage == Stage::Writing => {
                self.sections_done = (self.sections_done + 1).min(self.sections_total.max(1));
                self.write_last = Some(at);
            }
            EventKind::SynthesisFinished { .. } => {
                if brief_exists() {
                    self.stage = Stage::Ready;
                }
            }
            EventKind::JobFinished { job } if job == "crawl" || job == "synthesize" || job == "brief" => {
                self.running = false;
                if brief_exists() {
                    self.stage = Stage::Ready;
                } else if self.stage != Stage::Ready {
                    self.stage = Stage::Failed;
                    if self.problem.is_none() {
                        self.problem = Some(if self.extractor || self.fetched == 0 { "The build ended before the business profile was written.".into() } else { "No model could read the pages, so there is no profile yet.".into() });
                    }
                }
            }
            _ => {}
        }
        if matches!(self.stage, Stage::Crawling | Stage::Reading) {
            self.stage = if self.fetched >= self.target() { Stage::Reading } else { Stage::Crawling };
        }
        self.update_pct(at);
        self.stage != before
    }

    /// The pages this crawl will fetch: what it found, up to the limit.
    fn target(&self) -> usize {
        self.found.clamp(1, self.max_pages)
    }

    fn update_pct(&mut self, at: DateTime<Utc>) {
        let pct = match self.stage {
            Stage::Ready => 100.0,
            Stage::New | Stage::Failed => self.pct as f64,
            _ => {
                let fetch = (self.fetched as f64 / self.target() as f64).min(1.0);
                // Reading can't be ahead of fetching; before anything is queued it is as far as fetching.
                let read = if self.queued == 0 { 0.0 } else { (self.read as f64 / self.queued as f64).min(1.0) * fetch };
                let write = if self.stage == Stage::Writing && self.sections_total > 0 { self.sections_done as f64 / self.sections_total as f64 } else { 0.0 };
                let read = if self.stage == Stage::Writing { 1.0 } else { read };
                // Writing is never 100% until the build says it is ready.
                (FETCH * fetch + READ * read + WRITE * write).min(99.0)
            }
        };
        self.pct = self.pct.max(pct.round() as u8);
        self.eta_secs = self.eta(at);
    }

    /// Seconds left: each stage at its own pace (fetching, the model reading, writing sections),
    /// measured once there is something to measure. Rounded up to tens of seconds.
    fn eta(&self, at: DateTime<Utc>) -> Option<u64> {
        let start = self.started_at?;
        if !self.running || matches!(self.stage, Stage::Ready | Stage::Failed | Stage::New) || (at - start).num_seconds() < 3 {
            return None;
        }
        let secs = |a: DateTime<Utc>, b: DateTime<Utc>| ((b - a).num_milliseconds().max(0) as f64) / 1000.0;
        let pace = |from: Option<DateTime<Utc>>, last: Option<DateTime<Utc>>, done: usize, default: f64| match (from, last) {
            (Some(f), Some(l)) if done > 0 => (secs(f, l) / done as f64).max(0.5),
            _ => default,
        };
        let sections = if self.sections_total > 0 { self.sections_total as f64 } else { SECTIONS };
        let write_each = pace(self.write_from, self.write_last, self.sections_done, SECTION_SECS);
        let left = match self.stage {
            Stage::Writing => (sections - self.sections_done as f64).max(0.5) * write_each,
            _ => {
                // Pages still to fetch at the pace so far, then reading what is left (the model reads several at once,
                // which the measured pace already includes), then the whole profile.
                let fetch_left = if self.fetched < self.target() && self.fetched > 0 { secs(start, at) / self.fetched as f64 * (self.target() - self.fetched) as f64 } else { 0.0 };
                let to_read = self.target().saturating_sub(self.read) as f64;
                let read_left = to_read * pace(self.read_from, self.read_last, self.read, READ_SECS);
                fetch_left.max(read_left) + sections * write_each
            }
        };
        Some(((left / 10.0).ceil() * 10.0).max(10.0) as u64)
    }
}

pub type Builds = std::sync::Arc<tokio::sync::RwLock<HashMap<String, BuildState>>>;

/// The project's build: the live one, or what the files say (a brief means ready).
pub async fn state(app: &App, slug: &str) -> BuildState {
    if let Some(s) = app.builds.read().await.get(slug) {
        if s.stage != Stage::Ready || s.running {
            return s.clone();
        }
    }
    if brief_exists(app, slug) {
        let mut s = BuildState::settled(Stage::Ready);
        if let Some(live) = app.builds.read().await.get(slug) {
            s.running = live.running;
        }
        return s;
    }
    let last = kiqr_core::project::Project::load(&app.store, slug).ok().and_then(|p| p.last_crawl);
    let mut s = BuildState::settled(if last.is_some() { Stage::Failed } else { Stage::New });
    s.problem = last.map(|c| {
        if c.pages == 0 { "I couldn't open a single page of the site. Check the address, and that the site is up.".into() } else { "The site was read, but the business profile was never written.".into() }
    });
    s
}

fn brief_exists(app: &App, slug: &str) -> bool {
    matches!(Brief::load(&app.store, slug), Ok(Some(_)))
}

/// Refuse what needs the profile while the first build is still running.
pub async fn require_ready(app: &App, slug: &str, what: &str) -> Result<(), ApiError> {
    let s = state(app, slug).await;
    if s.stage == Stage::Ready {
        return Ok(());
    }
    let when = match s.eta_secs {
        Some(secs) if secs >= 90 => format!(" (about {} minutes)", secs.div_ceil(60)),
        Some(_) => " (about a minute)".into(),
        None => String::new(),
    };
    Err(ApiError(StatusCode::CONFLICT, format!("Seemo is still learning this business{when}; {what} once the business profile is ready.")))
}

/// A reason the build will not finish, said where the build is shown. The first reason stands:
/// later ones are usually its consequences.
pub async fn problem(app: &App, slug: &str, text: impl Into<String>) {
    if let Some(s) = app.builds.write().await.get_mut(slug) {
        s.problem.get_or_insert_with(|| text.into());
    }
}

/// Folds every project's crawl events into its build state and passes the progress on.
pub async fn run(app: App) {
    let mut rx = app.bus.subscribe();
    loop {
        let ev: Event = match rx.recv().await {
            Ok(ev) => ev,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => return,
        };
        let Some(slug) = ev.project.clone() else { continue };
        if matches!(ev.kind, EventKind::BuildProgress { .. } | EventKind::BuildStage { .. }) {
            continue;
        }
        let mut emit: Option<(BuildState, bool)> = None;
        {
            let mut builds = app.builds.write().await;
            if let EventKind::CrawlStarted { max_pages, .. } = &ev.kind {
                // Crawling a project that has a brief again does not lock it: it stays ready while it runs.
                let mut s = BuildState::new(*max_pages, ev.at);
                if brief_exists(&app, &slug) {
                    s.stage = Stage::Ready;
                    s.pct = 100;
                }
                builds.insert(slug.clone(), s);
            }
            // The profile written again on its own ("Build the profile" after a build that ended without one).
            if let EventKind::SynthesisStarted { .. } = &ev.kind {
                if !builds.contains_key(&slug) && !brief_exists(&app, &slug) {
                    let mut s = BuildState::new(1, ev.at);
                    s.stage = Stage::Writing;
                    s.pct = (FETCH + READ) as u8;
                    builds.insert(slug.clone(), s);
                }
            }
            let Some(s) = builds.get_mut(&slug) else { continue };
            if s.stage == Stage::Ready && !matches!(ev.kind, EventKind::JobFinished { .. }) {
                continue;
            }
            let changed = s.apply(&ev.kind, ev.at, || brief_exists(&app, &slug));
            let due = s.sent_at.is_none_or(|t| ev.at - t >= chrono::Duration::milliseconds(1000));
            if changed || due {
                s.sent_at = Some(ev.at);
                emit = Some((s.clone(), changed));
            }
        }
        if let Some((s, changed)) = emit {
            if changed {
                app.bus.emit(Some(&slug), EventKind::BuildStage { stage: stage_id(&s.stage).into(), problem: s.problem.clone() });
            }
            app.bus.emit(Some(&slug), EventKind::BuildProgress { pct: s.pct, stage: stage_id(&s.stage).into(), eta_secs: s.eta_secs });
        }
    }
}

pub fn stage_id(s: &Stage) -> &'static str {
    match s {
        Stage::New => "new",
        Stage::Crawling => "crawling",
        Stage::Reading => "reading",
        Stage::Writing => "writing",
        Stage::Ready => "ready",
        Stage::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiqr_core::events::FactPreview;

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000 + secs, 0).unwrap()
    }

    fn page(n: usize) -> String {
        format!("https://acme.test/{n}")
    }

    /// A crawl of three pages, each read by the model, then the profile in three sections and the brief.
    fn normal() -> Vec<(i64, EventKind)> {
        let mut evs = vec![];
        for n in 0..3 {
            evs.push((1, EventKind::PageDiscovered { url: page(n), from: None }));
        }
        for n in 0..3 {
            let t = 2 + n as i64 * 4;
            evs.push((t, EventKind::PageFetched { url: page(n), status: 200, bytes: 1, ms: 1, via: "local".into(), title: String::new(), words: 10 }));
            evs.push((t, EventKind::PageQueued { url: page(n), waiting: 1 }));
            evs.push((t + 3, EventKind::PageParsed { url: page(n), facts: 2, waiting: 0, preview: Vec::<FactPreview>::new() }));
        }
        evs.push((20, EventKind::CrawlFinished { pages: 3, blocked: 0, failed: 0 }));
        evs.push((21, EventKind::SynthesisStarted { total: 4 }));
        for (i, s) in ["offer", "audience", "voice", "brief"].iter().enumerate() {
            evs.push((25 + i as i64 * 5, EventKind::ProfileUpdated { section: s.to_string(), by: "seemo".into(), title: String::new(), items: vec![] }));
        }
        evs.push((45, EventKind::SynthesisFinished { sections: 4 }));
        evs.push((46, EventKind::JobFinished { job: "crawl".into() }));
        evs
    }

    fn play(evs: &[(i64, EventKind)], brief_at: Option<i64>) -> Vec<BuildState> {
        let mut s = BuildState::new(200, at(0));
        evs.iter()
            .map(|(t, ev)| {
                s.apply(ev, at(*t), || brief_at.is_some_and(|b| *t >= b));
                s.clone()
            })
            .collect()
    }

    #[test]
    fn a_normal_build_moves_forward_and_ends_ready() {
        let states = play(&normal(), Some(40));
        let pcts: Vec<u8> = states.iter().map(|s| s.pct).collect();
        assert!(pcts.windows(2).all(|w| w[0] <= w[1]), "the percentage never goes back: {pcts:?}");
        let stages: Vec<&Stage> = states.iter().map(|s| &s.stage).collect();
        assert!(stages.contains(&&Stage::Crawling) && stages.contains(&&Stage::Reading) && stages.contains(&&Stage::Writing));
        let last = states.last().unwrap();
        assert_eq!((last.stage.clone(), last.pct, last.running), (Stage::Ready, 100, false));
        assert!(states.iter().any(|s| s.eta_secs.is_some()), "an estimate appears once there is enough to go on");
        assert!(states.iter().filter(|s| s.stage != Stage::Ready).all(|s| s.pct < 100));
    }

    #[test]
    fn not_ready_without_a_brief() {
        let last = play(&normal(), None).pop().unwrap();
        assert_eq!(last.stage, Stage::Failed);
        assert!(last.problem.is_some());
    }

    #[test]
    fn no_model_means_no_reading_and_a_plain_problem() {
        let evs: Vec<(i64, EventKind)> = vec![
            (1, EventKind::PageDiscovered { url: page(0), from: None }),
            (2, EventKind::PageFetched { url: page(0), status: 200, bytes: 1, ms: 1, via: "local".into(), title: String::new(), words: 10 }),
            (3, EventKind::CrawlFinished { pages: 1, blocked: 0, failed: 0 }),
            (4, EventKind::JobFinished { job: "crawl".into() }),
        ];
        let last = play(&evs, None).pop().unwrap();
        assert_eq!(last.stage, Stage::Failed);
        assert!(last.problem.unwrap().contains("No model"));
    }

    #[test]
    fn an_unreachable_site_says_so() {
        let evs = vec![(1, EventKind::PageFailed { url: page(0), error: "connection refused".into() }), (2, EventKind::CrawlFinished { pages: 0, blocked: 0, failed: 1 }), (3, EventKind::JobFinished { job: "crawl".into() })];
        let last = play(&evs, None).pop().unwrap();
        assert_eq!(last.stage, Stage::Failed);
        assert!(last.problem.unwrap().contains("couldn't open"));
    }

    #[test]
    fn a_raised_limit_or_more_pages_never_moves_backwards() {
        let mut evs = normal();
        // Ten more pages found after the first three were fetched.
        let more: Vec<(i64, EventKind)> = (3..13).map(|n| (14, EventKind::PageDiscovered { url: page(n), from: None })).collect();
        evs.splice(10..10, more);
        let pcts: Vec<u8> = play(&evs, Some(40)).iter().map(|s| s.pct).collect();
        assert!(pcts.windows(2).all(|w| w[0] <= w[1]), "{pcts:?}");
    }
}
