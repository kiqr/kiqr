//! The site crawler: same-origin BFS with a priority boost for the pages that
//! say most about a business, a page cap, robots.txt, a raw cache, and live
//! events for every step.

use crate::events::{EventBus, EventKind};
use crate::extract::Extractor;
use crate::fetch::{obstacle, Fetched, Fetcher, LocalFetcher, UnblockingAuth, UnblockingFetcher};
use crate::markdown;
use crate::project::{CrawlSummary, Project};
use crate::Store;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use url::Url;

/// Paths that typically carry the most information, crawled first.
const PRIORITY_HINTS: &[(&str, i32)] = &[
    ("/pricing", 100),
    ("/plans", 90),
    ("/about", 90),
    ("/product", 80),
    ("/features", 80),
    ("/solutions", 70),
    ("/customers", 70),
    ("/case-stud", 70),
    ("/docs", 50),
    ("/blog", 30),
    ("/compare", 60),
    ("/vs", 60),
    ("/integrations", 60),
    ("/faq", 60),
    ("/contact", 40),
    ("/team", 40),
    ("/careers", 20),
];

const SKIP_EXTENSIONS: &[&str] = &[
    ".pdf", ".jpg", ".jpeg", ".png", ".gif", ".svg", ".webp", ".ico", ".css", ".js", ".json", ".xml", ".zip", ".mp4",
    ".mp3", ".woff", ".woff2", ".ttf", ".txt",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CachedPage {
    pub url: String,
    pub final_url: String,
    pub status: u16,
    pub via: String,
    pub fetched_at: chrono::DateTime<Utc>,
    pub title: String,
    pub description: Option<String>,
    pub text_len: usize,
    pub obstacle: Option<String>,
}

pub struct Crawler {
    store: Store,
    bus: EventBus,
    local: Arc<LocalFetcher>,
    unblocking: Option<Arc<UnblockingFetcher>>,
    extractor: Option<Arc<Extractor>>,
    concurrency: usize,
    ai_concurrency: usize,
    ai_tx: Option<tokio::sync::mpsc::UnboundedSender<ExtractJob>>,
    control: Option<Arc<CrawlControl>>,
    /// Where the business's profiles may live, (host, channel kind), for the brand.
    profiles: Vec<(String, String)>,
}

/// The page limit of a running crawl. It can be changed while pages are being
/// fetched; once fetching has ended it is closed, and a new limit only applies
/// to the next crawl.
#[derive(Debug)]
pub struct CrawlControl {
    /// (limit, fetching ended)
    inner: std::sync::Mutex<(usize, bool)>,
}

impl CrawlControl {
    pub fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self { inner: std::sync::Mutex::new((limit.max(1), false)) })
    }

    pub fn limit(&self) -> usize {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).0
    }

    /// Change the limit of the running crawl. False if fetching has already ended.
    pub fn set_limit(&self, limit: usize) -> bool {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if g.1 {
            return false;
        }
        g.0 = limit.max(1);
        true
    }

    /// End fetching unless `more(limit)` says there is still room and work,
    /// atomically with `set_limit`, so a raise is never lost. True if ended.
    fn close_unless(&self, more: impl Fn(usize) -> bool) -> bool {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if more(g.0) {
            return false;
        }
        g.1 = true;
        true
    }
}

struct ExtractJob {
    url: Url,
    title: String,
    markdown: String,
}

/// Per-run options, settable before each crawl.
#[derive(Clone, Debug, Default)]
pub struct CrawlOptions {
    pub max_pages: Option<usize>,
    pub concurrency: Option<usize>,
    pub ai_concurrency: Option<usize>,
}

struct State {
    queue: BTreeSet<(i32, u64, String)>, // (-priority, insertion order, url)
    seen: HashSet<String>,
    order: u64,
    fetched: usize,
    blocked: usize,
    failed: usize,
    escalate_whole_site: bool,
    /// Links that redirected to another website, by first path segment
    /// (`/till/…`, `/go/…`). After a few, the rest of that path is skipped unfetched.
    offsite: HashMap<String, usize>,
    skipped: usize,
    /// The project's start URL, as queued.
    start: String,
    /// Where each page read ended up, so two links to one page are read once.
    landed: HashSet<String>,
}

