import { useEffect, useRef, useState } from "react";
import { DEMO, demoProject, demoState, runDemo } from "./demo";
import { demoAssetFile, demoAssetRequest, demoAssetText } from "./demo-assets";
import { demoBrand, demoBrief } from "./demo-brief";

export type Project = {
  slug: string;
  name: string;
  url: string;
  created_at: string;
  crawl: { max_pages: number | null; concurrency: number | null; ai_concurrency: number | null; include: string[]; exclude: string[] };
  last_crawl: { finished_at: string; pages: number; blocked: number; failed: number } | null;
  /** Jobs running on the project now: "crawl", "synthesize", "brief". */
  jobs?: string[];
};

export type Evidence = { url: string; quote: string };
export type Field = {
  name: string;
  value: string;
  evidence: Evidence[];
  confidence: number;
  locked: boolean;
  edited_at: string | null;
};
export type Section = {
  id: string;
  title: string;
  description: string;
  fields: Field[];
  updated_at: string | null;
};

export type Offering = { name: string; kind: "product" | "service"; description: string; url?: string; sources?: string[] };
export type Audience = { name: string; needs: string[] };
export type ContentGap = { topic: string; rationale: string; intent: "informational" | "commercial" | "transactional" | "navigational" };
export type TopicCluster = { label: string; description: string; page_count: number; urls?: string[] };
/** The structured business profile: what the business sells, to whom, how it sounds, and what it has and hasn't written about. */
export type Brief = {
  version: number;
  source: "crawl" | "edited";
  updated_at: string;
  business_name: string;
  summary: string;
  language: string;
  geographies: string[];
  offerings: Offering[];
  audiences: Audience[];
  tone_of_voice: { description: string; traits: string[]; examples: string[]; avoid: string[] };
  brand_terms: string[];
  differentiators: string[];
  competitors: { name: string; note: string }[];
  pricing_summary: string;
  topic_clusters: TopicCluster[];
  content_gaps: ContentGap[];
};

/** The business's own look, collected from the crawled pages: logo, colours, fonts, imagery. */
export type Brand = {
  site_name: string;
  tagline: string;
  logo: string | null;
  icon: string | null;
  cover: string | null;
  colors: string[];
  fonts: string[];
  images: { url: string; alt: string; page: string }[];
  social: { network: string; url: string }[];
  updated_at: string;
};

/** A kind of asset. Types are data: built-ins ship with KIQR, users and Seemo add more. */
export type AssetType = {
  id: string;
  name: string;
  plural: string;
  /** A lucide icon name, kebab-case. */
  icon: string;
  description: string;
  preview: "article" | "post" | "image" | "video" | "audio" | "campaign" | "files" | string;
  files: { role: string; accept: string[] }[];
  builtin: boolean;
};
export type AssetStatus = "draft" | "ready" | "scheduled" | "published" | "archived";
/** Something made with Seemo: an article, a post, a meme, a video… */
export type Asset = {
  id: string;
  type: string;
  title: string;
  status: AssetStatus;
  tags: string[];
  summary?: string;
  created: { at: string; by: string };
  updated_at: string;
  files: { role: string; path: string; bytes: number; sha256: string }[];
  source?: Record<string, unknown>;
  publications: { connector: string; account?: string; url?: string; at: string; run?: string }[];
};
export type TypeCount = { type: string; count: number };
export type NewAsset = { type: string; title: string; status?: AssetStatus; tags?: string[]; summary?: string; source?: Record<string, unknown>; files?: { role: string; name: string; text: string }[] };

export type KiqrEvent = { at: string; project: string | null; type: string } & Record<string, unknown>;

export type Health = {
  ok: boolean;
  version: string;
  home: string;
  providers: { task: string; model: string; available: boolean }[];
  unblockingapi: boolean;
  /** Which secrets are set in `.env`. Names only; values never leave the daemon. */
  secrets?: { name: string; set: boolean }[];
  /** Model command-line tools found on this machine. */
  tools?: { name: string; installed: boolean }[];
  env_file?: string;
  address?: string;
};
/** `config.yaml`. */
export type Config = {
  daemon: { host: string; port: number };
  crawl: { max_pages: number; concurrency: number; ai_concurrency: number; escalate_to_unblockingapi: boolean };
  models: { extract: string; synthesize: string; chat: string };
};

