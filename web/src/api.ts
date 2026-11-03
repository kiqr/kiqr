import { useEffect, useRef, useState } from "react";
import { DEMO, demoProject, demoState, runDemo } from "./demo";
import { demoAssetFile, demoAssetRequest, demoAssetText, demoMediaRequest } from "./demo-assets";
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
  /** The first build: until it is `ready` the project stays in the welcome flow. */
  build?: Build;
};

export type BuildStage = "new" | "crawling" | "reading" | "writing" | "ready" | "failed";
/** A project's first build: crawl, read, write the profile. One overall percentage, the stage, the time left. */
export type Build = {
  stage: BuildStage;
  pct: number;
  eta_secs: number | null;
  max_pages: number;
  found: number;
  fetched: number;
  queued: number;
  read: number;
  facts: number;
  sections_done: number;
  sections_total: number;
  running: boolean;
  started_at: string | null;
  last_at: string | null;
  problem: string | null;
};

/** Why Seemo suggests an ability: it saw the service on the site, the owner named a network it publishes to, or it serves their goal. */
export type SetupWhy = { kind: "site" } | { kind: "every_site" } | { kind: "network"; name: string } | { kind: "goal"; goal: string };
/** An ability worth setting up for this project, from the welcome answers and the crawl. */
export type SetupSuggestion = { id: string; name: string; summary: string; icon: string; avatar: string | null; why: SetupWhy[]; installed: boolean; on: boolean; ready: boolean };
/** The suggestions, and the networks the owner named that no ability publishes to yet. */
export type Setup = { suggestions: SetupSuggestion[]; waiting: string[] };

/** What the owner told Seemo while it first read the site. */
export type Intake = {
  goal: "" | "customers" | "repeat" | "known" | "launch";
  goal_note: string;
  customers: string;
  time: "" | "lt1" | "1to3" | "3plus";
  networks: string[];
  skipped: string[];
  /** Suggested abilities the owner doesn't want (ids). */
  dismissed?: string[];
  updated_at?: string;
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
  /** Whether its assets carry an image by default: an article's cover always, a post's image usually. */
  image: "always" | "usually" | "none";
  builtin: boolean;
};
/** The brand as the renderer sees it: colours with roles, the site name, the logo. */
export type BrandKit = {
  site_name: string;
  palette: { dark: string; light: string; accent: string };
  logo: string | null;
  html_available: boolean;
  /** The business's own pictures in the Library (the `media` type), for the renderer to place. */
  media?: BrandMedia[];
};
/** One media asset as the brand kit hands it to a renderer: its image file and what is known about it. */
export type BrandMedia = { asset: string; title: string; file: string; url: string; alt: string; page: string | null; origin: string; width: number | null; height: number | null; orientation: "landscape" | "portrait" | "square" | null };
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
  /** Image files carry their pixel size once the daemon has read it. */
  files: { role: string; path: string; bytes: number; sha256: string; width?: number; height?: number }[];
  source?: Record<string, unknown>;
  /** The channel the user approved it for (Ready); when the daemon publishes it (Scheduled); why a scheduled publish failed. */
  target?: string | null;
  publish_at?: string | null;
  publish_error?: string | null;
  /** The last click of a publish was made but never seen to arrive: it may have gone out. */
  publish_attempt?: { at: string; channel?: string; note?: string } | null;
  archived?: boolean;
  publications: Publication[];
};
/** Where an asset went: the channel (empty for an email to one address), the account it went out as, the ability that sent it. */
export type Publication = { channel?: string; account: string; ability?: string; from?: string | null; to?: string | null; url?: string | null; at: string; run?: string | null };
export type TypeCount = { type: string; count: number };
/** Where the business's own pictures come from: the website (always first), then abilities that connect a folder, a drive or a bucket. */
export type MediaSource = { id: string; name: string; icon: string; builtin: boolean; ready: boolean; found?: number; imported: number; note: string };
/** One picture a source offers. `imported` is the media asset's id once it is in the Library; `url` is the picture itself (the website) or a thumbnail. */
export type MediaItem = { source: string; id: string; name: string; mime: string; bytes: number | null; modified: string | null; url: string | null; alt: string; page: string | null; imported: string | null };
export type MediaImport = { ok: boolean; new: boolean; asset: { type: string; id: string; title: string }; picture: Record<string, unknown> | null; note: string };
export type NewAsset = { type: string; title: string; tags?: string[]; summary?: string; source?: Record<string, unknown>; files?: { role: string; name: string; text: string }[] };

export type KiqrEvent = { at: string; project: string | null; type: string } & Record<string, unknown>;
/** Whether an account works changed (`account_state_changed`): its sign-in on a hosted program. */
export type CredentialStateEvent = { type: "credential_state_changed"; credential: string; state: SessionState; message: string };
/** One picture of the live view (`hands_frame`), a few a second while someone watches. Never kept. */
export type HandsFrameEvent = { type: "hands_frame"; credential: string; hands: string; mime: string; data: string; url?: string };
export type HandsStartedEvent = { type: "hands_started"; credential: string; hands: string };
export type HandsEndedEvent = { type: "hands_ended"; credential: string; hands: string; message: string };
/** A program KIQR installs for an ability (machine-wide events, on the stream without a project). */
export type ProgramInstallingEvent = { type: "program_installing"; program: string; version: string; note: string };
export type ProgramInstalledEvent = { type: "program_installed"; program: string; version: string };
export type ProgramFailedEvent = { type: "program_failed"; program: string; version: string; message: string };
export const SESSION_EVENTS = ["reconnected", "credential_state_changed"];
/** What the Vault page and the credential pickers reload on. */
export const VAULT_EVENTS = ["reconnected", "credential_saved", "credential_deleted", "credential_connected", "credential_state_changed", "grant_changed"];
/** What changes an ability's readiness: its grants, its credentials, its settings, installs. */
export const ACCESS_EVENTS = ["reconnected", "grant_changed", "credential_saved", "credential_deleted", "credential_connected", "credential_state_changed", "ability_settings_saved", "secret_set", "extension_installed", "extension_removed", "extension_paused"];
/** Everything that changes a channel or who publishes there: pages listing channels reload on these. */
export const CHANNEL_EVENTS = ["reconnected", "channel_saved", "channel_deleted", "grant_changed", "credential_saved", "credential_deleted", "credential_state_changed", "extension_installed", "extension_removed", "extension_paused"];
export const HANDS_EVENTS = ["hands_frame", "hands_started", "hands_ended"];
export const PROGRAM_EVENTS = ["program_installing", "program_installed", "program_failed", "program_log"];

