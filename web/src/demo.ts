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

const site = "https://unblockingapi.com";
const pages: [string, string | null, number][] = [
  ["/", null, 6],
  ["/pricing", "/", 9],
  ["/docs", "/", 2],
  ["/blog", "/", 1],
  ["/customers", "/", 5],
  ["/about", "/", 4],
  ["/docs/getting-started", "/docs", 3],
  ["/docs/templates", "/docs", 4],
  ["/docs/api", "/docs", 3],
  ["/blog/residential-vs-datacenter-proxies", "/blog", 2],
  ["/blog/scraping-at-scale", "/blog", 3],
  ["/customers/acme", "/customers", 3],
  ["/compare/brightdata", "/", 5],
  ["/compare/scrapingbee", "/", 4],
  ["/integrations", "/", 6],
  ["/app/dashboard", "/", -1],
  ["/docs/fetch-tuning", "/docs", 2],
  ["/blog/rank-tracker-tutorial", "/blog", 2],
  ["/changelog", "/", 3],
  ["/contact", "/", 1],
];
const samples: Record<string, { section: string; field: string; value: string; quote: string }[]> = {
  "/": [
    { section: "company", field: "tagline", value: "A real browser and residential proxies behind one HTTP call.", quote: "No SDK needed" },
    { section: "audience", field: "personas", value: "Developers building scrapers, monitors and aggregators.", quote: "for developers" },
  ],
  "/pricing": [
    { section: "pricing", field: "plans", value: "Pay as you go, Scout, Builder, Growth — concurrency 5 to 50.", quote: "Scout: 10 requests in flight" },
    { section: "pricing", field: "billing", value: "One credit per successful fetch; failures are free.", quote: "Failures cost nothing" },
  ],
  "/customers": [{ section: "trust", field: "logos", value: "Customer logos on the home page: four named companies.", quote: "Trusted by" }],
  "/about": [{ section: "company", field: "founded", value: "Founded in 2024 in Madrid.", quote: "started in Madrid in 2024" }],
  "/compare/brightdata": [{ section: "competitors", field: "named_competitors", value: "Bright Data, compared on price and setup time.", quote: "vs Bright Data" }],
  "/integrations": [{ section: "integrations", field: "integrations", value: "Zapier, n8n, Make, Google Sheets.", quote: "Works with Zapier" }],
};

export function runDemo(project: string, emit: (e: KiqrEvent) => void): () => void {
  const timers: number[] = [];
  const at = (ms: number, e: Record<string, unknown>) =>
    timers.push(window.setTimeout(() => emit({ at: new Date().toISOString(), project, ...e } as KiqrEvent), ms));
  const url = (p: string) => site + (p === "/" ? "/" : p);
  let t = 300;
  at(t, { type: "crawl_started", max_pages: 200, concurrency: 5, ai_concurrency: 3 });
  at(t + 100, { type: "page_discovered", url: url("/"), from: null });
  let waiting = 0;
  const parsedQueue: [number, string, number][] = [];
  pages.forEach(([p, from, facts], i) => {
    const start = t + 400 + i * 420;
    if (from) at(start - 300, { type: "page_discovered", url: url(p), from: url(from) });
    at(start, { type: "page_fetching", url: url(p), via: "local" });
    if (facts < 0) {
      at(start + 500, { type: "page_blocked", url: url(p), reason: "needs JavaScript" });
      return;
    }
    at(start + 350 + Math.random() * 400, { type: "page_fetched", url: url(p), status: 200, bytes: 42000, ms: 320 + Math.round(Math.random() * 500), via: "local" });
    waiting++;
    const w = waiting;
    at(start + 800, { type: "page_queued", url: url(p), waiting: Math.min(w, 4) });
    parsedQueue.push([start + 2400 + i * 600, p, facts]);
  });
  parsedQueue.forEach(([when, p, facts], i) => {
    at(when, { type: "page_parsed", url: url(p), facts, waiting: Math.max(0, parsedQueue.length - i - 1 > 3 ? 3 : parsedQueue.length - i - 1), preview: samples[p] ?? [{ section: "content", field: "posts", value: `${p.split("/").pop()?.replace(/-/g, " ")}`, quote: "" }] });
  });
  const end = parsedQueue[parsedQueue.length - 1][0] + 900;
  at(end, { type: "crawl_finished", pages: pages.length - 1, blocked: 1, failed: 0 });
  at(end + 200, { type: "seemo_says", text: "I read 19 pages. One wouldn't load for me — it needs JavaScript. If you connect UnblockingAPI I can read that too. Building the profile from what I have." });
  at(end + 6000, { type: "seemo_says", text: "The profile is ready. Have a look and lock anything I got wrong." });
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
