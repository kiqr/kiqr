// Live crawl state, fed by the project's event stream. Ported from ContentWriter's crawl store,
// mapped onto KIQR's events (see docs/PLAN.md and crates/kiqr-core/src/events.rs).
import type { KiqrEvent } from "../api";

export type NodeStatus = "queued" | "fetching" | "rendering" | "ok" | "blocked" | "error" | "skipped";
export type Phase = "connecting" | "discovering" | "crawling" | "learning" | "done";

export interface GraphNode {
  id: string;
  path: string;
  parent: string | null;
  depth: number;
  source: string;
  status: NodeStatus;
  httpStatus?: number;
  title?: string | null;
  words?: number;
  ms?: number;
  reason?: string;
  via?: string;
  /** Waiting for or being read by the model. */
  reading: boolean;
  facts?: number;
  section?: string;
  inbound: number;
  /** performance.now() timestamps driving canvas animations */
  spawnedAt: number;
  changedAt: number;
}

export interface FeedItem {
  key: string;
  url: string;
  kind: "fetched" | "blocked" | "failed";
  status?: number;
  title?: string | null;
  words?: number;
  ms?: number;
  reason?: string;
  via?: string;
  t: number;
}

export interface Effect {
  kind: "packet-out" | "packet-in" | "burst";
  url: string;
  color?: string;
  at: number;
}

export type Fact = { section: string; field: string; value: string; quote?: string };

export interface CrawlState {
  phase: Phase;
  siteUrl: string;
  maxPages: number;
  concurrency: number | null;
  aiConcurrency: number | null;
  nodes: Map<string, GraphNode>;
  fetching: Set<string>;
  feed: FeedItem[];
  stats: { crawled: number; found: number; queued: number; errors: number; blocked: number; avgMs: number; words: number; facts: number };
  sitemapUrls: number;
  unblocked: number;
  completed: { pages: number; blocked: number; failed: number; durationMs: number } | null;
  analysis: {
    read: number;
    waiting: number;
    total: number;
    recent: { url: string; facts: Fact[]; count: number }[];
    sections: { section: string; label: string; items: string[] }[];
    brief: string[] | null;
    started: boolean;
  };
  /** Event-clock milliseconds. */
  startedAt: number | null;
  lastEventAt: number | null;
  /** True once any event has arrived. */
  live: boolean;
}

const MAX_FEED = 120;

export const SECTION_TITLES: Record<string, string> = {
  company: "Company",
  products: "Products & services",
  pricing: "Pricing",
  audience: "Audience",
  positioning: "Positioning",
  voice: "Brand voice",
  competitors: "Competitors",
  seo: "SEO",
  channels: "Channels",
  content: "Content",
  integrations: "Integrations",
  trust: "Trust",
  brief: "Business brief",
};
export const sectionTitle = (id: string) => SECTION_TITLES[id] ?? id.replace(/_/g, " ");

export function pathOf(url: string) {
  try {
    const u = new URL(url);
    return u.pathname + u.search || "/";
  } catch {
    return url;
  }
}

function segments(url: string) {
  return pathOf(url).split("?")[0].split("/").filter(Boolean);
}

export function initialState(siteUrl = "", maxPages = 200): CrawlState {
  return {
    phase: "connecting",
    siteUrl,
    maxPages,
    concurrency: null,
    aiConcurrency: null,
    nodes: new Map(),
    fetching: new Set(),
    feed: [],
    stats: { crawled: 0, found: 0, queued: 0, errors: 0, blocked: 0, avgMs: 0, words: 0, facts: 0 },
    sitemapUrls: 0,
    unblocked: 0,
    completed: null,
    analysis: { read: 0, waiting: 0, total: 0, recent: [], sections: [], brief: null, started: false },
    startedAt: null,
    lastEventAt: null,
    live: false,
  };
}

const str = (v: unknown) => (typeof v === "string" ? v : undefined);
const num = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? v : undefined);

/**
 * Mutable store fed by events. The canvas reads `state` every frame; React subscribes
 * via `subscribe`/`getVersion` and is notified at most once per animation frame.
 */
export class CrawlStore {
  state: CrawlState;
  effects: Effect[] = [];
  private version = 0;
  private listeners = new Set<() => void>();
  private scheduled = false;
  private msTotal = 0;
  private msCount = 0;

  constructor(siteUrl: string, maxPages: number) {
    this.state = initialState(siteUrl, maxPages);
  }

  subscribe = (fn: () => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };
  getVersion = () => this.version;

  /** Force a React update (e.g. for the elapsed clock). */
  touch() {
    this.notify();
  }

  private notify() {
    if (this.scheduled) return;
    this.scheduled = true;
    const run = () => {
      this.scheduled = false;
      this.version++;
      for (const l of this.listeners) l();
    };
    if (typeof requestAnimationFrame === "function") requestAnimationFrame(run);
    else setTimeout(run, 16);
  }

