import { useEffect, useRef, useState } from "react";
import { DEMO, demoProject, demoState, runDemo } from "./demo";
import { demoAssetFile, demoAssetRequest, demoAssetText } from "./demo-assets";
import { demoBrand, demoBrief } from "./demo-brief";
import { demoSeemoRequest, onDemoSeemo } from "./demo-chat";

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
  /** Seemo's sessions: spend ceiling per turn, how many run at once, the timezone for schedules. */
  agent?: { max_budget_usd: number; max_concurrent: number; timezone: string };
  /** Which connection serves an ability when several are ready: `{ image: "fal-image" }`. */
  abilities?: Record<string, string>;
};

// ---- Seemo: chats, approvals, tasks

/** A tool Seemo used while writing a message. `ok` is null while it runs. */
export type Step = { tool: string; label: string; ok: boolean | null; note?: string; image?: string };
/** A card in a chat: an approval Seemo is waiting for, which becomes a task card once approved. */
export type Card = {
  kind: "approval" | "task" | string;
  approval?: string;
  approval_kind?: "task" | "config" | string;
  status: "pending" | "approved" | "declined" | "expired" | string;
  task?: string;
  payload: Record<string, unknown>;
  result?: Record<string, unknown>;
};
export type Message = {
  id: string;
  at: string;
  role: "user" | "seemo" | "card" | "note";
  text: string;
  steps?: Step[];
  card?: Card;
  kind?: string;
  error?: string;
};
export type Chat = { id: string; title: string; created_at: string; updated_at: string; session_id: string | null; model: string; last_message: string; messages: number };
/** A reply in progress, so a client that joins late catches up. */
export type Streaming = { message: string; text: string; steps: Step[]; started: string };
export type ChatView = Chat & { messages: Message[]; streaming: Streaming | null };

export type Schedule = { kind: "once" | "rrule"; at?: string | null; rrule?: string | null; tz?: string; text?: string };
export type RunStatus = "running" | "ok" | "failed" | "missed";
export type Task = {
  id: string;
  title: string;
  instruction: string;
  schedule: Schedule;
  schedule_text: string;
  allowed: string[];
  allowed_labels: string[];
  status: "active" | "paused" | "done";
  created_by: string;
  follow_up_chat?: string | null;
  created_at: string;
  next_run: string | null;
  last_run: string | null;
  last_status: RunStatus | null;
  runs: number;
  running: boolean;
};
export type Run = {
  id: string;
  task: string;
  started: string;
  finished: string | null;
  status: RunStatus;
  trigger: string;
  summary: string;
  steps: Step[];
  assets: string[];
  session_id: string | null;
  cost_usd: number | null;
};
export type CalendarItem = { task: string; title: string; at: string; schedule_text: string };
export type ToolGroup = { id: string; label: string; tools: string[] };