/// Off-site redirects under one path before the crawler stops fetching that path.
const OFFSITE_PATTERN: usize = 3;

/// The first path segment, the unit an outbound-link pattern is learned on.
fn first_segment(u: &Url) -> String {
    u.path_segments().and_then(|mut s| s.next()).unwrap_or("").to_string()
}

/// A crawled URL that ended up on another website: an affiliate or outbound
/// link that redirected. Its content belongs to that site, not this business.
pub fn left_site(c: &CachedPage) -> bool {
    match (Url::parse(&c.url), Url::parse(&c.final_url)) {
        (Ok(a), Ok(b)) => canonical_host(&a) != canonical_host(&b),
        _ => false,
    }
}

/// Whether the cached page stored under `key` (the hash also used for its
/// facts file) left the site.
pub fn cached_left_site(store: &Store, slug: &str, key: &str) -> bool {
    let path = store.home().cache_dir(slug).join("pages").join(format!("{key}.yaml"));
    store.read_yaml::<CachedPage>(&path).is_ok_and(|c| left_site(&c))
}

impl Crawler {
    /// `unblocking_key`: the UnblockingAPI key for this project's pages (its
    /// own, else the machine's), used only when escalation is on.
    pub fn new(store: Store, bus: EventBus, concurrency: usize, ai_concurrency: usize, escalate: bool, unblocking: Option<UnblockingAuth>, extractor: Option<Arc<Extractor>>) -> Result<Self> {
        let unblocking = if escalate { UnblockingFetcher::with_auth(unblocking).map(Arc::new) } else { None };
        Ok(Self {
            store,
            bus,
            local: Arc::new(LocalFetcher::new()?),
            unblocking,
            extractor,
            concurrency: concurrency.clamp(1, 10),
            ai_concurrency: ai_concurrency.clamp(1, 10),
            ai_tx: None,
            control: None,
            profiles: Vec::new(),
        })
    }

    pub fn has_escalation(&self) -> bool {
        self.unblocking.is_some()
    }