  /** Set defaults known from the project before any event arrives. */
  configure(siteUrl: string, maxPages: number) {
    const s = this.state;
    if (!s.live) {
      s.siteUrl = siteUrl || s.siteUrl;
      s.maxPages = maxPages || s.maxPages;
      this.notify();
    }
  }

  reset() {
    const { siteUrl, maxPages } = this.state;
    this.state = initialState(siteUrl, maxPages);
    this.effects = [];
    this.msTotal = 0;
    this.msCount = 0;
  }

  apply(e: KiqrEvent) {
    this.applyOne(e);
    this.notify();
  }

  /** Synthesis never started (older daemon, or nothing to synthesise): settle on done. */
  settle() {
    const s = this.state;
    if (s.phase === "learning" && !s.analysis.started) {
      s.phase = "done";
      this.notify();
    }
  }

  private recount() {
    const s = this.state;
    let queued = 0;
    for (const n of s.nodes.values()) if (n.status === "queued") queued++;
    s.stats.found = s.nodes.size;
    s.stats.queued = queued;
  }

  /** Nearest known ancestor by path, else the root. */
  private guessParent(url: string): GraphNode | undefined {
    const s = this.state;
    let origin = "";
    try {
      origin = new URL(url).origin;
    } catch {
      /* ignore */
    }
    const segs = segments(url);
    for (let i = segs.length - 1; i > 0; i--) {
      const p = `${origin}/${segs.slice(0, i).join("/")}`;
      const hit = s.nodes.get(p) ?? s.nodes.get(p + "/");
      if (hit && hit.id !== url) return hit;
    }
    return s.nodes.values().next().value;
  }

  private ensureRoot(url: string, now: number) {
    const s = this.state;
    if (s.nodes.size || !s.siteUrl || url.replace(/\/$/, "") === s.siteUrl.replace(/\/$/, "")) return;
    s.nodes.set(s.siteUrl, {
      id: s.siteUrl,
      path: "/",
      parent: null,
      depth: 0,
      source: "start",
      // We joined mid-crawl: the start page was already read.
      status: "ok",
      reading: false,
      inbound: 0,
      spawnedAt: now,
      changedAt: now,
    });
  }

  private addNode(url: string, from: string | null, now: number, start = false): GraphNode {
    const s = this.state;
    const existing = s.nodes.get(url);
    if (existing) return existing;
    if (!start) this.ensureRoot(url, now);
    let parent: GraphNode | undefined;
    if (from && from !== "sitemap") parent = s.nodes.get(from);
    if (!parent && s.nodes.size > 0) parent = this.guessParent(url);
    if (parent) parent.inbound++;
    const n: GraphNode = {
      id: url,
      path: pathOf(url),
      parent: parent?.id ?? null,
      depth: parent ? parent.depth + 1 : 0,
      source: from === "sitemap" ? "sitemap" : from ? "link" : "start",
      status: "queued",
      reading: false,
      inbound: 0,
      spawnedAt: now,
      changedAt: now,
    };
    if (!s.siteUrl && !parent) s.siteUrl = url;
    s.nodes.set(url, n);
    return n;
  }

  private pushFeed(item: Omit<FeedItem, "key">) {
    this.state.feed.unshift({ ...item, key: `${item.url}|${item.kind}|${item.t}` });
    if (this.state.feed.length > MAX_FEED) this.state.feed.length = MAX_FEED;
  }

