// Demo mode: replays a scripted crawl into the real screens, without a daemon.
// Active when the page is opened with ?demo. Used for previews and the kiqr.dev site.
import type { KiqrEvent } from "./api";

declare global {
  interface Window {
    __KIQR_DEMO__?: boolean;
    __KIQR_DEMO_START__?: string;
  }
}
export const DEMO =
  typeof window !== "undefined" && (window.__KIQR_DEMO__ === true || (typeof location !== "undefined" && /[?&]demo\b/.test(location.search)));

/** `?demo&fresh` starts from an empty workspace, on the onboarding screen. */
export const DEMO_FRESH = DEMO && typeof location !== "undefined" && /[?&]fresh\b/.test(location.search);
export const demoState = { created: !DEMO_FRESH };

const site = "https://unblockingapi.com";

type DemoPage = { path: string; from: "sitemap" | string | null; title: string; words: number; blocked?: string; failed?: string; facts?: number };

const T = (t: string) => `${t} · UnblockingAPI`;
// Order matters: a page found by a link is listed after the page that links to it.
const pages: DemoPage[] = [
  { path: "/", from: null, title: "UnblockingAPI — fetch any public page with one call", words: 1240, facts: 6 },
  // from sitemap.xml
  { path: "/pricing", from: "sitemap", title: T("Pricing"), words: 980, facts: 9 },
  { path: "/docs", from: "sitemap", title: T("Documentation"), words: 640, facts: 2 },
  { path: "/blog", from: "sitemap", title: T("Blog"), words: 520, facts: 1 },
  { path: "/templates", from: "sitemap", title: T("Templates"), words: 860, facts: 4 },
  { path: "/compare", from: "sitemap", title: T("Compare"), words: 410, facts: 2 },
  { path: "/customers", from: "sitemap", title: T("Customers"), words: 760, facts: 5 },
  { path: "/about", from: "sitemap", title: T("About us"), words: 690, facts: 4 },
  { path: "/integrations", from: "sitemap", title: T("Integrations"), words: 570, facts: 6 },
  { path: "/changelog", from: "sitemap", title: T("Changelog"), words: 2310, facts: 3 },
  { path: "/contact", from: "sitemap", title: T("Contact"), words: 180, facts: 1 },
  { path: "/security", from: "sitemap", title: T("Security"), words: 820, facts: 3 },
  { path: "/status", from: "sitemap", title: "", words: 0, blocked: "403 from the CDN" },
  { path: "/docs/getting-started", from: "sitemap", title: T("Getting started"), words: 1430, facts: 3 },
  { path: "/docs/api", from: "sitemap", title: T("API reference"), words: 2980, facts: 3 },
  { path: "/docs/templates", from: "sitemap", title: T("Templates guide"), words: 1650, facts: 4 },
  { path: "/docs/fetch-tuning", from: "sitemap", title: T("Fetch tuning"), words: 1880, facts: 2 },
  { path: "/docs/errors", from: "sitemap", title: T("Errors and retries"), words: 1120, facts: 1 },
  { path: "/docs/concurrency", from: "sitemap", title: T("Concurrency"), words: 740, facts: 2 },
  { path: "/blog/residential-vs-datacenter-proxies", from: "sitemap", title: "Residential vs datacenter proxies", words: 2140, facts: 2 },
  { path: "/blog/scraping-at-scale", from: "sitemap", title: "Scraping at scale without babysitting proxies", words: 2560, facts: 3 },
  { path: "/blog/rank-tracker-tutorial", from: "sitemap", title: "Build a rank tracker in an afternoon", words: 3120, facts: 2 },
  { path: "/blog/price-monitoring", from: "sitemap", title: "Price monitoring that survives redesigns", words: 1980, facts: 2 },
  { path: "/blog/headless-browsers-explained", from: "sitemap", title: "Headless browsers, explained", words: 2270, facts: 1 },
  { path: "/templates/google-search", from: "sitemap", title: T("Google Search template"), words: 610, facts: 2 },
  { path: "/templates/amazon-product", from: "sitemap", title: T("Amazon product template"), words: 590, facts: 1 },
  { path: "/templates/linkedin-company", from: "sitemap", title: T("LinkedIn company template"), words: 540, facts: 1 },
  { path: "/templates/zillow-listing", from: "sitemap", title: T("Zillow listing template"), words: 520, facts: 1 },
  { path: "/compare/brightdata", from: "sitemap", title: "UnblockingAPI vs Bright Data", words: 1340, facts: 5 },
  { path: "/compare/scrapingbee", from: "sitemap", title: "UnblockingAPI vs ScrapingBee", words: 1210, facts: 4 },
  { path: "/compare/zyte", from: "sitemap", title: "UnblockingAPI vs Zyte", words: 1150, facts: 3 },
  // found by following links
  { path: "/app/dashboard", from: "/", title: "", words: 0, blocked: "login required" },
  { path: "/legal/terms", from: "/", title: T("Terms of service"), words: 3420, facts: 1 },
  { path: "/legal/privacy", from: "/", title: T("Privacy policy"), words: 2890, facts: 1 },
  { path: "/pricing/enterprise", from: "/pricing", title: T("Enterprise"), words: 670, facts: 4 },
  { path: "/docs/sdks", from: "/docs", title: T("SDKs and examples"), words: 980, facts: 2 },
  { path: "/docs/webhooks", from: "/docs", title: "", words: 0, failed: "timed out after 20 s" },
  { path: "/docs/api/unblock", from: "/docs/api", title: T("POST /unblock"), words: 2240, facts: 2 },
  { path: "/docs/api/templates", from: "/docs/api", title: T("GET /templates"), words: 1310, facts: 1 },
  { path: "/blog/tags/tutorials", from: "/blog", title: T("Tutorials"), words: 380, facts: 0 },
  { path: "/blog/tags/proxies", from: "/blog", title: T("Proxies"), words: 340, facts: 0 },
  { path: "/blog/cloudflare-challenges", from: "/blog", title: "What a Cloudflare challenge actually checks", words: 2410, facts: 2 },
  { path: "/templates/booking-hotel", from: "/templates", title: T("Booking.com hotel template"), words: 560, facts: 1 },
  { path: "/templates/indeed-jobs", from: "/templates", title: T("Indeed jobs template"), words: 530, facts: 1 },
  { path: "/templates/tripadvisor", from: "/templates", title: T("Tripadvisor template"), words: 500, facts: 1 },
  { path: "/compare/apify", from: "/compare", title: "UnblockingAPI vs Apify", words: 1180, facts: 3 },
  { path: "/customers/acme", from: "/customers", title: "Acme: 2M pages a month", words: 1460, facts: 3 },
  { path: "/customers/northwind", from: "/customers", title: "Northwind tracks 40,000 prices a day", words: 1290, facts: 2 },
  { path: "/careers", from: "/about", title: T("Careers"), words: 450, facts: 2 },
];