export type Health = {
  ok: boolean;
  version: string;
  pid?: number;
  /** New on every start, also a restart in place. */
  boot?: string;
  home: string;
  providers: { task: string; model: string; available: boolean }[];
  unblockingapi: boolean;
  /** Which secrets are set in `.env`. Names only; values never leave the daemon. */
  secrets?: { name: string; set: boolean }[];
  /** Model command-line tools found on this machine. */
  tools?: { name: string; installed: boolean }[];
  env_file?: string;
  address?: string;
  /** `KIQR_HOST` / `KIQR_PORT` when set: they override config.yaml. */
  address_env?: string[];
  /** The machine the daemon runs on (`linux`, `macos`, `windows`): what an ability can run here depends on it, not on the page's device. */
  os?: string;
  /** Its package manager (`apt`, `dnf`, `pacman`…), when known, and whether KIQR can run it through the desktop's password dialog. */
  package_manager?: string | null;
  pkexec?: boolean;
  /** A newer KIQR, when the daemon's last check found one. */
  update?: { latest: string | null; available: boolean; can_self_update: boolean };
};
/** `GET /api/update`: what the daemon knows about KIQR releases. */
export type UpdateStatus = {
  current: string;
  latest: string | null;
  available: boolean;
  notes_url: string | null;
  checked_at: string | null;
  can_self_update: boolean;
  reason: string | null;
  error: string | null;
  updating: boolean;
};
/** `GET /api/telemetry`: the anonymous usage ping, and exactly what it sends. */
export type Telemetry = {
  enabled: boolean;
  /** What switched it off: `config`, `KIQR_TELEMETRY`, `DO_NOT_TRACK` or `no-cloud`. */
  off_by: string | null;
  endpoint: string;
  every_minutes: number;
  payload: Record<string, unknown>;
};
/** `config.yaml`. */
export type Config = {
  daemon: { host: string; port: number };
  crawl: { max_pages: number; concurrency: number; ai_concurrency: number };
  models: { extract: string; synthesize: string; chat: string };
  /** Seemo's sessions: spend ceiling per turn, how many run at once, the timezone for schedules. */
  agent?: { max_budget_usd: number; max_concurrent: number; timezone: string };
  /** Which connection serves an ability when several are ready: `{ image: "fal-image" }`. */
  abilities?: Record<string, string>;
  /** Whether KIQR installs new versions on its own. */
  updates?: { auto: boolean };
};

// ---- Seemo: chats, approvals, tasks

/** A tool Seemo used while writing a message. `ok` is null while it runs. */
export type Step = { tool: string; label: string; ok: boolean | null; note?: string; image?: string; asset?: { type: string; id: string } };
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
/** The marketing plan Seemo owns: a period, a few objectives with a KPI and a target each, and the campaigns under them. */
export type Objective = { id: string; title: string; why: string; kpi: string; target?: number | null; unit: string; by?: string | null; status: "active" | "done" | "dropped" };
export type Plan = { version: number; period: string; summary: string; objectives: Objective[]; channels: string[]; updated_by: string; updated_at?: string | null };
export type Campaign = {
  id: string;
  title: string;
  goal: string;
  objective?: string | null;
  objective_title?: string | null;
  channels: string[];
  start?: string | null;
  end?: string | null;
  status: "active" | "done" | "dropped";
  created_by: string;
  created_at?: string | null;
  updated_at?: string | null;
  assets: { type: string; id: string; title: string; status: AssetStatus; updated_at: string }[];
  tasks: { id: string; title: string; status: string; next_run: string | null; runs: number }[];
};
export type PlanView = { plan: Plan | null; kpis: { id: string; label: string }[]; campaigns: Campaign[] };
/** The numbers against the plan, from daily snapshots. */
export type PerfSource = { id: string; label: string; provider: string | null; candidates: { id: string; name: string; ready: boolean }[]; taken_at: string | null; from: string | null; to: string | null; rows: number };
export type PerfMetric = { source: string; metric: string; label: string; lower_is_better: boolean; last7: number | null; prev7: number | null; last30: number | null; series: { date: string; value: number }[] };
export type PerfKpi = { objective: string; title: string; kpi: string; label: string | null; target: number | null; unit: string; by: string | null; source: string | null; measured: boolean; value: number | null; prev: number | null; window_days: number };
export type Performance = { sources: PerfSource[]; metrics: PerfMetric[]; tops: Record<string, { key: string; value: number }[]>; kpis: PerfKpi[]; plan_period: string | null; today: string; refreshed?: { note: string; ok: boolean }[] };
/** A task run coming up, or a publish the daemon will do (`kind: "publish"`, with the asset, the channel and who publishes there). */
export type CalendarItem = { kind?: "task" | "publish"; task?: string; asset?: { type: string; id: string; title: string }; target?: string | null; target_label?: string; via?: string | null; title: string; at: string; schedule_text: string };
/** What waits for the user: open approvals from every chat, drafts to approve, what is approved but not sent, publishes coming up, failed publishes. */
/** A kind of place content goes (a network, a blog, an inbox). Installed ones carry what their ability declares; listed ones only a name. */
export type Network = { kind: string; name: string; icon: string; network: boolean; installed: boolean; text_limit?: number | null; image?: { width: number; height: number } | null; preview?: string; ability?: string };
/** A channel that cannot be published to yet, with its one fix. */
export type ChannelWaiting = { id: string; label: string; domain: string; kind: string; readiness: ChannelReadiness };
export type Todo = {
  channels_waiting?: ChannelWaiting[];
  approvals: { id: string; kind: string; chat: string | null; chat_title?: string; message?: string | null; created: string; payload: Record<string, unknown> }[];
  drafts: Asset[];
  ready: Asset[];
  scheduled: Asset[];
  attention: Asset[];
  /** Logins used here whose sign-in needs a person. */
  credentials_needing_hand?: NeedsHand[];
  count: number;
};
/** An account whose sign-in waits for a person in the live view. */
export type NeedsHand = { credential: string; label: string; message: string; domain?: string | null; since?: string | null };
export type ToolGroup = { id: string; label: string; tools: string[] };