    pub async fn run(&self, project: &Project, max_pages: usize) -> Result<CrawlSummary> {
        let slug = project.slug.clone();
        let start = Url::parse(&project.url)?;
        let host = canonical_host(&start);
        let cache = self.store.home().cache_dir(&slug).join("pages");
        std::fs::create_dir_all(&cache)?;

        self.bus.emit(
            Some(&slug),
            EventKind::CrawlStarted { max_pages, concurrency: self.concurrency, ai_concurrency: self.ai_concurrency },
        );

        // The model reads pages from a queue with its own parallelism, so a slow
        // model never slows the fetcher down.
        let (ai_tx, ai_rx) = tokio::sync::mpsc::unbounded_channel::<ExtractJob>();
        let ai_pool = tokio::spawn(ai_worker_pool(
            ai_rx,
            self.extractor.clone(),
            self.bus.clone(),
            slug.clone(),
            self.ai_concurrency,
        ));
        let mut me = self.clone_for_task();
        me.ai_tx = Some(ai_tx);
        let this_run = Arc::new(me);

        let robots = self.load_robots(&start).await;
        let state = Arc::new(Mutex::new(State {
            queue: BTreeSet::new(),
            seen: HashSet::new(),
            order: 0,
            fetched: 0,
            blocked: 0,
            failed: 0,
            escalate_whole_site: false,
            offsite: HashMap::new(),
            skipped: 0,
            start: start.to_string(),
            landed: HashSet::new(),
        }));

        {
            let mut st = state.lock().await;
            if let Some(key) = push(&mut st, &start, 1000) {
                st.start = key;
            }
            self.bus.emit(Some(&slug), EventKind::PageDiscovered { url: start.to_string(), from: None });
            for u in self.load_sitemap(&start).await {
                if canonical_host(&u) == host {
                    if let Some(key) = push(&mut st, &u, priority_for(&u) + 10) {
                        self.bus.emit(Some(&slug), EventKind::PageDiscovered { url: key, from: Some("sitemap".into()) });
                    }
                }
            }
        }

        let control = self.control.clone().unwrap_or_else(|| CrawlControl::new(max_pages));
        let sem = Arc::new(Semaphore::new(self.concurrency));
        let mut in_flight = tokio::task::JoinSet::new();

        loop {
            let next = {
                let mut st = state.lock().await;
                if st.fetched + in_flight.len() >= control.limit() {
                    None
                } else {
                    st.queue.iter().next().cloned().map(|k| {
                        st.queue.remove(&k);
                        k.2
                    })
                }
            };
            match next {
                Some(url_s) => {
                    let url = Url::parse(&url_s)?;
                    if let Some(r) = &robots {
                        if !r.allowed(url.as_str()) {
                            continue;
                        }
                    }
                    let seg = first_segment(&url);
                    let outbound = {
                        let mut st = state.lock().await;
                        let hit = !seg.is_empty() && st.offsite.get(&seg).copied().unwrap_or(0) >= OFFSITE_PATTERN;
                        st.skipped += usize::from(hit);
                        hit
                    };
                    if outbound {
                        self.bus.emit(
                            Some(&slug),
                            EventKind::PageSkipped { url: url_s, reason: format!("Links under /{seg}/ lead to other websites") },
                        );
                        continue;
                    }
                    let permit = sem.clone().acquire_owned().await?;
                    let this = this_run.clone();
                    let st = state.clone();
                    let cache = cache.clone();
                    let slug = slug.clone();
                    let host = host.clone();
                    in_flight.spawn(async move {
                        let _permit = permit;
                        this.handle(&slug, &host, url, &cache, st).await;
                    });
                }
                None => {
                    if in_flight.is_empty() {
                        // The limit may have been raised since we looked.
                        let st = state.lock().await;
                        let more = |limit: usize| !st.queue.is_empty() && st.fetched < limit;
                        if control.close_unless(more) {
                            break;
                        }
                        continue;
                    }
                    // Wait for one task to finish, then loop: it may have enqueued more.
                    let _ = in_flight.join_next().await;
                }
            }
        }
        while in_flight.join_next().await.is_some() {}
        // Brand assets come from the raw HTML already on disk, so they are
        // ready while the model is still reading.
        match crate::brand::refresh(&self.store, &self.bus, project, true, self.profiles.clone()).await {
            Ok(_) => {}
            Err(e) => tracing::warn!(%slug, error = %e, "brand collection failed"),
        }
        // Close the reading queue and wait for the model to finish what is left.
        drop(this_run);
        let _ = ai_pool.await;

        let st = state.lock().await;
        let summary = CrawlSummary { finished_at: Utc::now(), pages: st.fetched, blocked: st.blocked, failed: st.failed };
        drop(st);

        let mut project = Project::load(&self.store, &slug)?;
        project.last_crawl = Some(summary.clone());
        project.save(&self.store)?;
        self.store.commit(&format!("[auto] crawl({slug}): {} pages, {} blocked", summary.pages, summary.blocked)).await?;
        self.bus.emit(Some(&slug), EventKind::CrawlFinished { pages: summary.pages, blocked: summary.blocked, failed: summary.failed });

        let text = seemo_crawl_summary(&summary, self.unblocking.is_some());
        self.bus.emit(Some(&slug), EventKind::SeemoSays { text });
        Ok(summary)
    }

    fn clone_for_task(&self) -> Crawler {
        Crawler {
            store: self.store.clone(),
            bus: self.bus.clone(),
            local: self.local.clone(),
            unblocking: self.unblocking.clone(),
            extractor: self.extractor.clone(),
            concurrency: self.concurrency,
            ai_concurrency: self.ai_concurrency,
            ai_tx: self.ai_tx.clone(),
            control: self.control.clone(),
            profiles: self.profiles.clone(),
        }
    }