  private applyOne(e: KiqrEvent) {
    const now = performance.now();
    if (e.type === "crawl_started") {
      this.reset();
    }
    const s = this.state;
    const t = Date.parse(e.at) || Date.now();
    const url = str(e.url);
    const pageEvent = e.type.startsWith("page_");
    // Joined mid-crawl: any page event means a crawl is live.
    if (pageEvent || e.type === "crawl_started" || e.type === "crawl_finished" || e.type.startsWith("synthesis_")) {
      s.live = true;
      s.startedAt ??= t;
      s.lastEventAt = t;
    }

    switch (e.type) {
      case "crawl_started":
        s.phase = "discovering";
        s.startedAt = t;
        s.maxPages = num(e.max_pages) ?? s.maxPages;
        s.concurrency = num(e.concurrency) ?? null;
        s.aiConcurrency = num(e.ai_concurrency) ?? null;
        break;

      case "page_discovered": {
        if (!url) break;
        if (s.phase === "connecting") s.phase = "discovering";
        const from = e.from === null || e.from === undefined ? null : String(e.from);
        if (from === "sitemap" && !s.nodes.has(url)) s.sitemapUrls++;
        this.addNode(url, from, now, from === null);
        this.recount();
        break;
      }

      case "page_fetching": {
        if (!url) break;
        if (s.phase === "connecting" || s.phase === "discovering") s.phase = "crawling";
        const n = this.addNode(url, null, now);
        const via = str(e.via);
        if (via === "unblockingapi" && n.via !== "unblockingapi") s.unblocked++;
        n.via = via;
        n.status = via === "unblockingapi" ? "rendering" : "fetching";
        n.changedAt = now;
        s.fetching.add(url);
        this.effects.push({ kind: "packet-out", url, at: now });
        this.recount();
        break;
      }

      case "page_fetched": {
        if (!url) break;
        if (s.phase === "connecting" || s.phase === "discovering") s.phase = "crawling";
        const n = this.addNode(url, null, now);
        s.fetching.delete(url);
        const status = num(e.status) ?? 200;
        const ms = num(e.ms);
        const words = num(e.words);
        const title = str(e.title) ?? null;
        Object.assign(n, { status: status >= 400 ? "error" : "ok", httpStatus: status, title, words, ms, via: str(e.via) ?? n.via, changedAt: now });
        s.stats.crawled++;
        if (ms !== undefined) {
          this.msTotal += ms;
          this.msCount++;
          s.stats.avgMs = Math.round(this.msTotal / this.msCount);
        }
        if (words) s.stats.words += words;
        this.pushFeed({ url, kind: "fetched", status, title, words, ms, via: n.via, t });
        this.effects.push({ kind: "packet-in", url, at: now });
        this.effects.push({ kind: "burst", url, at: now });
        this.recount();
        break;
      }

      case "page_blocked": {
        if (!url) break;
        const n = this.addNode(url, null, now);
        s.fetching.delete(url);
        const reason = str(e.reason) ?? "blocked";
        Object.assign(n, { status: "blocked", reason, changedAt: now });
        s.stats.blocked++;
        this.pushFeed({ url, kind: "blocked", reason, t });
        this.effects.push({ kind: "burst", url, color: "#fbbf24", at: now });
        this.recount();
        break;
      }

      case "page_failed": {
        if (!url) break;
        const n = this.addNode(url, null, now);
        s.fetching.delete(url);
        const reason = str(e.error) ?? "failed";
        Object.assign(n, { status: "error", reason, changedAt: now });
        s.stats.errors++;
        this.pushFeed({ url, kind: "failed", reason, t });
        this.recount();
        break;
      }

      case "page_queued": {
        if (!url) break;
        const n = s.nodes.get(url);
        if (n) {
          n.reading = true;
          n.changedAt = now;
        }
        s.analysis.waiting = num(e.waiting) ?? s.analysis.waiting;
        break;
      }

      case "page_parsed": {
        if (!url) break;
        const n = s.nodes.get(url);
        const facts = num(e.facts) ?? 0;
        const preview = (Array.isArray(e.preview) ? e.preview : []) as Fact[];
        s.analysis.read++;
        s.analysis.waiting = num(e.waiting) ?? s.analysis.waiting;
        s.stats.facts += facts;
        s.analysis.recent = [{ url, facts: preview, count: facts }, ...s.analysis.recent.filter((r) => r.url !== url)].slice(0, 30);
        if (n) {
          n.reading = false;
          n.facts = facts;
          n.section = preview[0]?.section;
          n.changedAt = now;
        }
        this.effects.push({ kind: "burst", url, color: "#818cf8", at: now });
        break;
      }

      case "crawl_finished": {
        const pages = num(e.pages) ?? s.stats.crawled;
        s.completed = {
          pages,
          blocked: num(e.blocked) ?? s.stats.blocked,
          failed: num(e.failed) ?? s.stats.errors,
          durationMs: Math.max(0, t - (s.startedAt ?? t)),
        };
        s.stats.queued = 0;
        s.fetching.clear();
        for (const n of s.nodes.values()) {
          if (n.status === "queued" || n.status === "fetching" || n.status === "rendering") {
            n.status = "skipped";
            n.changedAt = now;
          }
        }
        s.phase = "learning";
        break;
      }

      case "synthesis_started":
        s.phase = "learning";
        s.analysis.started = true;
        s.analysis.total = num(e.total) ?? 0;
        break;

      case "profile_updated": {
        // Only synthesis carries a title and items; per-page extraction and user edits don't.
        const items = Array.isArray(e.items) ? (e.items as unknown[]).map(String) : null;
        const section = str(e.section);
        if (!items || !section) break;
        if (section === "brief") {
          s.analysis.brief = items;
          break;
        }
        const label = str(e.title) ?? sectionTitle(section);
        s.analysis.sections = [...s.analysis.sections.filter((x) => x.section !== section), { section, label, items }];
        break;
      }

      case "synthesis_finished":
        s.phase = "done";
        break;
    }
  }
}