// ---- Skills and connections
export type Scope = { scope: "global" } | { scope: "project"; project: string };
/** A folder under skills/: a skill (SKILL.md), a connection (extension.yaml), or both. */
export type Skill = {
  name: string;
  title: string;
  description: string;
  abilities: string[];
  channels: string[];
  scope: Scope;
  builtin: boolean;
  has_skill: boolean;
  /** One of KIQR's own pipeline prompts (reading pages, building the profile), not something Seemo picks up in a chat. */
  pipeline?: boolean;
  connection: { id: string; name: string; ready: boolean } | null;
  error: string | null;
  folder: string;
};
export type ConnectionAuth = { env: string; label: string; help: string; optional: boolean; set: boolean };
/** What provides an ability: a service behind a key, a program here, or KIQR itself. */
export type Connection = {
  id: string;
  name: string;
  version: string;
  summary: string;
  transport: "http" | "command" | "mcp" | "builtin";
  homepage: string;
  builtin: boolean;
  folder: string;
  scope: Scope;
  ready: boolean;
  missing: string[];
  auth: ConnectionAuth[];
  provides: { ability: string; label: string; cost: string | null }[];
  tools: { name: string; label: string; cost: string | null }[];
  channels: { kind: string; label: string }[];
  hosts: string[];
  command: string | null;
  env: string[];
  program_found: boolean | null;
};
/** Where a project publishes. */
export type Channel = {
  id: string;
  label: string;
  extension: string;
  kind: string;
  destination: Record<string, unknown>;
  created_by: string;
  created_at: string;
  asks_first: boolean;
  publications: number;
  last_published?: string | null;
  connection_name: string;
  ready: boolean;
  destination_text: string;
};
export type ChannelKindInfo = { kind: string; label: string; connection: string; connection_name: string; ready: boolean; destination: Record<string, unknown> };
export type AbilityInfo = { id: string; label: string; tool: string; configured: string | null; active: { id: string; name: string } | null; connections: { id: string; name: string; ready: boolean }[] };
export type Extensions = { connections: Connection[]; broken: { folder: string; scope: Scope; error: string }[]; abilities: AbilityInfo[] };

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

  // Seemo
  chats: (slug: string) => req<Chat[]>(`/api/projects/${slug}/chats`),
  createChat: (slug: string, title?: string) => req<Chat>(`/api/projects/${slug}/chats`, { method: "POST", body: JSON.stringify({ title: title ?? "" }) }),
  chat: (slug: string, id: string) => req<ChatView>(`/api/projects/${slug}/chats/${id}`),
  renameChat: (slug: string, id: string, title: string) => req<Chat>(`/api/projects/${slug}/chats/${id}`, { method: "PUT", body: JSON.stringify({ title }) }),
  deleteChat: (slug: string, id: string) => req<void>(`/api/projects/${slug}/chats/${id}`, { method: "DELETE" }),
  /** Say something; Seemo's reply streams as `chat_delta`/`chat_tool` events and lands as `chat_message`. */
  sendMessage: (slug: string, id: string, text: string) => req<{ message: string; reply: string }>(`/api/projects/${slug}/chats/${id}/messages`, { method: "POST", body: JSON.stringify({ text }) }),
  stopChat: (slug: string, id: string) => req<{ stopping: boolean }>(`/api/projects/${slug}/chats/${id}/stop`, { method: "POST", body: "{}" }),
  resolveApproval: (slug: string, id: string, decision: "approve" | "decline", allowed?: string[]) =>
    req<{ id: string; status: string; result: Record<string, unknown> }>(`/api/projects/${slug}/approvals/${id}`, { method: "POST", body: JSON.stringify({ decision, allowed }) }),
  tasks: (slug: string) => req<Task[]>(`/api/projects/${slug}/tasks`),
  task: (slug: string, id: string) => req<Task>(`/api/projects/${slug}/tasks/${id}`),
  runs: (slug: string, id: string) => req<Run[]>(`/api/projects/${slug}/tasks/${id}/runs`),
  runTask: (slug: string, id: string) => req<{ started: boolean; run: string }>(`/api/projects/${slug}/tasks/${id}/run`, { method: "POST", body: "{}" }),
  pauseTask: (slug: string, id: string) => req<Task>(`/api/projects/${slug}/tasks/${id}/pause`, { method: "POST", body: "{}" }),
  resumeTask: (slug: string, id: string) => req<Task>(`/api/projects/${slug}/tasks/${id}/resume`, { method: "POST", body: "{}" }),
  deleteTask: (slug: string, id: string) => req<void>(`/api/projects/${slug}/tasks/${id}`, { method: "DELETE" }),
  calendar: (slug: string, from?: string, to?: string) =>
    req<CalendarItem[]>(`/api/projects/${slug}/calendar${from ? `?from=${encodeURIComponent(from)}${to ? `&to=${encodeURIComponent(to)}` : ""}` : ""}`),
  tools: () => req<{ groups: ToolGroup[] }>("/api/tools"),

  // Skills and connections
  /** Skills a project sees (its own first), or every folder when `slug` is empty. */
  skills: (slug?: string) => req<Skill[]>(slug ? `/api/projects/${slug}/skills` : "/api/skills"),
  skillText: (slug: string, name: string) => (DEMO ? demoSeemoRequest<string>(`/api/projects/${slug}/skills/${name}`) : fetch(`/api/projects/${slug}/skills/${encodeURIComponent(name)}`).then((r) => (r.ok ? r.text() : Promise.reject(new Error(`${r.status}`))))),
  /** Write a SKILL.md: for one project (`slug`) or everywhere (`null`). */
  saveSkill: (slug: string | null, name: string, text: string) => req<Skill>(slug ? `/api/projects/${slug}/skills/${name}` : `/api/skills/${name}`, { method: "PUT", body: JSON.stringify({ text }) }),
  deleteSkill: (slug: string | null, name: string) => req<void>(slug ? `/api/projects/${slug}/skills/${name}` : `/api/skills/${name}`, { method: "DELETE" }),
  /** Install a skill folder (a path on this machine or a git URL). */
  installSkill: (source: string, slug?: string, name?: string) => req<Skill>(slug ? `/api/projects/${slug}/skills` : "/api/skills", { method: "POST", body: JSON.stringify({ source, name }) }),
  extensions: () => req<Extensions>("/api/extensions"),
  removeConnection: (id: string, slug?: string) => req<void>(`/api/extensions/${id}${slug ? `?project=${encodeURIComponent(slug)}` : ""}`, { method: "DELETE" }),
  /** Set a secret in .env by name; an empty value clears it. The value is never read back. */
  setSecret: (name: string, value: string) => req<{ name: string; set: boolean }>(`/api/secrets/${name}`, { method: "PUT", body: JSON.stringify({ value }) }),

  // Channels and publishing
  channels: (slug: string) => req<{ channels: Channel[]; kinds: ChannelKindInfo[] }>(`/api/projects/${slug}/channels`),
  createChannel: (slug: string, c: { label: string; kind: string; connection?: string; destination: Record<string, string> }) => req<Channel>(`/api/projects/${slug}/channels`, { method: "POST", body: JSON.stringify(c) }),
  deleteChannel: (slug: string, id: string) => req<void>(`/api/projects/${slug}/channels/${id}`, { method: "DELETE" }),
  /** Publish an asset through a channel. From the UI the click is the yes. */
  publishAsset: (slug: string, a: Pick<Asset, "type" | "id">, channel: string, note = "") =>
    req<{ published: boolean; url: string | null; detail: string; channel_label: string }>(`/api/projects/${slug}/assets/${a.type}/${a.id}/publish`, { method: "POST", body: JSON.stringify({ channel, note, by: "user" }) }),
  sendEmail: (slug: string, m: { to: string; subject: string; text: string; asset?: { type: string; id: string } }) => req<{ sent: boolean; to: string; subject: string; connection: string }>(`/api/projects/${slug}/email`, { method: "POST", body: JSON.stringify(m) }),
};