    /// Let the caller change the page limit while this crawler runs.
    pub fn with_control(mut self, control: Arc<CrawlControl>) -> Self {
        self.control = Some(control);
        self
    }

    /// The hosts profiles live on, from the channel kinds abilities declare.
    pub fn with_profiles(mut self, profiles: Vec<(String, String)>) -> Self {
        self.profiles = profiles;
        self
    }

    async fn handle(&self, slug: &str, host: &str, url: Url, cache: &std::path::Path, state: Arc<Mutex<State>>) {
        self.bus.emit(Some(slug), EventKind::PageFetching { url: url.to_string(), via: "local".into() });

        let escalate_first = state.lock().await.escalate_whole_site && self.unblocking.is_some();
        let fetched = if escalate_first {
            self.fetch_escalated(slug, &url).await
        } else {
            match self.local.fetch(&url).await {
                Ok(f) => Ok(f),
                Err(e) => Err(e),
            }
        };

        let fetched = match fetched {
            Ok(f) => f,
            Err(e) => {
                state.lock().await.failed += 1;
                self.bus.emit(Some(slug), EventKind::PageFailed { url: url.to_string(), error: e.to_string() });
                return;
            }
        };

        // An external redirect is not followed (see LocalFetcher), so the other
        // site's page is never fetched; a fetcher that did follow one shows it
        // in `final_url`. Either way the link leads off the site.
        let offsite = match &fetched.redirect_to {
            Some(to) if canonical_host(to) != host => Some(canonical_host(to)),
            _ if canonical_host(&fetched.final_url) != host => Some(canonical_host(&fetched.final_url)),
            _ => None,
        };
        if let Some(to) = offsite {
            let mut st = state.lock().await;
            *st.offsite.entry(first_segment(&url)).or_default() += 1;
            st.skipped += 1;
            let is_start = st.start == url.as_str();
            drop(st);
            if is_start {
                self.bus.emit(
                    Some(slug),
                    EventKind::SeemoSays {
                        text: format!("{url} sends visitors on to {to}, so there's nothing of this site's own for me to read. If {to} is the business's website, add it as the project instead."),
                    },
                );
            }
            self.bus.emit(Some(slug), EventKind::PageSkipped { url: url.to_string(), reason: format!("Redirects to {to}") });
            return;
        }

        if !fetched.content_type.contains("html") && !fetched.content_type.is_empty() {
            self.bus.emit(Some(slug), EventKind::PageSkipped { url: url.to_string(), reason: "Not a web page".into() });
            return;
        }

        // Two links that redirect to the same page (an internal 301 and a direct link): read it once.
        let mut landed = fetched.final_url.clone();
        landed.set_fragment(None);
        if !state.lock().await.landed.insert(landed.to_string()) {
            let path = landed.path().to_string();
            self.bus.emit(Some(slug), EventKind::PageSkipped { url: url.to_string(), reason: format!("Same page as {path}") });
            return;
        }

        let mut page = markdown::parse(&fetched.final_url, &fetched.body);
        let mut fetched = fetched;
        let mut obstacle_hit = obstacle(&fetched, page.text_len, page.script_len);

        if obstacle_hit.is_some() && fetched.via == "local" {
            if self.unblocking.is_some() {
                state.lock().await.escalate_whole_site = true;
                match self.fetch_escalated(slug, &url).await {
                    Ok(f) => {
                        page = markdown::parse(&f.final_url, &f.body);
                        fetched = f;
                        obstacle_hit = None;
                    }
                    Err(e) => {
                        tracing::warn!(%url, error = %e, "escalation failed");
                    }
                }
            }
        }

        let cached = CachedPage {
            url: url.to_string(),
            final_url: fetched.final_url.to_string(),
            status: fetched.status,
            via: fetched.via.to_string(),
            fetched_at: Utc::now(),
            title: page.title.clone(),
            description: page.description.clone(),
            text_len: page.text_len,
            obstacle: obstacle_hit.as_ref().map(|o| o.reason()),
        };
        let key = hash(url.as_str());
        let _ = self.store.write_text(&cache.join(format!("{key}.html")), &fetched.body);
        let _ = self.store.write_text(&cache.join(format!("{key}.md")), &page.markdown);
        let _ = self.store.write_yaml(&cache.join(format!("{key}.yaml")), &cached);

        if let Some(o) = obstacle_hit {
            state.lock().await.blocked += 1;
            self.bus.emit(Some(slug), EventKind::PageBlocked { url: url.to_string(), reason: o.reason() });
            return;
        }

        self.bus.emit(
            Some(slug),
            EventKind::PageFetched {
                url: url.to_string(),
                status: fetched.status,
                bytes: fetched.body.len(),
                ms: fetched.elapsed.as_millis() as u64,
                via: fetched.via.to_string(),
                title: page.title.clone(),
                words: page.markdown.split_whitespace().count(),
            },
        );

        let discovered = {
            let mut st = state.lock().await;
            st.fetched += 1;
            let mut new = Vec::new();
            for link in &page.links {
                if canonical_host(link) == host && is_page(link) {
                    let p = priority_for(link);
                    if let Some(key) = push(&mut st, link, p) {
                        new.push(key);
                    }
                }
            }
            new
        };
        for key in discovered {
            self.bus.emit(Some(slug), EventKind::PageDiscovered { url: key, from: Some(url.to_string()) });
        }

        if let Some(tx) = &self.ai_tx {
            let _ = tx.send(ExtractJob { url: url.clone(), title: page.title.clone(), markdown: page.markdown.clone() });
        }
    }

