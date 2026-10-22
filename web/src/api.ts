import { useEffect, useRef, useState } from "react";

export type Project = {
  slug: string;
  name: string;
  url: string;
  created_at: string;
  crawl: { max_pages: number | null; include: string[]; exclude: string[] };
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

export type KiqrEvent = { at: string; project: string | null; type: string } & Record<string, unknown>;

export type Health = {
  ok: boolean;
  version: string;
  home: string;
  providers: { task: string; model: string; available: boolean }[];
  unblockingapi: boolean;
};

async function req<T>(path: string, init?: RequestInit): Promise<T> {
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
  synthesize: (slug: string) => req<{ started: boolean }>(`/api/projects/${slug}/synthesize`, { method: "POST", body: "{}" }),
  profile: (slug: string) => req<Section[]>(`/api/projects/${slug}/profile`),
  saveSection: (slug: string, section: Section) =>
    req<Section>(`/api/projects/${slug}/profile/${section.id}`, { method: "PUT", body: JSON.stringify(section) }),
  history: (slug: string) => req<{ hash: string; at: string; message: string }[]>(`/api/projects/${slug}/history`),
};

/** Subscribe to the daemon's event stream, optionally for one project. */
export function useEvents(project: string | null, onEvent: (e: KiqrEvent) => void) {
  const handler = useRef(onEvent);
  handler.current = onEvent;
  useEffect(() => {
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