type Sample = { section: string; field: string; value: string; quote: string };
const samples: Record<string, Sample[]> = {
  "/": [
    { section: "company", field: "name", value: "UnblockingAPI", quote: "UnblockingAPI" },
    { section: "positioning", field: "value_props", value: "A real browser and residential proxies behind one HTTP call, no SDK needed.", quote: "No SDK needed" },
    { section: "audience", field: "personas", value: "Developers building scrapers, monitors and aggregators.", quote: "for developers" },
    { section: "voice", field: "tone", value: "Plain and technical; short sentences, no superlatives.", quote: "Fetch any public page." },
  ],
  "/pricing": [
    { section: "pricing", field: "plans", value: "Pay as you go, Scout, Builder, Growth — concurrency 5 to 50 requests in flight.", quote: "Scout: 10 requests in flight" },
    { section: "pricing", field: "billing", value: "One credit per successful fetch; failed fetches are free.", quote: "Failures cost nothing" },
    { section: "pricing", field: "free_tier", value: "1,000 free credits on sign-up.", quote: "Start with 1,000 credits" },
  ],
  "/docs": [{ section: "products", field: "offering", value: "REST API: POST /unblock with a URL; returns cleaned HTML or JSON via templates.", quote: "POST /unblock" }],
  "/blog": [{ section: "content", field: "posts", value: "A developer blog with tutorials on proxies, scraping at scale and rank tracking.", quote: "From the blog" }],
  "/templates": [
    { section: "products", field: "templates", value: "A public library of templates that turn well-known sites into JSON.", quote: "Browse templates" },
    { section: "seo", field: "landing_pages", value: "One landing page per template, each targeting '<site> scraper API'.", quote: "Google Search scraper API" },
  ],
  "/compare": [{ section: "competitors", field: "comparison_pages", value: "A comparison hub against four named competitors.", quote: "How we compare" }],
  "/customers": [
    { section: "trust", field: "logos", value: "Customer logos on the customers page, including Acme and Northwind.", quote: "Trusted by" },
    { section: "trust", field: "testimonials", value: "Two quoted testimonials from engineering leads.", quote: "cut our scraping costs in half" },
  ],
  "/about": [
    { section: "company", field: "founded", value: "Founded in 2024 in Madrid by two engineers.", quote: "started in Madrid in 2024" },
    { section: "channels", field: "social", value: "X and GitHub accounts linked in the footer.", quote: "Follow us" },
  ],
  "/integrations": [
    { section: "integrations", field: "integrations", value: "Zapier, n8n, Make and Google Sheets.", quote: "Works with Zapier" },
    { section: "channels", field: "community", value: "A Discord server for developers.", quote: "Join the Discord" },
  ],
  "/changelog": [{ section: "content", field: "changelog", value: "Monthly changelog since March 2025.", quote: "Changelog" }],
  "/contact": [{ section: "company", field: "contact", value: "Email support and a sales form.", quote: "Talk to sales" }],
  "/security": [{ section: "trust", field: "security", value: "No page content stored after delivery; EU data processing.", quote: "We don't keep your pages" }],
  "/docs/getting-started": [{ section: "integrations", field: "stack", value: "Examples in JavaScript, Python and curl.", quote: "fetch in JavaScript" }],
  "/docs/api": [{ section: "seo", field: "keywords", value: "Pages target 'web unblocking API', 'residential proxy API', 'scraping API'.", quote: "web unblocking API" }],
  "/docs/templates": [{ section: "products", field: "features", value: "Templates parse well-known sites into JSON; users can publish their own.", quote: "publish a template" }],
  "/docs/fetch-tuning": [{ section: "products", field: "features", value: "Scroll, wait-for-selector, consent-wall rules and country selection.", quote: "wait_for" }],
  "/docs/concurrency": [{ section: "pricing", field: "limits", value: "Concurrency is the plan limit; there is no monthly request cap.", quote: "no monthly cap" }],
  "/blog/scraping-at-scale": [{ section: "audience", field: "jobs_to_be_done", value: "Fetch thousands of pages reliably without maintaining proxy pools.", quote: "without babysitting proxies" }],
  "/blog/rank-tracker-tutorial": [{ section: "content", field: "posts", value: "Tutorial: build a Google rank tracker in an afternoon.", quote: "" }],
  "/blog/price-monitoring": [{ section: "audience", field: "use_cases", value: "Price monitoring for e-commerce teams.", quote: "track competitor prices" }],
  "/compare/brightdata": [{ section: "competitors", field: "named_competitors", value: "Bright Data — compared on price and setup time.", quote: "vs Bright Data" }],
  "/compare/scrapingbee": [{ section: "competitors", field: "named_competitors", value: "ScrapingBee — compared on JavaScript rendering and templates.", quote: "vs ScrapingBee" }],
  "/compare/zyte": [{ section: "competitors", field: "named_competitors", value: "Zyte — compared on pricing model (credits vs bandwidth).", quote: "vs Zyte" }],
  "/compare/apify": [{ section: "competitors", field: "named_competitors", value: "Apify — positioned as simpler: one endpoint instead of actors.", quote: "no actors to maintain" }],
  "/pricing/enterprise": [{ section: "pricing", field: "enterprise", value: "Enterprise plan with dedicated concurrency, SLA and invoicing.", quote: "Talk to us about volume" }],
  "/customers/acme": [{ section: "trust", field: "case_studies", value: "Acme case study: 2M pages a month, migrated in a week.", quote: "migrated in a week" }],
  "/customers/northwind": [{ section: "trust", field: "case_studies", value: "Northwind tracks 40,000 prices a day with templates.", quote: "40,000 prices a day" }],
  "/careers": [{ section: "company", field: "team", value: "A remote team of nine, hiring a developer advocate.", quote: "We're nine people" }],
  "/legal/terms": [{ section: "company", field: "legal_entity", value: "Operated by Unblocking Labs S.L., Madrid.", quote: "Unblocking Labs S.L." }],
};