async function req<T>(path: string, init?: RequestInit): Promise<T> {
  if (DEMO) return demoRequest<T>(path, init);
  const res = await fetch(path, { headers: { "Content-Type": "application/json" }, ...init });
  if (res.status === 204) return undefined as T;
  const body = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(body.error ?? `${res.status} ${res.statusText}`);
  return body as T;
}

export const api = {
  health: () => req<Health>("/api/health"),
  config: () => req<Config>("/api/config"),
  saveConfig: (c: Config) => req<Config>("/api/config", { method: "PUT", body: JSON.stringify(c) }),
  projects: () => req<Project[]>("/api/projects"),
  project: (slug: string) => req<Project>(`/api/projects/${slug}`),
  createProject: (url: string) => req<Project>("/api/projects", { method: "POST", body: JSON.stringify({ url }) }),
  deleteProject: (slug: string) => req<void>(`/api/projects/${slug}`, { method: "DELETE" }),
  crawl: (slug: string) => req<{ started: boolean }>(`/api/projects/${slug}/crawl`, { method: "POST", body: "{}" }),
  /** Set the page limit; `live` when the running crawl picked it up. */
  setCrawlLimit: (slug: string, max_pages: number) =>
    req<{ max_pages: number; live: boolean }>(`/api/projects/${slug}/crawl/limit`, { method: "PUT", body: JSON.stringify({ max_pages }) }),
  crawlSettings: (slug: string, s: { max_pages?: number; concurrency?: number; ai_concurrency?: number }) =>
    req<Project>(`/api/projects/${slug}/settings`, { method: "PUT", body: JSON.stringify(s) }),
  synthesize: (slug: string) => req<{ started: boolean }>(`/api/projects/${slug}/synthesize`, { method: "POST", body: "{}" }),
  profile: (slug: string) => req<Section[]>(`/api/projects/${slug}/profile`),
  saveSection: (slug: string, section: Section) =>
    req<Section>(`/api/projects/${slug}/profile/${section.id}`, { method: "PUT", body: JSON.stringify(section) }),
  brief: (slug: string) => req<Brief | null>(`/api/projects/${slug}/brief`),
  saveBrief: (slug: string, brief: Brief) => req<Brief>(`/api/projects/${slug}/brief`, { method: "PUT", body: JSON.stringify(brief) }),
  rebuildBrief: (slug: string) => req<{ started: boolean }>(`/api/projects/${slug}/brief`, { method: "POST", body: "{}" }),
  brand: (slug: string) => req<Brand>(`/api/projects/${slug}/brand`).catch(() => null),
  rebuildBrand: (slug: string) => req<Brand>(`/api/projects/${slug}/brand`, { method: "POST", body: "{}" }),
  assetTypes: () => req<AssetType[]>("/api/asset-types"),
  assets: (slug: string, type?: string) => req<Asset[]>(`/api/projects/${slug}/assets${type ? `?type=${encodeURIComponent(type)}` : ""}`),
  assetSummary: (slug: string) => req<TypeCount[]>(`/api/projects/${slug}/assets/summary`),
  /** Have Seemo write assets from the business profile, in the background (posts for now). */
  generateAssets: (slug: string, type: string, count: number) =>
    req<{ started: boolean }>(`/api/projects/${slug}/assets/generate`, { method: "POST", body: JSON.stringify({ type, count }) }),
  asset: (slug: string, type: string, id: string) => req<Asset>(`/api/projects/${slug}/assets/${type}/${id}`),
  createAsset: (slug: string, a: NewAsset) => req<Asset>(`/api/projects/${slug}/assets`, { method: "POST", body: JSON.stringify(a) }),
  saveAsset: (slug: string, a: Asset) => req<Asset>(`/api/projects/${slug}/assets/${a.type}/${a.id}`, { method: "PUT", body: JSON.stringify(a) }),
  deleteAsset: (slug: string, type: string, id: string) => req<void>(`/api/projects/${slug}/assets/${type}/${id}`, { method: "DELETE" }),
  /** Where the browser loads one of an asset's files. */
  assetFile: (slug: string, a: Pick<Asset, "type" | "id">, name: string) => (DEMO ? demoAssetFile(a, name) : `/api/projects/${slug}/assets/${a.type}/${a.id}/files/${encodeURIComponent(name)}`),
  /** The text of an asset's text file (article body, post copy). */
  assetText: (slug: string, a: Pick<Asset, "type" | "id">, name: string) =>
    DEMO ? Promise.resolve(demoAssetText(a, name)) : fetch(api.assetFile(slug, a, name)).then((r) => (r.ok ? r.text() : Promise.reject(new Error(`${r.status}`)))),
  history: (slug: string) => req<{ hash: string; at: string; message: string }[]>(`/api/projects/${slug}/history`),
};