// ---- Abilities
export type Scope = { scope: "global" } | { scope: "project"; project: string };
/** A key an ability needs. `set` says where, for a core ability's own key (this project or the machine); an installed ability's keys live on its accounts. */
export type MarketAuth = { env: string; label: string; help: string; optional: boolean; options?: string[]; placeholder?: string; set: "project" | "global" | boolean | null };
/** A credential as an ability's Access shows it: who it is and whether it is complete; never a value. */
export type AccessCred = { credential: string; label: string; identity?: string | null; domain?: string | null; template: string; complete: boolean };
/** What an ability asks for, and in this project what was allowed and what else in the Vault would fit. */
export type AccessNeed = { id: string; describe: string; /** The kiqr.cloud provider a sign-in goes through, by name ("Google"), when known. */ provider_name?: string | null; template: string[]; hosts: string[]; provider: string; scopes: string[]; for: string; optional: boolean; granted: AccessCred[]; fitting: AccessCred[]; /** A one-click sign-in of the ability's own ("Connect with WordPress"); `needs` are settings asked first. */ connect?: { label: string; needs: string[]; /** It connects a website: started from the website's channel, which gives the address. */ from_channel?: boolean } | null };
/** A kind an ability publishes as, and the target fields that name a spot of it (a subreddit). */
export type KindInfo = { kind: string; label: string; name: string; section: string; target: Record<string, SpecField>; publishes: boolean };
/** One field a manifest declares: a kind's target field (a subreddit) or an ability's setting (a GA4 property). */
export type SpecField = { type?: string; label?: string; help?: string; optional?: boolean; enum?: string[]; placeholder?: string; pattern?: string; /** Picked from a list read through the account (`GET …/accounts/{id}/choices/{field}`), not typed. */ choices?: Record<string, unknown> | boolean | null };
/** What Seemo may do in a channel: never, after a yes, or on its own (the card is the record). */
export type ChannelMode = "off" | "ask" | "auto";
/** The business's own (a profile, a page, a list), or somewhere it takes part (a subreddit, a forum). */
export type Ownership = "owned" | "third_party";
/** Whether a channel can be published to now, in a line, and the one fix. */
export type ChannelReadiness = {
  state: "ready" | "suggested" | "no_ability" | "pick_ability" | "needs_fields" | "publish_off" | "no_publisher" | "publisher_not_ready" | "publisher_gone";
  ready: boolean;
  text: string;
  fix: { action: "add" | "abilities" | "install" | "publisher" | "credential"; label: string; credential?: string; ability?: string } | null;
};
/** Who a channel publishes as (an account, for now), and whether that works. */
export type ChannelPublisher = { credential: string; label: string; domain?: string | null; template?: string; state: CredStateWord; ready: boolean };
/** Everywhere the business's people are: what Seemo may do there and who it posts as. */
export type Channel = {
  id: string;
  label: string;
  ownership: Ownership;
  domain: string;
  kind: string;
  fields?: Record<string, unknown>;
  url?: string;
  description?: string;
  publisher: string;
  ability?: string;
  read: boolean;
  publish: ChannelMode;
  reply: ChannelMode;
  learn: boolean;
  added_by: string;
  suggested: boolean;
  reason?: string;
  created_at: string;
  publications?: number;
  last_published?: string | null;
  last_learned?: string | null;
  learned?: number;
  publisher_view: ChannelPublisher | null;
  /** Who could publish here. */
  publishers: ChannelPublisher[];
  /** What a publisher must be: the publishing ability's need, to prefill "Add a credential". */
  publisher_need?: { ability: string; ability_name: string; describe: string; provider_name?: string | null; /** The ability's own one-click sign-in for this website ("Connect with WordPress"). */ connect?: { need: string; label: string } | null; template: string[]; hosts: string[]; provider: string; scopes: string[] } | null;
  readiness: ChannelReadiness;
  kind_name?: string;
  kind_label?: string;
  icon?: string;
  publishes?: boolean;
  network?: boolean;
  ability_name?: string;
  target?: Record<string, SpecField>;
  can_learn?: boolean;
  /** A website's ways of being published (`kind: website`): abilities here and in the marketplace whose kind publishes a site. */
  site_ways?: SiteWay[];
  /** A list's people, counted. */
  contacts?: number;
  unsubscribed?: number;
  /** Choices a publish there offers (a blog's status), and what Seemo prepares for each publish. */
  options?: Record<string, { type?: string; enum?: string[]; description?: string }>;
  prepared?: string[];
};
/** What can be added: a kind on its site, with its target fields; or a list. */
export type ChannelKind = { kind: string; label: string; name: string; icon: string; domain: string; ownership: Ownership; target: Record<string, SpecField>; account_is_target: boolean; publishes?: boolean; ability?: string; ability_name?: string };
export type ChannelsView = { channels: Channel[]; kinds: ChannelKind[] };
/** How a website can be published: an ability whose kind publishes a site (a repository that builds it, a blog engine). */
export type SiteWay = { ability: string; ability_name: string; kind: string; name: string; label: string; icon: string; installed: boolean };
export type NewChannel = { kind?: string; url?: string; domain?: string; fields?: Record<string, unknown>; label?: string; description?: string; ownership?: Ownership | ""; publisher?: string; publish?: ChannelMode; reply?: ChannelMode; read?: boolean };
export type ChannelPatch = { label?: string; description?: string; url?: string; ownership?: Ownership; publisher?: string; ability?: string; kind?: "website"; fields?: Record<string, string>; read?: boolean; publish?: ChannelMode; reply?: ChannelMode; learn?: boolean; add?: boolean };
/** Somewhere to publish that is no channel yet: it becomes one with the yes. */
export type Spot = { kind: string; fields: Record<string, unknown>; account?: string };
/** A service as the grid of things to connect shows it. `group` is one of SERVICE_GROUPS. */
export type ServiceCard = { id: string; name?: string; icon?: string; group?: string; profile_hosts?: string[] };
/** Where a service sits in the grid of things to connect, in the order shown. */
export const SERVICE_GROUPS: { id: string; label: string }[] = [
  { id: "social", label: "Social" },
  { id: "website", label: "Website and blog" },
  { id: "email", label: "Email" },
  { id: "numbers", label: "Numbers" },
  { id: "passwords", label: "Passwords" },
  { id: "sites", label: "Sites behind a login" },
  { id: "tools", label: "Tools" },
];
/** How an ability signs in with one button (its manifest's `connect`): what to ask first, what comes back. */
export type ConnectInfo = { label: string; flow: "redirect" | "oauth2" | "cloud"; needs: string[]; returns: string[]; https_only: boolean; /** kiqr.cloud finishes this sign-in (it holds the provider's app). */ via_cloud?: boolean; provider?: string | null; /** A page at the service where access to more of it is granted (an app installed on a repository), shown when a picked list lacks what the person looks for. */ grant?: ConnectGrant | null };
export type ConnectGrant = { label: string; url: string; help?: string };
/** One instructions file of an ability, and where its text is overridden. */
export type InstructionFile = { file: string; label: string; project_copy: boolean; machine_copy: boolean };
/** One ability: what Seemo can do. Core ones are code; the rest are folders with instructions (SKILL.md), a connection (extension.yaml), or both. */
export type Ability = {
  id: string;
  name: string;
  /** The vendor: the GitHub user or organisation behind it (`kiqr` for KIQR's own, `you` for yours). */
  author: string;
  author_url: string | null;
  summary: string;
  /** When Seemo uses it (from its instructions). */
  description: string;
  version: string;
  latest: string;
  homepage: string;
  categories: string[];
  icon: string;
  cost: string | null;
  provides: { ability: string; label: string }[];
  /** The networks it publishes to, by name. */
  publishes_to: string[];
  sources: string[];
  avatar?: string | null;
  tools: number;
  trust: "official" | "verified" | "community" | "local" | string;
  transport: "http" | "command" | "mcp" | "builtin" | "instructions" | string;
  runs_program: boolean;
  command: string | null;
  program_found: boolean | null;
  /** The programs KIQR installs for it (a browser), and whether each is on this machine. Null in the marketplace. */
  programs?: ProgramStatus[] | null;
  /** Where it runs (`linux`, `macos`, `windows`); missing means everywhere. */
  platforms?: string[] | null;
  hosts: string[];
  source: "core" | "shipped" | "marketplace" | "local" | "yours";
  repo: string | null;
  core: boolean;
  builtin: boolean;
  installed: boolean;
  scope: Scope | null;
  folder: string | null;
  git: boolean;
  /** The tools Seemo gets from it (its own, and an MCP server's exposed ones). */
  tool_list?: { name: string; label: string; description: string; role: string }[];
  /** Installed from a repository or shipped with KIQR, so Update can refresh it. */
  updatable?: boolean;
  /** A core ability switched off in config.yaml for every project. Installed abilities are switched per project only. */
  machine_off: boolean;
  /** Switched off for this project. */
  paused: boolean;
  ready: boolean;
  missing: string[];
  auth: MarketAuth[];
  /** A core ability KIQR signs in to: this project's own sign-in, the machine's, or none. */
  signed_in?: "project" | "machine" | null;
  /** What it asks for from the Vault, and what this project allowed it. */
  access?: AccessNeed[];
  /** The kinds it publishes as. */
  kinds?: KindInfo[];
  /** What the project picks once for it (a property, a site), field by field, and what it picked. */
  settings?: Record<string, SpecField> | null;
  setting?: AbilitySetting | null;
  /** How a card shows it. */
  service_card?: ServiceCard | null;
  /** It can teach the project from an owned channel: what the switch says. */
  learn?: LearnInfo | null;
  active_for: string[];
  instructions: InstructionFile[];
  in_prompt: boolean;
  error: string | null;
};
export type Contract = { id: string; label: string; tool: string; configured: string | null; active: { id: string; name: string } | null; candidates: { id: string; name: string; ready: boolean }[] };
export type AbilitiesView = { abilities: Ability[]; market: Ability[]; contracts: Contract[]; broken: { folder: string; scope: Scope; error: string }[]; guide: Ability | null; index: string; index_fetched?: boolean; env_file: string; project_env_file: string | null };
/** One of KIQR's own sign-ins: kiqr.cloud, UnblockingAPI. `waiting` while a device code is shown. */
export type SignIn = { id: string; name: string; signed_in: boolean; account: string | null; per_project?: boolean; scope?: "project" | "machine" | null; waiting: { user_code: string; verification_uri: string; verification_uri_complete?: string | null } | null };
/** The one readiness rule's word for a credential. */
export type CredStateWord = "ready" | "missing_fields" | "needs_program" | "signed_out" | "needs_hand" | "checking" | "expired";
/** What a project picked once for an ability (a GA4 property), and the credential its calls go through. */
export type AbilitySetting = { credential?: string } & Record<string, unknown>;
/** How a credential's sign-in on a browser stands, for one project's abilities. */
export type CredentialSession = { has_session: boolean; session: SessionStatus; has_login: boolean; live_view: boolean; by_hand?: boolean; start_url?: string | null };
/** What abilities did as a credential, in every project. */
export type CredentialActivity = { credential: string; label: string; today: number; activity: { at: string; project?: string; ability: string; role: string; tool: string; target?: string; channel?: string; url?: string | null }[] };
/** Abilities that could use a credential, by host: allowed here, installed, or in the marketplace. */
export type Recommendation = { credential: string; label: string; domain?: string | null; provider?: string | null; abilities: { id: string; name: string; icon: string; summary: string; status: "allowed" | "installed" | "available"; need: string; for: string; more_access: boolean }[] };
export type LearnInfo = { label: string };
/** `unknown`, `checking`, `signed_in`, `signed_out`, `needs_hand`. */
export type SessionState = "unknown" | "checking" | "signed_in" | "signed_out" | "needs_hand";
/** How an account's sign-in on a hosted program stands. */
export type SessionStatus = { state: SessionState | ""; message: string; checked_at: string | null; signed_in_at: string | null; by_user?: boolean };
/** The values a login saved in KIQR has, by key name. */
export const LOGIN_KEYS = ["LOGIN_USERNAME", "LOGIN_PASSWORD", "LOGIN_TOTP_SECRET"] as const;
/** A program an ability needs: found here or not, and how KIQR gets it (`system`: a package the user installs; by package manager). */
export type ProgramStatus = {
  program: string;
  version: string;
  found: boolean;
  /** How KIQR gets it here: a download it makes itself, a `uv` tool, a system package, or no way at all. */
  how: "download" | "uv" | "system" | "none" | string;
  /** The package per package manager (`apt: chromium`). */
  system?: Record<string, string>;
  /** The one install command for this machine's package manager, when KIQR knows it. */
  command?: string | null;
  /** KIQR can run that command for the user through the desktop's password dialog. */
  pkexec?: boolean;
};
/** What the live view can do on this account. */
export type HandsCan = { click: boolean; key: boolean; type: boolean; navigate: boolean };
/** One input from the person at the controls: x, y are fractions of the picture, width and height its pixel size. */
export type HandsInput = { kind: "click"; x: number; y: number; width: number; height: number } | { kind: "key"; key: string } | { kind: "type"; text: string } | { kind: "navigate"; url: string };
/** Where KIQR listens, when a factory reset could keep it (it isn't the default). `remote`: reachable from other machines. */
export type ResetNetwork = { host: string; port: number; remote: boolean };
/** A list of people the project emails: a channel of kind `list`, sent by its publisher. The addresses live outside git; this is the record and the counts. */
export type MailList = { id: string; label: string; description?: string; publisher: string; contacts: number; unsubscribed: number; created_at: string; added_by?: string };
export type Contact = { email: string; name?: string; consent?: string; added?: string | null; unsubscribed?: string | null };
/** One send to a list: so many sent, so many failed, still running or not. */
export type ListSend = { key: string; sent: number; failed: number; at: string | null; running: boolean };

