//! The site crawler: same-origin BFS with a priority boost for the pages that
//! say most about a business, a page cap, robots.txt, a raw cache, and live
//! events for every step.

use crate::events::{EventBus, EventKind};
use crate::extract::Extractor;
use crate::fetch::{obstacle, Fetched, Fetcher, LocalFetcher, UnblockingFetcher};
use crate::markdown;
use crate::project::{CrawlSummary, Project};
use crate::Store;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashSet};
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
}

struct State {
    queue: BTreeSet<(i32, u64, String)>, // (-priority, insertion order, url)
    seen: HashSet<String>,
    order: u64,
    fetched: usize,
    blocked: usize,
    failed: usize,
    escalate_whole_site: bool,
}

impl Crawler {
    pub fn new(store: Store, bus: EventBus, concurrency: usize, escalate: bool, extractor: Option<Arc<Extractor>>) -> Result<Self> {
        let unblocking = if escalate { UnblockingFetcher::from_env().map(Arc::new) } else { None };
        Ok(Self { store, bus, local: Arc::new(LocalFetcher::new()?), unblocking, extractor, concurrency })
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

        self.bus.emit(Some(&slug), EventKind::CrawlStarted { max_pages });

        let robots = self.load_robots(&start).await;
        let state = Arc::new(Mutex::new(State {
            queue: BTreeSet::new(),
            seen: HashSet::new(),
            order: 0,
            fetched: 0,
            blocked: 0,
            failed: 0,
            escalate_whole_site: false,
        }));

        {
            let mut st = state.lock().await;
            push(&mut st, &start, 1000);
            for u in self.load_sitemap(&start).await {
                if canonical_host(&u) == host {
                    push(&mut st, &u, priority_for(&u) + 10);
                }
            }
        }

        let sem = Arc::new(Semaphore::new(self.concurrency));
        let mut in_flight = tokio::task::JoinSet::new();

        loop {
            let next = {
                let mut st = state.lock().await;
                if st.fetched + in_flight.len() >= max_pages {
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
                    let permit = sem.clone().acquire_owned().await?;
                    let this = self.clone_for_task();
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
                        break;
                    }
                    // Wait for one task to finish, then loop: it may have enqueued more.
                    let _ = in_flight.join_next().await;
                }
            }
        }
        while in_flight.join_next().await.is_some() {}

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
        }
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

        if !fetched.content_type.contains("html") && !fetched.content_type.is_empty() {
            return; // not a page
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
            },
        );

        {
            let mut st = state.lock().await;
            st.fetched += 1;
            for link in &page.links {
                if canonical_host(link) == host && is_page(link) {
                    let p = priority_for(link);
                    push(&mut st, link, p);
                }
            }
        }

        if let Some(ex) = &self.extractor {
            match ex.extract_page(slug, &url, &page.title, &page.markdown).await {
                Ok(n) => self.bus.emit(Some(slug), EventKind::PageParsed { url: url.to_string(), facts: n }),
                Err(e) => tracing::warn!(%url, error = %e, "extraction failed"),
            }
        } else {
            self.bus.emit(Some(slug), EventKind::PageParsed { url: url.to_string(), facts: 0 });
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

fn push(st: &mut State, url: &Url, priority: i32) {
    let mut u = url.clone();
    u.set_fragment(None);
    strip_tracking(&mut u);
    let key = u.to_string();
    if st.seen.insert(key.clone()) {
        st.order += 1;
        st.queue.insert((-priority, st.order, key));
    }
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