/** What synthesis writes per section once the crawl is done. */
const synthesis: [string, string, string[]][] = [
  ["company", "Company", ["UnblockingAPI, run by Unblocking Labs S.L.", "Founded 2024 in Madrid", "Remote team of nine", "Support by email; sales form for volume"]],
  ["products", "Products & services", ["Unblock API: one POST /unblock call per page", "Templates that return JSON for well-known sites", "Fetch tuning: scroll, wait-for, consent walls, country", "SDK examples in JavaScript, Python and curl"]],
  ["pricing", "Pricing", ["Credits: one per successful fetch, failures free", "Plans set concurrency, from 5 to 50 in flight", "1,000 free credits on sign-up", "Enterprise plan with SLA and invoicing"]],
  ["audience", "Audience", ["Developers building scrapers and monitors", "E-commerce teams tracking prices", "SEO teams running rank trackers"]],
  ["positioning", "Positioning", ["A real browser and proxies behind one HTTP call", "No SDK, no proxy pools to maintain", "Simpler than actor platforms, cheaper than bandwidth billing"]],
  ["voice", "Brand voice", ["Plain and technical", "Short sentences, no superlatives", "Shows code before claims"]],
  ["competitors", "Competitors", ["Bright Data", "ScrapingBee", "Zyte", "Apify"]],
  ["seo", "SEO", ["Targets 'web unblocking API' and 'scraping API'", "One landing page per template", "Comparison pages for each competitor"]],
  ["channels", "Channels", ["Developer blog with tutorials", "Discord community", "X and GitHub"]],
  ["content", "Content", ["Tutorials: rank tracker, price monitoring", "Explainers on proxies and browser challenges", "Monthly changelog since March 2025"]],
  ["integrations", "Integrations", ["Zapier, n8n and Make", "Google Sheets", "Plain REST from any language"]],
  ["trust", "Trust", ["Case studies: Acme, Northwind", "Testimonials from engineering leads", "No page content kept after delivery"]],
];
const brief = ["Unblock API", "Templates", "Fetch tuning", "Real-browser rendering", "Residential proxies"];