    async fn fetch_escalated(&self, slug: &str, url: &Url) -> Result<Fetched> {
        let f = self.unblocking.as_ref().expect("checked by caller");
        self.bus.emit(Some(slug), EventKind::PageFetching { url: url.to_string(), via: "unblockingapi".into() });
        f.fetch(url).await
    }

    async fn load_robots(&self, start: &Url) -> Option<texting_robots::Robot> {
        let mut u = start.clone();
        u.set_path("/robots.txt");
        u.set_query(None);
        let f = self.local.fetch(&u).await.ok()?;
        if f.status != 200 {
            return None;
        }
        texting_robots::Robot::new("KIQR", f.body.as_bytes()).ok()
    }

    async fn load_sitemap(&self, start: &Url) -> Vec<Url> {
        let mut u = start.clone();
        u.set_path("/sitemap.xml");
        u.set_query(None);
        let Ok(f) = self.local.fetch(&u).await else { return vec![] };
        if f.status != 200 {
            return vec![];
        }
        let mut out = Vec::new();
        let mut rest = f.body.as_str();
        while let Some(i) = rest.find("<loc>") {
            rest = &rest[i + 5..];
            if let Some(j) = rest.find("</loc>") {
                if let Ok(u) = Url::parse(rest[..j].trim()) {
                    out.push(u);
                }
                rest = &rest[j + 6..];
            } else {
                break;
            }
        }
        out.truncate(2000);
        out
    }
}

/// Enqueue a URL once. Returns the canonical key when it was new.
fn push(st: &mut State, url: &Url, priority: i32) -> Option<String> {
    let mut u = url.clone();
    u.set_fragment(None);
    strip_tracking(&mut u);
    let key = u.to_string();
    if st.seen.insert(key.clone()) {
        st.order += 1;
        st.queue.insert((-priority, st.order, key.clone()));
        Some(key)
    } else {
        None
    }
}