let demoConfig: Config = {
  daemon: { host: "127.0.0.1", port: 7777 },
  crawl: { max_pages: 200, concurrency: 5, ai_concurrency: 3, escalate_to_unblockingapi: true },
  models: { extract: "claude-cli/haiku", synthesize: "claude-cli/sonnet", chat: "claude-cli/sonnet" },
  agent: { max_budget_usd: 2, max_concurrent: 3, timezone: "Europe/Stockholm" },
  abilities: {},
};

function demoRequest<T>(path: string, init?: RequestInit): Promise<T> {
  if (/\/(chats|approvals|tasks|calendar|skills|channels|email)(\/|\?|$)/.test(path) || /\/publish$/.test(path) || path === "/api/tools" || path.startsWith("/api/extensions") || path.startsWith("/api/secrets")) return demoSeemoRequest<T>(path, init);
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
    return subscribe(url, (e) => handler.current(e), key);
  }, [project, replay, key, demo]);
}

/**
 * One EventSource per URL, shared by every listener: browsers allow six
 * connections per origin and a chat page with the widget open has more
 * listeners than that. `key` forces a fresh connection (a replay) for all.
 */
const sources = new Map<string, { es: EventSource; key: number; listeners: Set<(e: KiqrEvent) => void> }>();
function subscribe(url: string, listener: (e: KiqrEvent) => void, key: number) {
  let src = sources.get(url);
  if (src && src.key !== key) {
    src.es.close();
    sources.delete(url);
    src = undefined;
  }
  if (!src) {
    const es = new EventSource(url);
    const entry = { es, key, listeners: new Set<(e: KiqrEvent) => void>() };
    es.onmessage = (m) => {
      let ev: KiqrEvent;
      try {
        ev = JSON.parse(m.data);
      } catch {
        return;
      }
      for (const l of entry.listeners) l(ev);
    };
    sources.set(url, entry);
    src = entry;
  }
  src.listeners.add(listener);
  return () => {
    const cur = sources.get(url);
    if (!cur) return;
    cur.listeners.delete(listener);
    if (cur.listeners.size === 0) {
      cur.es.close();
      sources.delete(url);
    }
  };
}

/** Demo mode: chat events come from the scripted conversation. */
export function useDemoSeemo(onEvent: (e: KiqrEvent) => void) {
  const handler = useRef(onEvent);
  handler.current = onEvent;
  useEffect(() => {
    if (!DEMO) return;
    return onDemoSeemo((e) => handler.current(e));
  }, []);
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