/** Small seeded PRNG so the replay looks the same every time. */
function rng(seed: number) {
  return () => {
    seed = (seed * 1664525 + 1013904223) >>> 0;
    return seed / 2 ** 32;
  };
}

export function runDemo(project: string, emit: (e: KiqrEvent) => void): () => void {
  const timers: number[] = [];
  const at = (ms: number, e: Record<string, unknown>) =>
    timers.push(window.setTimeout(() => emit({ at: new Date().toISOString(), project, ...e } as KiqrEvent), ms));
  const url = (p: string) => site + p;
  const rand = rng(7);
  const FETCH_SLOTS = 5;
  const AI_SLOTS = 3;

  const t0 = 300;
  at(t0, { type: "crawl_started", max_pages: 200, concurrency: FETCH_SLOTS, ai_concurrency: AI_SLOTS });
  at(t0 + 100, { type: "page_discovered", url: url("/"), from: null });

  // Fetch scheduling: five slots, first come first served.
  const slots = Array<number>(FETCH_SLOTS).fill(t0 + 300);
  const fetchedAt = new Map<string, number>();
  const reads: { path: string; queuedAt: number }[] = [];
  let sitemapAt = t0 + 900;
  let lastFetch = 0;
  let ok = 0;
  let blocked = 0;
  let failed = 0;
  for (const pg of pages) {
    let ready: number;
    if (pg.from === null) ready = t0 + 200;
    else if (pg.from === "sitemap") {
      ready = sitemapAt;
      at(sitemapAt, { type: "page_discovered", url: url(pg.path), from: "sitemap" });
      sitemapAt += 25;
    } else {
      const parentDone = fetchedAt.get(pg.from) ?? t0 + 1000;
      ready = parentDone + 60 + rand() * 120;
      at(ready - 40, { type: "page_discovered", url: url(pg.path), from: url(pg.from) });
    }
    const slot = slots.indexOf(Math.min(...slots));
    const start = Math.max(ready, slots[slot]);
    const dur = 380 + rand() * 820;
    const end = start + dur;
    slots[slot] = end + 20;
    lastFetch = Math.max(lastFetch, end);
    fetchedAt.set(pg.path, end);
    at(start, { type: "page_fetching", url: url(pg.path), via: "local" });
    if (pg.blocked) {
      blocked++;
      at(end, { type: "page_blocked", url: url(pg.path), reason: pg.blocked });
    } else if (pg.failed) {
      failed++;
      at(end, { type: "page_failed", url: url(pg.path), error: pg.failed });
    } else {
      ok++;
      at(end, { type: "page_fetched", url: url(pg.path), status: 200, bytes: pg.words * 9 + 18000, ms: Math.round(dur), via: "local", title: pg.title, words: pg.words });
      reads.push({ path: pg.path, queuedAt: end + 60 });
    }
  }

  // The model reads fetched pages three at a time, in the order they landed.
  reads.sort((a, b) => a.queuedAt - b.queuedAt);
  const ai = Array<number>(AI_SLOTS).fill(0);
  const parsed = reads.map((r) => {
    const slot = ai.indexOf(Math.min(...ai));
    const start = Math.max(r.queuedAt, ai[slot]);
    const done = start + 420 + rand() * 520;
    ai[slot] = done;
    return { ...r, parsedAt: done };
  });
  const waitingAt = (t: number) => parsed.filter((p) => p.queuedAt <= t && p.parsedAt > t).length;
  let lastParse = lastFetch;
  for (const p of parsed) {
    const pg = pages.find((x) => x.path === p.path)!;
    const facts = pg.facts ?? 1;
    const preview = (samples[p.path] ?? (facts > 0 ? [{ section: "content", field: "posts", value: pg.title, quote: "" }] : [])).slice(0, 6);
    at(p.queuedAt, { type: "page_queued", url: url(p.path), waiting: waitingAt(p.queuedAt) });
    at(p.parsedAt, { type: "page_parsed", url: url(p.path), facts, waiting: Math.max(0, waitingAt(p.parsedAt + 1)), preview });
    lastParse = Math.max(lastParse, p.parsedAt);
  }

  const end = lastParse + 500;
  at(end, { type: "crawl_finished", pages: ok, blocked, failed });
  at(end + 200, {
    type: "seemo_says",
    text: `I read ${ok} pages. ${blocked} wouldn't load for me and ${failed} timed out. If you connect UnblockingAPI I can try the blocked ones again. Writing the profile from what I have.`,
  });
  let ts = end + 600;
  at(ts, { type: "synthesis_started", total: synthesis.length + 1 });
  for (const [section, title, items] of synthesis) {
    ts += 480 + rand() * 260;
    at(ts, { type: "profile_updated", section, by: "seemo", title, items });
  }
  ts += 700;
  at(ts, { type: "profile_updated", section: "brief", by: "seemo", title: "Business brief", items: brief });
  at(ts + 300, { type: "synthesis_finished", sections: synthesis.length + 1 });
  at(ts + 600, { type: "seemo_says", text: "The profile is ready. Have a look and lock anything I got wrong." });
  return () => timers.forEach(clearTimeout);
}

export const demoProject = {
  slug: "unblockingapi-com",
  name: "unblockingapi.com",
  url: site,
  created_at: new Date().toISOString(),
  crawl: { max_pages: null, concurrency: null, ai_concurrency: null, include: [], exclude: [] },
  last_crawl: null,
};