let demoConfig: Config = {
  daemon: { host: "127.0.0.1", port: 7777 },
  crawl: { max_pages: 200, concurrency: 5, ai_concurrency: 3, escalate_to_unblockingapi: true },
  models: { extract: "claude-cli/haiku", synthesize: "claude-cli/sonnet", chat: "claude-cli/sonnet" },
};

function demoRequest<T>(path: string, init?: RequestInit): Promise<T> {
  if (path === "/api/projects" && init?.method === "POST") {
    demoState.created = true;
    return Promise.resolve(demoProject as T);
  }
  if (path === "/api/projects") return Promise.resolve((demoState.created ? [demoProject] : []) as T);
  if (path.startsWith("/api/projects/") && path.split("/").length === 4) return Promise.resolve(demoProject as T);
  if (path === "/api/asset-types" || path.includes("/assets")) return demoAssetRequest<T>(path, init);
  if (path.endsWith("/brief")) return Promise.resolve((init?.method === "PUT" ? JSON.parse(String(init.body)) : demoState.created ? demoBrief : null) as T);
  if (path.endsWith("/crawl/limit")) return Promise.resolve({ max_pages: JSON.parse(String(init?.body)).max_pages, live: true } as T);
  if (path.endsWith("/brand")) return demoState.created ? Promise.resolve(demoBrand as T) : Promise.reject(new Error("404"));
  if (path.endsWith("/profile")) return Promise.resolve([] as T);
  if (path === "/api/config") {
    if (init?.method === "PUT") demoConfig = JSON.parse(String(init.body));
    return Promise.resolve(demoConfig as T);
  }
  if (path === "/api/health")
    return Promise.resolve({
      ok: true,
      version: "demo",
      home: "~/.kiqr",
      env_file: "~/.kiqr/.env",
      address: "127.0.0.1:7777",
      providers: (["extract", "synthesize", "chat"] as const).map((task) => ({ task, model: demoConfig.models[task], available: !demoConfig.models[task].startsWith("openai/") })),
      unblockingapi: true,
      secrets: ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN", "UNBLOCKINGAPI_KEY", "KIQR_CLOUD_TOKEN"].map((name) => ({ name, set: name === "UNBLOCKINGAPI_KEY" || name === "ANTHROPIC_API_KEY" })),
      tools: [
        { name: "claude", installed: true },
        { name: "codex", installed: false },
      ],
    } as T);
  return Promise.resolve({} as T);
}

/**
 * Subscribe to the daemon's event stream, optionally for one project. With `replay`,
 * the daemon first sends the project's latest crawl job (events marked `replay: true`),
 * then `replay_done` with `running`, then live events; it does so again on reconnect.
 * Changing `key` reconnects.
 */
export function useEvents(project: string | null, onEvent: (e: KiqrEvent) => void, opts: { replay?: boolean; key?: number; demo?: boolean } = {}) {
  const handler = useRef(onEvent);
  handler.current = onEvent;
  const { replay = false, key = 0, demo = true } = opts;
  useEffect(() => {
    if (DEMO) {
      // The demo's scripted crawl runs for the screens that show it, not for every listener.
      if (!demo) return;
      if (replay) handler.current({ type: "replay_done", project, at: new Date().toISOString(), running: false });
      return runDemo(project ?? demoProject.slug, (e) => handler.current(e));
    }
    const url = project ? `/api/events?project=${encodeURIComponent(project)}${replay ? "&replay=1" : ""}` : "/api/events";
    const es = new EventSource(url);
    es.onmessage = (m) => {
      try {
        handler.current(JSON.parse(m.data));
      } catch {
        /* ignore malformed frames */
      }
    };
    return () => es.close();
  }, [project, replay, key, demo]);
}

export function useAsync<T>(fn: () => Promise<T>, deps: unknown[]) {
  const [state, set] = useState<{ data?: T; error?: string; loading: boolean }>({ loading: true });
  const reload = () => {
    set((s) => ({ ...s, loading: true }));
    fn().then((data) => set({ data, loading: false })).catch((e) => set({ error: String(e.message ?? e), loading: false }));
  };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(reload, deps);
  return { ...state, reload };
}
