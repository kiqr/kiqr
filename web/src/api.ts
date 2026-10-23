import { useEffect, useRef, useState } from "react";
import { DEMO, demoProject, demoState, runDemo } from "./demo";
import { demoBrand, demoBrief } from "./demo-brief";

export type Project = {
  slug: string;
  name: string;
  url: string;
  created_at: string;
  crawl: { max_pages: number | null; concurrency: number | null; ai_concurrency: number | null; include: string[]; exclude: string[] };
  last_crawl: { finished_at: string; pages: number; blocked: number; failed: number } | null;
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

export type KiqrEvent = { at: string; project: string | null; type: string } & Record<string, unknown>;

export type Health = {
  ok: boolean;
  version: string;
  home: string;
  providers: { task: string; model: string; available: boolean }[];
  unblockingapi: boolean;
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
  projects: () => req<Project[]>("/api/projects"),
  project: (slug: string) => req<Project>(`/api/projects/${slug}`),
  createProject: (url: string) => req<Project>("/api/projects", { method: "POST", body: JSON.stringify({ url }) }),
  deleteProject: (slug: string) => req<void>(`/api/projects/${slug}`, { method: "DELETE" }),
  crawl: (slug: string) => req<{ started: boolean }>(`/api/projects/${slug}/crawl`, { method: "POST", body: "{}" }),
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
  history: (slug: string) => req<{ hash: string; at: string; message: string }[]>(`/api/projects/${slug}/history`),
};

function demoRequest<T>(path: string, init?: RequestInit): Promise<T> {
  if (path === "/api/projects" && init?.method === "POST") {
    demoState.created = true;
    return Promise.resolve(demoProject as T);
  }
  if (path === "/api/projects") return Promise.resolve((demoState.created ? [demoProject] : []) as T);
  if (path.startsWith("/api/projects/") && path.split("/").length === 4) return Promise.resolve(demoProject as T);
  if (path.endsWith("/brief")) return Promise.resolve((init?.method === "PUT" ? JSON.parse(String(init.body)) : demoState.created ? demoBrief : null) as T);
  if (path.endsWith("/brand")) return demoState.created ? Promise.resolve(demoBrand as T) : Promise.reject(new Error("404"));
  if (path.endsWith("/profile")) return Promise.resolve([] as T);
  if (path === "/api/health")
    return Promise.resolve({ ok: true, version: "demo", home: "~/.kiqr", providers: [{ task: "extract", model: "claude-cli/haiku", available: true }], unblockingapi: false } as T);
  return Promise.resolve({} as T);
}

/** Subscribe to the daemon's event stream, optionally for one project. */
export function useEvents(project: string | null, onEvent: (e: KiqrEvent) => void) {
  const handler = useRef(onEvent);
  handler.current = onEvent;
  useEffect(() => {
    if (DEMO) return runDemo(project ?? demoProject.slug, (e) => handler.current(e));
    const url = project ? `/api/events?project=${encodeURIComponent(project)}` : "/api/events";
    const es = new EventSource(url);
    es.onmessage = (m) => {
      try {
        handler.current(JSON.parse(m.data));
      } catch {
        /* ignore malformed frames */
      }
    };
    return () => es.close();
  }, [project]);
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