/** One field of a credential as the UI sees it: whether it is set, and the value only for what may be shown (a username, a host). */
export type CredentialField = { name: string; label: string; kind: "text" | "secret" | "totp" | "url" | "number"; set: boolean; value?: string; optional: boolean };
/** A credential in the Vault. Secrets never come back from the daemon. */
export type Credential = {
  id: string;
  label: string;
  template: string;
  domains: string[];
  identity: string | null;
  provider: string | null;
  scopes: string[];
  login_url: string | null;
  notes: string;
  missing: string[];
  fields: CredentialField[];
  /** Whether it works in the project the list was read for (`?project=`): signed out, needs a hand, … */
  state?: CredStateWord;
  state_text?: string;
  created_at: string;
  updated_at: string;
};
export type CredentialTemplate = { id: string; label: string; fields: { name: string; label: string; kind: CredentialField["kind"]; optional: boolean }[] };
export type VaultStatus = { unlocked: boolean; key: string | null; key_text: string; problem: string | null; count: number };
export type VaultView = { status: VaultStatus; credentials: Credential[]; templates: CredentialTemplate[] };
/** A provider kiqr.cloud signs in to, with where its tokens may go and the access it can ask for. */
export type SignInProvider = { id: string; name: string; domains: string[]; scopes: { scope: string; label: string }[] };
/** A new credential, or changes to one: values by field name (an empty value clears it). */
export type CredentialDraft = { label?: string; template?: string; domains?: string[]; values?: Record<string, string>; fields?: { name: string; label: string; kind: CredentialField["kind"]; optional?: boolean }[]; login_url?: string; notes?: string };
export type Access = { grants: Record<string, string[]> };
/** Seemo → Memory: the owner's rules (`by: user`) and what Seemo learned; `pending` lessons wait for a Keep. */
export type MemoryKind = "rule" | "preference" | "fact" | "lesson";
export type MemoryScope = "everywhere" | "own" | "others";
export type Memory = { id: string; text: string; kind: MemoryKind; scope?: MemoryScope; by: "user" | "seemo"; from?: string | null; pending?: boolean; created_at: string; updated_at: string };
export type Memories = { items: Memory[]; learning: boolean; pending: number };

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
  updateStatus: (refresh = false) => req<UpdateStatus>(`/api/update${refresh ? "?refresh=1" : ""}`),
  applyUpdate: () => req<UpdateStatus>("/api/update", { method: "POST", body: "{}" }),
  telemetry: () => req<Telemetry>("/api/telemetry"),
  setTelemetry: (enabled: boolean) => req<Telemetry>("/api/telemetry", { method: "PUT", body: JSON.stringify({ enabled }) }),
  config: () => req<Config>("/api/config"),
  /** Factory reset, step one: a one-time token and the phrase to type. Refused to anything but the web UI. */
  resetStart: () => req<{ token: string; phrase: string; captcha: string; home: string; expires_in: number; network: ResetNetwork | null }>("/api/reset/start", { method: "POST", body: "{}" }),
  /** Step two: erase the home and restart as a new installation, on `port`. */
  reset: (token: string, phrase: string, captcha: string, keepNetwork = false) => req<{ resetting: boolean; port: number }>("/api/reset", { method: "POST", body: JSON.stringify({ token, phrase, captcha, keep_network: keepNetwork }) }),
  saveConfig: (c: Config) => req<Config>("/api/config", { method: "PUT", body: JSON.stringify(c) }),
  projects: () => req<Project[]>("/api/projects"),
  project: (slug: string) => req<Project>(`/api/projects/${slug}`),
  createProject: (url: string) => req<Project>("/api/projects", { method: "POST", body: JSON.stringify({ url }) }),
  deleteProject: (slug: string) => req<void>(`/api/projects/${slug}`, { method: "DELETE" }),
  crawl: (slug: string) => req<{ started: boolean }>(`/api/projects/${slug}/crawl`, { method: "POST", body: "{}" }),
  build: (slug: string) => req<Build>(`/api/projects/${slug}/build`),
  intake: (slug: string) => req<Intake>(`/api/projects/${slug}/intake`),
  memory: (slug: string) => req<Memories>(`/api/projects/${slug}/memory`),
  addMemory: (slug: string, text: string, kind: MemoryKind, scope: MemoryScope) => req<Memories>(`/api/projects/${slug}/memory`, { method: "POST", body: JSON.stringify({ text, kind, scope }) }),
  updateMemory: (slug: string, id: string, b: { text?: string; kind?: MemoryKind; scope?: MemoryScope }) => req<Memories>(`/api/projects/${slug}/memory/${id}`, { method: "PUT", body: JSON.stringify(b) }),
  keepMemory: (slug: string, id: string) => req<Memories>(`/api/projects/${slug}/memory/${id}/keep`, { method: "POST", body: "{}" }),
  deleteMemory: (slug: string, id: string) => req<Memories>(`/api/projects/${slug}/memory/${id}`, { method: "DELETE" }),
  setLearning: (slug: string, on: boolean) => req<Memories>(`/api/projects/${slug}/memory/learning`, { method: "PUT", body: JSON.stringify({ on }) }),
  setup: (slug: string) => req<Setup>(`/api/projects/${slug}/setup`),
  saveIntake: (slug: string, i: Intake) => req<Intake>(`/api/projects/${slug}/intake`, { method: "PUT", body: JSON.stringify(i) }),
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
  brandKit: (slug: string) => req<BrandKit>(`/api/projects/${slug}/brand/kit`),
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

  // Media: the business's own pictures, from the website and from abilities
  mediaSources: (slug: string) => req<{ sources: MediaSource[] }>(`/api/projects/${slug}/media/sources`),
  /** 404 for an unknown source; 412 (with the reason as the error) for one not set up yet. */
  mediaItems: (slug: string, source: string) => req<{ source: string; items: MediaItem[] }>(`/api/projects/${slug}/media/sources/${encodeURIComponent(source)}/items`),
  /** Bring one picture into the Library: a source's item, or any URL. Returns the asset, new or already there. */
  mediaImport: (slug: string, b: { source?: string; id?: string; url?: string; alt?: string }) => req<MediaImport>(`/api/projects/${slug}/media/import`, { method: "POST", body: JSON.stringify(b) }),
  /** The website's pictures, up to 12, best effort. */
  mediaImportSite: (slug: string) => req<{ imported: number; assets: { id: string; title: string }[] }>(`/api/projects/${slug}/media/import-site`, { method: "POST", body: "{}" }),

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
  todo: (slug: string) => req<Todo>(`/api/projects/${slug}/todo`),
  /** Every kind of place the project knows: its abilities' in full, the marketplace's by name. */
  networks: (slug: string) => req<{ networks: Network[] }>(`/api/projects/${slug}/networks`),
  /** Drop the cached marketplace index and read it again in the background; `marketplace_refreshed` follows. */
  refreshMarketplace: () => req<void>("/api/abilities/refresh-index", { method: "POST", body: "{}" }),

  // The marketing plan and campaigns
  plan: (slug: string) => req<PlanView>(`/api/projects/${slug}/plan`),
  savePlan: (slug: string, plan: Plan) => req<PlanView>(`/api/projects/${slug}/plan`, { method: "PUT", body: JSON.stringify(plan) }),
  createCampaign: (slug: string, c: { title: string; goal: string; objective?: string; channels?: string[]; start?: string; end?: string }) => req<Campaign>(`/api/projects/${slug}/campaigns`, { method: "POST", body: JSON.stringify(c) }),
  updateCampaign: (slug: string, id: string, c: Partial<{ title: string; goal: string; objective: string; channels: string[]; start: string; end: string; status: "active" | "done" | "dropped" }>) => req<Campaign>(`/api/projects/${slug}/campaigns/${id}`, { method: "PUT", body: JSON.stringify(c) }),
  deleteCampaign: (slug: string, id: string) => req<void>(`/api/projects/${slug}/campaigns/${id}`, { method: "DELETE" }),
  performance: (slug: string) => req<Performance>(`/api/projects/${slug}/performance`),
  refreshPerformance: (slug: string, source?: string) => req<Performance>(`/api/projects/${slug}/performance/refresh`, { method: "POST", body: JSON.stringify({ source }) }),
  tools: () => req<{ groups: ToolGroup[] }>("/api/tools"),

  // Abilities
  /** Everything Seemo can do in a project (or on the machine when `slug` is empty), and the marketplace. */
  abilities: (slug?: string) => req<AbilitiesView>(slug ? `/api/projects/${slug}/abilities` : "/api/abilities"),
  /** An ability's instructions as they apply (`file` picks one when it has several). Reads switched-off ones too. */
  instructions: (slug: string | null, id: string, file?: string) => {
    const path = `${slug ? `/api/projects/${slug}` : "/api"}/abilities/${encodeURIComponent(id)}/instructions?all=1${file ? `&file=${encodeURIComponent(file)}` : ""}`;
    return DEMO ? demoSeemoRequest<string>(path) : fetch(path).then((r) => (r.ok ? r.text() : r.json().then((e: { error?: string }) => Promise.reject(new Error(e.error ?? `${r.status}`)))));
  },
  /** Write instructions: a new ability of yours, an edit, or a copy of shipped text; for one project (`slug`) or every project (`null`). */
  saveInstructions: (slug: string | null, id: string, text: string, file?: string) => req<Ability>(`${slug ? `/api/projects/${slug}` : "/api"}/abilities/${encodeURIComponent(id)}/instructions`, { method: "PUT", body: JSON.stringify({ text, file }) }),
  /** Drop the copy at that scope: a project's copy, or the machine's edit of shipped text (back to the shipped one). */
  deleteInstructions: (slug: string | null, id: string, file?: string) => req<void>(`${slug ? `/api/projects/${slug}` : "/api"}/abilities/${encodeURIComponent(id)}/instructions${file ? `?file=${encodeURIComponent(file)}` : ""}`, { method: "DELETE" }),
  /** Install for the machine: the code only. A project adds its accounts afterwards. */
  installAbility: (id: string) => req<{ id: string; folder: string }>(`/api/abilities/${id}/install`, { method: "POST", body: "{}" }),
  /** Install the programs an installed ability needs (again); `program_*` events show the progress. Answers when done. */
  installPrograms: (id: string) => req<{ id: string; programs: ProgramStatus[] }>(`/api/abilities/${id}/programs`, { method: "POST", body: "{}" }),
  /** The install log so far (program, line) and the abilities whose programs are installing right now. */
  programLog: () => req<{ lines: { program: string; line: string }[]; installing: string[]; programs: string[] }>("/api/programs/log"),
  /** Install the system packages an ability still needs, through the desktop's password dialog (a person in the web UI only). */
  installSystemPrograms: (id: string) => req<{ installed: string[]; programs: ProgramStatus[] }>(`/api/abilities/${id}/programs/system`, { method: "POST", body: "{}" }),
  installAbilitySource: (b: { source: string; scope: "global" | "project"; project?: string; name?: string }) => req<Ability>("/api/abilities/install", { method: "POST", body: JSON.stringify(b) }),
  updateAbility: (id: string, slug?: string) => req<{ detail: string }>(`/api/abilities/${id}/update${slug ? `?project=${encodeURIComponent(slug)}` : ""}`, { method: "POST", body: "{}" }),
  /** Off or on for every project: core abilities only (config.yaml). */
  pauseAbility: (id: string, slug?: string) => req<{ paused: boolean }>(`/api/abilities/${id}/pause${slug ? `?project=${encodeURIComponent(slug)}` : ""}`, { method: "POST", body: "{}" }),
  resumeAbility: (id: string, slug?: string) => req<{ paused: boolean }>(`/api/abilities/${id}/resume${slug ? `?project=${encodeURIComponent(slug)}` : ""}`, { method: "POST", body: "{}" }),
  removeAbility: (id: string, slug?: string) => req<void>(`/api/abilities/${id}${slug ? `?project=${encodeURIComponent(slug)}` : ""}`, { method: "DELETE" }),
  /** Off or on for one project. */
  pauseProjectAbility: (slug: string, id: string) => req<{ paused: boolean }>(`/api/projects/${slug}/abilities/${id}/pause`, { method: "POST", body: "{}" }),
  resumeProjectAbility: (slug: string, id: string) => req<{ paused: boolean }>(`/api/projects/${slug}/abilities/${id}/resume`, { method: "POST", body: "{}" }),
  /** Set one of KIQR's own keys in .env by name; an empty value clears it. The value is never read back. An ability's keys go on an account. */
  setSecret: (name: string, value: string) => req<{ name: string; set: boolean }>(`/api/secrets/${name}`, { method: "PUT", body: JSON.stringify({ value }) }),
  /** The same for one project: its own .env wins over the machine's. */
  setProjectSecret: (slug: string, name: string, value: string) => req<{ name: string; set: boolean }>(`/api/projects/${slug}/secrets/${name}`, { method: "PUT", body: JSON.stringify({ value }) }),

  /** Start signing in: `account` to sign one in again, else a new one is made when the sign-in comes back (with the name and permissions chosen). Saves what was asked first and returns the page to open; `callback: false` means the user pastes what it shows into the account. */
  vault: (project?: string) => req<VaultView>(`/api/vault${project ? `?project=${encodeURIComponent(project)}` : ""}`),
  credential: (id: string) => req<Credential>(`/api/vault/${encodeURIComponent(id)}`),
  createCredential: (d: CredentialDraft) => req<Credential>("/api/vault", { method: "POST", body: JSON.stringify(d) }),
  updateCredential: (id: string, d: CredentialDraft) => req<Credential>(`/api/vault/${encodeURIComponent(id)}`, { method: "PUT", body: JSON.stringify(d) }),
  deleteCredential: (id: string) => req<{ ok: boolean }>(`/api/vault/${encodeURIComponent(id)}`, { method: "DELETE" }),
  /** The providers kiqr.cloud offers for "Sign in with …", and whether this KIQR is signed in to kiqr.cloud. */
  signInProviders: () => req<{ providers: SignInProvider[]; cloud_signed_in: boolean }>("/api/connect/providers"),
  /** Start a "Sign in with …": opens the provider; `credential_connected` says how it ended. */
  vaultConnect: (provider: string, b: { origin: string; scopes?: string[]; credential?: string; label?: string; grant?: { project: string; ability: string }[] }) =>
    req<{ url: string; callback: boolean }>(`/api/vault/connect/${encodeURIComponent(provider)}`, { method: "POST", body: JSON.stringify(b) }),
  access: (slug: string) => req<Access>(`/api/projects/${slug}/access`),
  grant: (slug: string, ability: string, credential: string) => req<Access>(`/api/projects/${slug}/access/grant`, { method: "POST", body: JSON.stringify({ ability, credential }) }),
  revoke: (slug: string, ability: string, credential: string) => req<Access>(`/api/projects/${slug}/access/grant`, { method: "DELETE", body: JSON.stringify({ ability, credential }) }),
  signIns: (slug?: string) => req<{ signins: SignIn[]; cloud_url: string }>(`/api/signins${slug ? `?project=${encodeURIComponent(slug)}` : ""}`),
  /** `redirect`: open `url`. `device`: show the code and where to enter it; the daemon waits for the approval. */
  startSignIn: (id: string, origin: string, project?: string) => req<{ mode: "redirect"; url: string } | { mode: "device"; user_code: string; verification_uri: string; verification_uri_complete?: string | null }>(`/api/signins/${id}/start`, { method: "POST", body: JSON.stringify({ origin, project }) }),
  signOut: (id: string, project?: string) => req<{ signed_in: boolean }>(`/api/signins/${id}${project ? `?project=${encodeURIComponent(project)}` : ""}`, { method: "DELETE" }),

  // Credentials at work: sign-ins on a browser (for a project's abilities), the live view, what was done, recommendations
  credentialSession: (slug: string, id: string) => req<CredentialSession>(`/api/projects/${slug}/credentials/${encodeURIComponent(id)}/session`),
  /** Check and sign in now, in the background; `credential_state_changed` says how it went. */
  credentialSignIn: (slug: string, id: string) => req<{ started: boolean; note?: string; session?: SessionStatus }>(`/api/projects/${slug}/credentials/${encodeURIComponent(id)}/signin`, { method: "POST", body: "{}" }),
  credentialSignOut: (slug: string, id: string) => req<{ session: SessionStatus }>(`/api/projects/${slug}/credentials/${encodeURIComponent(id)}/signout`, { method: "POST", body: "{}" }),
  /** Take the controls of a login's browser: the live view. */
  startHands: (slug: string, id: string, url?: string) => req<{ hands: string; token: string; can: HandsCan }>(`/api/projects/${slug}/credentials/${encodeURIComponent(id)}/hands`, { method: "POST", body: JSON.stringify(url ? { url } : {}) }),
  handsInput: (slug: string, hands: string, token: string, input: HandsInput) => req<{ ok: boolean; message?: string | null }>(`/api/projects/${slug}/hands/${encodeURIComponent(hands)}/input`, { method: "POST", body: JSON.stringify({ token, ...input }) }),
  endHands: (slug: string, hands: string, token: string, signedIn: boolean) => req<{ ended: boolean }>(`/api/projects/${slug}/hands/${encodeURIComponent(hands)}/end`, { method: "POST", body: JSON.stringify({ token, signed_in: signedIn }), keepalive: true }),
  credentialActivity: (id: string, limit = 20) => req<CredentialActivity>(`/api/vault/${encodeURIComponent(id)}/activity?limit=${limit}`),
  /** Abilities that could use the credentials (or one), matched by host. */
  recommendations: (slug: string, credential?: string) => req<{ recommendations: Recommendation[] }>(`/api/projects/${slug}/recommendations${credential ? `?credential=${encodeURIComponent(credential)}` : ""}`),
  /** The values a setting can take, read through the credential the ability's calls go through. */
  abilityChoices: (slug: string, ability: string, name: string, credential?: string) => req<{ name: string; choices: { value: string; label: string }[] }>(`/api/projects/${slug}/abilities/${encodeURIComponent(ability)}/choices/${name}${credential ? `?credential=${encodeURIComponent(credential)}` : ""}`),
  /** Start an ability's own one-click sign-in for a need; `callback: false` means the user types what the page shows. */
  abilityConnect: (slug: string, ability: string, b: { origin: string; need?: string; values?: Record<string, string>; channel?: string }) => req<{ url: string; callback: boolean; relayed: boolean; fields: string[]; template?: string }>(`/api/projects/${slug}/abilities/${encodeURIComponent(ability)}/connect`, { method: "POST", body: JSON.stringify(b) }),
  /** What the project picks once for an ability (a GA4 property, a site), and the credential its calls go through. */
  setAbilitySettings: (slug: string, ability: string, credential: string, values: Record<string, string>) => req<{ ability: string; setting: AbilitySetting }>(`/api/projects/${slug}/abilities/${encodeURIComponent(ability)}/settings`, { method: "PUT", body: JSON.stringify({ credential, values }) }),

  // Channels: everywhere the business's people are, what Seemo may do there, who it posts as
  channels: (slug: string) => req<ChannelsView>(`/api/projects/${slug}/channels`),
  channel: (slug: string, id: string) => req<Channel>(`/api/projects/${slug}/channels/${encodeURIComponent(id)}`),
  /** Add one by link, site or kind; or a list (`kind: "list"`). */
  addChannel: (slug: string, b: NewChannel) => req<Channel>(`/api/projects/${slug}/channels`, { method: "POST", body: JSON.stringify(b) }),
  /** Change what Seemo may do there, who it posts as, its name; or add a suggestion (`add: true`). */
  channelChoices: (slug: string, id: string, name: string) => req<{ name: string; choices: { value: string; label: string }[] }>(`/api/projects/${slug}/channels/${encodeURIComponent(id)}/choices/${name}`),
  updateChannel: (slug: string, id: string, b: ChannelPatch) => req<Channel>(`/api/projects/${slug}/channels/${encodeURIComponent(id)}`, { method: "PUT", body: JSON.stringify(b) }),
  /** Remove it (a list with everyone on it), or dismiss a suggestion. */
  removeChannel: (slug: string, id: string) => req<void>(`/api/projects/${slug}/channels/${encodeURIComponent(id)}`, { method: "DELETE" }),
  learnChannel: (slug: string, id: string) => req<{ started: boolean }>(`/api/projects/${slug}/channels/${encodeURIComponent(id)}/learn`, { method: "POST" }),

  // Publishing: to a channel (`target`), or to a new spot that becomes one with the yes.
  /** Publish an asset now, to the channel it was approved for or the one given. From the UI the click is the yes. */
  publishAsset: (slug: string, a: Pick<Asset, "type" | "id">, where: { target?: string; spot?: Spot } = {}, note = "", options: Record<string, unknown> = {}) =>
    req<{ published: boolean; url: string | null; detail: string; target: string; target_label: string; recipients?: number }>(`/api/projects/${slug}/assets/${a.type}/${a.id}/publish`, { method: "POST", body: JSON.stringify({ ...where, note, options, by: "user" }) }),
  /** The user's yes to an asset, for a channel (or a new spot, which becomes one): it becomes Ready. Status is never set by hand; these change what it follows from. */
  approveAsset: (slug: string, a: Pick<Asset, "type" | "id">, where: { target?: string; spot?: Spot }) => req<Asset>(`/api/projects/${slug}/assets/${a.type}/${a.id}/approve`, { method: "POST", body: JSON.stringify(where) }),
  scheduleAsset: (slug: string, a: Pick<Asset, "type" | "id">, at: string, where: { target?: string; spot?: Spot } = {}, options: Record<string, unknown> = {}) => req<Asset>(`/api/projects/${slug}/assets/${a.type}/${a.id}/schedule`, { method: "POST", body: JSON.stringify({ at, ...where, options }) }),
  unscheduleAsset: (slug: string, a: Pick<Asset, "type" | "id">) => req<Asset>(`/api/projects/${slug}/assets/${a.type}/${a.id}/schedule`, { method: "DELETE" }),
  confirmAttempt: (slug: string, a: Pick<Asset, "type" | "id">, went_out: boolean) => req<{ asset: Asset }>(`/api/projects/${slug}/assets/${a.type}/${a.id}/attempt`, { method: "POST", body: JSON.stringify({ went_out }) }),
  unpublishAsset: (slug: string, a: Pick<Asset, "type" | "id">) => req<Asset>(`/api/projects/${slug}/assets/${a.type}/${a.id}/unpublish`, { method: "POST", body: "{}" }),
  setAssetText: (slug: string, a: Pick<Asset, "type" | "id">, text: string) => req<Asset>(`/api/projects/${slug}/assets/${a.type}/${a.id}`, { method: "PATCH", body: JSON.stringify({ text }) }),
  archiveAsset: (slug: string, a: Pick<Asset, "type" | "id">, archived: boolean) => req<Asset>(`/api/projects/${slug}/assets/${a.type}/${a.id}`, { method: "PATCH", body: JSON.stringify({ archived }) }),
  sendEmail: (slug: string, m: { to: string; subject: string; text: string; asset?: { type: string; id: string } }) => req<{ sent: boolean; to: string; subject: string; connection: string }>(`/api/projects/${slug}/email`, { method: "POST", body: JSON.stringify(m) }),

  // A list channel's people
  contacts: (slug: string, id: string) => req<{ list: string; contacts: Contact[] }>(`/api/projects/${slug}/channels/${encodeURIComponent(id)}/contacts`),
  /** Add people; `consent` is how they agreed to hear from the business, in the user's words. */
  addContacts: (slug: string, id: string, contacts: { email: string; name?: string }[], consent: string) => req<{ added: number; list: MailList }>(`/api/projects/${slug}/channels/${encodeURIComponent(id)}/contacts`, { method: "POST", body: JSON.stringify({ contacts, consent }) }),
  removeContact: (slug: string, id: string, email: string) => req<{ removed: boolean; list: MailList }>(`/api/projects/${slug}/channels/${encodeURIComponent(id)}/contacts`, { method: "DELETE", body: JSON.stringify({ email }) }),
  /** A CSV file's text: an email column (or a column of addresses), a name column when there is one. */
  importContacts: (slug: string, id: string, csv: string, consent: string) => req<{ found: number; added: number; list: MailList }>(`/api/projects/${slug}/channels/${encodeURIComponent(id)}/import`, { method: "POST", body: JSON.stringify({ csv, consent }) }),
  unsubscribe: (slug: string, id: string, email: string) => req<{ unsubscribed: boolean; list: MailList }>(`/api/projects/${slug}/channels/${encodeURIComponent(id)}/unsubscribe`, { method: "POST", body: JSON.stringify({ email }) }),
  listSends: (slug: string, id: string) => req<{ list: string; sends: ListSend[] }>(`/api/projects/${slug}/channels/${encodeURIComponent(id)}/sends`),
};