/// Reads fetched pages with the model, `ai_concurrency` at a time, emitting
/// queue depth so the UI can show Seemo working through the backlog.
async fn ai_worker_pool(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<ExtractJob>,
    extractor: Option<Arc<Extractor>>,
    bus: EventBus,
    slug: String,
    ai_concurrency: usize,
) {
    let sem = Arc::new(Semaphore::new(ai_concurrency));
    let waiting = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut tasks = tokio::task::JoinSet::new();
    while let Some(job) = rx.recv().await {
        let n = waiting.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        bus.emit(Some(&slug), EventKind::PageQueued { url: job.url.to_string(), waiting: n });
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };
        let (ex, bus, slug, waiting) = (extractor.clone(), bus.clone(), slug.clone(), waiting.clone());
        tasks.spawn(async move {
            let _permit = permit;
            let facts = match &ex {
                Some(ex) => match ex.extract_page(&slug, &job.url, &job.title, &job.markdown).await {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::warn!(url = %job.url, error = %e, "extraction failed");
                        vec![]
                    }
                },
                None => vec![],
            };
            let left = waiting.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) - 1;
            let preview = facts
                .iter()
                .take(6)
                .map(|f| crate::events::FactPreview { section: f.section.clone(), field: f.field.clone(), value: f.value.clone(), quote: f.quote.clone() })
                .collect();
            bus.emit(Some(&slug), EventKind::PageParsed { url: job.url.to_string(), facts: facts.len(), waiting: left, preview });
        });
        // Reap finished tasks so the set does not grow unbounded.
        while tasks.try_join_next().is_some() {}
    }
    while tasks.join_next().await.is_some() {}
}

fn strip_tracking(u: &mut Url) {
    let keep: Vec<(String, String)> = u
        .query_pairs()
        .filter(|(k, _)| !k.starts_with("utm_") && k != "fbclid" && k != "gclid" && k != "ref")
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if keep.is_empty() {
        u.set_query(None);
    } else {
        let mut q = u.query_pairs_mut();
        q.clear();
        for (k, v) in keep {
            q.append_pair(&k, &v);
        }
    }
}

fn canonical_host(u: &Url) -> String {
    u.host_str().unwrap_or("").trim_start_matches("www.").to_ascii_lowercase()
}

fn is_page(u: &Url) -> bool {
    let p = u.path().to_ascii_lowercase();
    !SKIP_EXTENSIONS.iter().any(|e| p.ends_with(e))
}

fn priority_for(u: &Url) -> i32 {
    let p = u.path().to_ascii_lowercase();
    let hint = PRIORITY_HINTS.iter().filter(|(k, _)| p.starts_with(k) || p.contains(&format!("{k}/"))).map(|(_, v)| *v).max().unwrap_or(0);
    let depth = p.matches('/').count() as i32;
    hint - depth * 5
}

fn hash(s: &str) -> String {
    let d = Sha256::digest(s.as_bytes());
    d.iter().take(12).map(|b| format!("{b:02x}")).collect()
}

/// What Seemo says when a crawl finishes. UnblockingAPI is mentioned only when
/// pages could not be read, and only then.
pub fn seemo_crawl_summary(s: &CrawlSummary, escalation_enabled: bool) -> String {
    if s.blocked == 0 {
        return format!("I finished reading the site. {} pages. I'm building the profile now.", s.pages);
    }
    if escalation_enabled {
        format!(
            "I finished reading the site. {} pages; {} still wouldn't load even with UnblockingAPI. I'm building the profile from what I have.",
            s.pages, s.blocked
        )
    } else {
        format!(
            "I read {} pages. {} wouldn't load for me — they were blocked or need JavaScript. If you connect UnblockingAPI I can read those too. Building the profile from what I have.",
            s.pages, s.blocked
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(url: &str, final_url: &str) -> CachedPage {
        CachedPage {
            url: url.into(),
            final_url: final_url.into(),
            status: 200,
            via: "local".into(),
            fetched_at: Utc::now(),
            title: String::new(),
            description: None,
            text_len: 0,
            obstacle: None,
        }
    }

    #[test]
    fn outbound_redirects_leave_the_site() {
        assert!(left_site(&page("https://whitening.se/till/strips", "https://www.meds.se/strips")));
        assert!(!left_site(&page("https://whitening.se/a", "https://www.whitening.se/a/")));
        assert_eq!(first_segment(&Url::parse("https://whitening.se/till/strips").unwrap()), "till");
    }
}