let demoTelemetry = true;
let demoConfig: Config = {
  daemon: { host: "127.0.0.1", port: 4040 },
  crawl: { max_pages: 200, concurrency: 5, ai_concurrency: 3 },
  models: { extract: "claude-cli/haiku", synthesize: "claude-cli/sonnet", chat: "claude-cli/sonnet" },
  agent: { max_budget_usd: 2, max_concurrent: 3, timezone: "Europe/Stockholm" },
  abilities: {},
};

const demoNow = "2026-11-03T09:00:00Z";
let demoMemory: Memories = {
  learning: true,
  pending: 1,
  items: [
    { id: "m_1", text: "Help people first and never promote; being useful is the promotion.", kind: "rule", scope: "others", by: "user", from: "chat:demo", created_at: demoNow, updated_at: demoNow },
    { id: "m_2", text: "No hashtags on X or LinkedIn.", kind: "preference", by: "user", created_at: demoNow, updated_at: demoNow },
    { id: "m_3", text: "The owner rewrote every X post shorter: aim for 240 characters.", kind: "lesson", by: "seemo", from: "chat:demo", created_at: demoNow, updated_at: demoNow },
    { id: "m_4", text: "r/webscraping removed the post that linked to the pricing page.", kind: "lesson", by: "seemo", from: "run:community/20261102T100000Z", pending: true, created_at: demoNow, updated_at: demoNow },
  ],
};

function demoMemoryRequest<T>(path: string, init?: RequestInit): Promise<T> {
  const m = init?.method ?? "GET";
  const body = init?.body ? JSON.parse(String(init.body)) : {};
  const id = path.match(/\/memory\/(m_[^/]+)/)?.[1];
  let items = demoMemory.items;
  if (path.endsWith("/memory/learning")) demoMemory = { ...demoMemory, learning: body.on };
  else if (path.endsWith("/keep")) items = items.map((x) => (x.id === id ? { ...x, pending: false } : x));
  else if (m === "POST") items = [...items, { id: `m_${Date.now()}`, text: body.text, kind: body.kind, scope: body.scope, by: "user", created_at: new Date().toISOString(), updated_at: new Date().toISOString() }];
  else if (m === "PUT" && id) items = items.map((x) => (x.id === id ? { ...x, ...body, by: "user", pending: false } : x));
  else if (m === "DELETE") items = items.filter((x) => x.id !== id);
  demoMemory = { ...demoMemory, items, pending: items.filter((x) => x.pending).length };
  return Promise.resolve(demoMemory as T);
}

let demoIntake: Intake = { goal: "", goal_note: "", customers: "", time: "", networks: [], skipped: [] };

function demoRequest<T>(path: string, init?: RequestInit): Promise<T> {
  if (path.startsWith("/api/signins")) return Promise.resolve({ signins: [{ id: "kiqr-cloud", name: "kiqr.cloud", signed_in: false, account: null, waiting: null }, { id: "unblockingapi", name: "UnblockingAPI", signed_in: false, account: null, waiting: null }], cloud_url: "https://kiqr.cloud" } as T);
  if (/\/(chats|approvals|tasks|calendar|credentials|recommendations|email|secrets|abilities|connect|networks|suggestions|todo|plan|campaigns|performance|channels|hands|access)(\/|\?|$)/.test(path) || path.startsWith("/api/vault") || path === "/api/connect/providers" || path === "/api/tools" || path.startsWith("/api/abilities") || path.startsWith("/api/secrets")) return demoSeemoRequest<T>(path, init);
  if (path === "/api/projects" && init?.method === "POST") {
    demoState.created = true;
    return Promise.resolve(demoProject as T);
  }
  if (path === "/api/projects") return Promise.resolve((demoState.created ? [demoProject] : []) as T);
  if (path.startsWith("/api/projects/") && path.split("/").length === 4) return Promise.resolve(demoProject as T);
  if (path.endsWith("/brand/kit")) return Promise.resolve({ site_name: "UnblockingAPI", palette: { dark: "#0b1220", light: "#f8fafc", accent: "#e11d74" }, logo: null, html_available: true } as T);
  if (path.includes("/media/")) return demoMediaRequest<T>(path, init);
  if (path === "/api/asset-types" || path.includes("/assets")) return demoAssetRequest<T>(path, init);
  if (path.endsWith("/brief")) return Promise.resolve((init?.method === "PUT" ? JSON.parse(String(init.body)) : demoState.created ? demoBrief : null) as T);
  if (path.endsWith("/build")) return Promise.resolve({ stage: "crawling", pct: 0, eta_secs: null, max_pages: 200, found: 0, fetched: 0, queued: 0, read: 0, facts: 0, sections_done: 0, sections_total: 12, running: true, started_at: null, last_at: null, problem: null } as T);
  if (/\/memory(\/|$)/.test(path)) return demoMemoryRequest<T>(path, init);
  if (path.endsWith("/intake")) {
    if (init?.method === "PUT") demoIntake = JSON.parse(String(init.body));
    return Promise.resolve(demoIntake as T);
  }
  if (path.endsWith("/setup")) {
    const avatar = (id: string) => `https://raw.githubusercontent.com/kiqr/marketplace/main/${id}/avatar.svg`;
    const s: Setup = {
      suggestions: [
        { id: "google-analytics", name: "Read Google Analytics", summary: "Sessions, users, page views and key events, once a day into Performance.", icon: "chart", avatar: avatar("google-analytics"), why: [{ kind: "site" }], installed: true, on: true, ready: true },
        { id: "search-console", name: "Read Google Search Console", summary: "Clicks, impressions and the queries people find you with, once a day into Performance.", icon: "search", avatar: avatar("search-console"), why: [{ kind: "every_site" }, ...(demoIntake.goal === "customers" ? [{ kind: "goal" as const, goal: "customers" }] : [])], installed: false, on: false, ready: false },
        ...(demoIntake.goal === "repeat" || demoIntake.goal === "launch" ? [{ id: "kiqr-smtp", name: "Send email from your mail account", summary: "Email over SMTP from your own address. Every send asks you first.", icon: "mail", avatar: avatar("email-smtp"), why: [{ kind: "goal" as const, goal: demoIntake.goal }], installed: false, on: false, ready: false }] : []),
      ],
      waiting: demoIntake.networks.filter((n) => !["linkedin", "x", "reddit"].includes(n)),
    };
    return Promise.resolve(s as T);
  }
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
      os: "linux",
      package_manager: "apt",
      pkexec: true,
      home: "~/.kiqr",
      env_file: "~/.kiqr/.env",
      address: "127.0.0.1:4040",
      providers: (["extract", "synthesize", "chat"] as const).map((task) => ({ task, model: demoConfig.models[task], available: !demoConfig.models[task].startsWith("openai/") })),
      unblockingapi: true,
      secrets: ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN", "UNBLOCKINGAPI_KEY", "KIQR_CLOUD_TOKEN"].map((name) => ({ name, set: name === "UNBLOCKINGAPI_KEY" || name === "ANTHROPIC_API_KEY" })),
      tools: [
        { name: "claude", installed: true },
        { name: "codex", installed: false },
      ],
      update: { latest: "0.2.0", available: true, can_self_update: true },
    } as T);
  if (path === "/api/telemetry") {
    if (init?.method === "PUT") demoTelemetry = JSON.parse(String(init.body)).enabled;
    return Promise.resolve({ enabled: demoTelemetry, off_by: demoTelemetry ? null : "config", endpoint: "https://kiqr.cloud/api/v1/ping", every_minutes: 15, payload: { id: "(made on the first ping)", version: "demo", os: "linux", arch: "x86_64", docker: false, cloud: false, abilities: ["github"], other_abilities: 0 } } as T);
  }
  if (path.startsWith("/api/update"))
    return init?.method === "POST"
      ? Promise.reject(new Error("The demo cannot update itself."))
      : Promise.resolve({ current: "demo", latest: "0.2.0", available: true, notes_url: "https://github.com/kiqr/kiqr/releases/latest", checked_at: new Date().toISOString(), can_self_update: true, reason: null, error: null, updating: false } as T);
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
    // The browser reconnects on its own after the daemon restarts, but what changed in between is not sent
    // again: tell listeners so a page can reload what it shows.
    let dropped = false;
    es.onerror = () => {
      dropped = true;
    };
    es.onopen = () => {
      if (!dropped) return;
      dropped = false;
      const ev: KiqrEvent = { type: "reconnected", project: null, at: new Date().toISOString() };
      for (const l of entry.listeners) l(ev);
    };
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
