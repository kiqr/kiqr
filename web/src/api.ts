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
export type SetupWhy = { kind: "site" } | { kind: "network"; name: string } | { kind: "goal"; goal: string };
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
  /** The account the user approved it for (Ready), and the place in it; when the daemon publishes it (Scheduled); why a scheduled publish failed. */
  channel?: string | null;
  place?: string | null;
  publish_at?: string | null;
  publish_error?: string | null;
  archived?: boolean;
  publications: { connector: string; place?: string | null; account?: string; url?: string; at: string; run?: string }[];
};
export type TypeCount = { type: string; count: number };
export type NewAsset = { type: string; title: string; tags?: string[]; summary?: string; source?: Record<string, unknown>; files?: { role: string; name: string; text: string }[] };

export type KiqrEvent = { at: string; project: string | null; type: string } & Record<string, unknown>;
/** How an account's sign-in on a hosted program changed (`session_changed`). */
export type SessionChangedEvent = { type: "session_changed"; connection: string; state: SessionState; message: string };
/** One picture of the live view (`hands_frame`), a few a second while someone watches. Never kept. */
export type HandsFrameEvent = { type: "hands_frame"; connection: string; hands: string; mime: string; data: string; url?: string };
export type HandsStartedEvent = { type: "hands_started"; connection: string; hands: string };
export type HandsEndedEvent = { type: "hands_ended"; connection: string; hands: string; message: string };
/** A program KIQR installs for an ability (machine-wide events, on the stream without a project). */
export type ProgramInstallingEvent = { type: "program_installing"; program: string; version: string; note: string };
export type ProgramInstalledEvent = { type: "program_installed"; program: string; version: string };
export type ProgramFailedEvent = { type: "program_failed"; program: string; version: string; message: string };
export const SESSION_EVENTS = ["session_changed"];
export const HANDS_EVENTS = ["hands_frame", "hands_started", "hands_ended"];
export const PROGRAM_EVENTS = ["program_installing", "program_installed", "program_failed"];

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
/** A task run coming up, or a publish the daemon will do (`kind: "publish"`, with the asset, the account and the place in it). */
export type CalendarItem = { kind?: "task" | "publish"; task?: string; asset?: { type: string; id: string; title: string }; channel?: string | null; channel_label?: string; place?: string | null; place_label?: string | null; title: string; at: string; schedule_text: string };
/** What waits for the user: open approvals from every chat, drafts to approve, what is approved but not sent, publishes coming up, failed publishes. */
/** A kind of place content can go (the code's channel kind). Installed ones carry what their ability declares; listed ones only a name. */
export type Network = { kind: string; name: string; icon: string; network: boolean; installed: boolean; text_limit?: number | null; image?: { width: number; height: number } | null; preview?: string; ability?: string };
/** A profile the business has where an ability in the marketplace could publish. */
export type Suggestion = { kind: string; name: string; profile_url: string; ability: string; ability_name: string };
export type Todo = {
  suggestions?: Suggestion[];
  approvals: { id: string; kind: string; chat: string | null; chat_title?: string; message?: string | null; created: string; payload: Record<string, unknown> }[];
  drafts: Asset[];
  ready: Asset[];
  scheduled: Asset[];
  attention: Asset[];
  /** Accounts whose sign-in needs a person (the daemon's `accounts_needing_hand`; `needs_hand` in older drafts). */
  accounts_needing_hand?: NeedsHand[];
  needs_hand?: NeedsHand[];
  count: number;
};
/** An account whose sign-in waits for a person in the live view. */
export type NeedsHand = { connection: string; label: string; message: string; service?: string; since?: string | null };
export type ToolGroup = { id: string; label: string; tools: string[] };

// ---- Abilities
export type Scope = { scope: "global" } | { scope: "project"; project: string };
/** A key an ability needs. `set` says where, for a core ability's own key (this project or the machine); an installed ability's keys live on its accounts. */
export type MarketAuth = { env: string; label: string; help: string; optional: boolean; options?: string[]; placeholder?: string; set: "project" | "global" | boolean | null };
/** One of this project's accounts of an ability: which keys it has, never their values. */
export type AccountSummary = { id: string; label: string; default: boolean; channel: boolean; kind: string; destination_text: string; missing: string[]; set: string[] };
/** A kind an ability publishes as, and the fields a place of it has. */
export type KindInfo = { kind: string; label: string; name: string; place: Record<string, PlaceField>; does?: string[] };
/** How far Seemo may go with one role on an account: on its own, after a yes, or not at all. Roles that never act know only auto (on) and off. */
export type Allow = "auto" | "ask" | "off";
/** One role an account can be allowed: `acts` roles change something out there and have three choices. */
export type Permission = { role: string; allow: Allow; acts: boolean };
/** Somewhere inside an account content can go: a subreddit, a branch, a list of people (`kind: "list"`, `fields.list`). */
export type Place = { id: string; kind: string; label: string; fields?: Record<string, unknown>; default?: boolean };
/** One field a place has, as its kind declares it. */
export type PlaceField = { type?: string; label?: string; help?: string; optional?: boolean; enum?: string[]; placeholder?: string; /** Picked from a list read through the account (`GET …/choices/{field}`), not typed. */ choices?: Record<string, unknown> | boolean | null };
/**
 * One kind of place an account can have: the kind it publishes as ("Publishes to"), or an ability of
 * its service that works on a place of its own (a GA4 property, a Search Console site: "Measures",
 * "Works on"; `kind` is that ability's id). `needs`: a required field is missing and no place of it exists yet.
 */
export type PlaceKind = { kind: string; label: string; verb: string; fields: Record<string, PlaceField>; needs: boolean; lists?: boolean };
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
export type ConnectInfo = { label: string; flow: "redirect" | "oauth2" | "cloud"; needs: string[]; returns: string[]; https_only: boolean; /** kiqr.cloud finishes this sign-in (it holds the provider's app). */ via_cloud?: boolean; provider?: string | null };
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
  channels: string[];
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
  connect?: ConnectInfo | null;
  /** A core ability KIQR signs in to: this project's own sign-in, the machine's, or none. */
  signed_in?: "project" | "machine" | null;
  /** This project's accounts of it; empty for one that needs no keys. */
  accounts?: AccountSummary[];
  /** The kinds it publishes as. */
  kinds?: KindInfo[];
  /** The place of its own it works on (a property, a site), field by field, when it has one. */
  places?: Record<string, PlaceField>;
  /** The service it is an ability of: its id, and when the daemon sends it, the card the grid shows. */
  service?: string | null;
  service_card?: ServiceCard | null;
  /** What an account of it allows Seemo at first (the manifest's `allow`). */
  allow?: Record<string, Allow>;
  /** An account of it can be a source too: what the Source switch says. */
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
/** A key on one account: set or not, never its value. */
export type AccountKey = { env: string; label: string; help: string; optional: boolean; options?: string[]; placeholder?: string; set: boolean; /** A sign-in keeps it (a token): never typed. */ written?: boolean; /** Its values are read through the account once it is signed in. */ choices?: boolean };
/** One account in this project: everything Seemo signs in to (an X account, a mail account, a site), with its own keys, what Seemo may do there, and the places inside it. */
export type Account = {
  id: string;
  label: string;
  ability: string;
  ability_name: string;
  default: boolean;
  /** It has a kind: its service publishes. */
  channel: boolean;
  /** It may be published to now (a kind, and publishing is not off). */
  publishes?: boolean;
  kind: string;
  kind_label?: string;
  kind_name?: string;
  created_by: string;
  created_at: string;
  /** What Seemo may do here, by role. */
  allow?: Permission[];
  /** Where inside it content goes. */
  places?: Place[];
  /** The fields a place here has, and whether one is needed before anything goes out. */
  place_fields?: Record<string, PlaceField>;
  needs_place?: boolean;
  /** Every kind of place it can have, with what each needs. */
  place_kinds?: PlaceKind[];
  /** Who it is out there (a mail address, a site's host), when the daemon tells. */
  identity?: string;
  /** Its places are lists of people (a mail account), when the daemon says so. */
  takes_lists?: boolean;
  service_card?: ServiceCard;
  publications: number;
  last_published?: string | null;
  connection_name: string;
  ready: boolean;
  missing: string[];
  auth: AccountKey[];
  connect?: ConnectInfo | null;
  destination_text: string;
  icon?: string;
  error?: string;
  /** Signed in one way or another (a token kept, a key file set). */
  signed_in?: boolean;
  /** The service it is an account of; every ability of that service on here uses it. */
  service?: string;
  /** Those abilities, with what each does on this channel (empty for a plain account). */
  used_by?: { id: string; name: string; does: string[] }[];
  /** A channel's: the ability that publishes to it, and those that could. */
  publisher?: string | null;
  publisher_name?: string | null;
  publishers?: { id: string; name: string }[];
  /** Learning from it is allowed: the project learns from it into the profile. */
  source?: boolean;
  can_learn?: boolean;
  /** Its ability can teach the project; what the switch says. */
  learn?: LearnInfo | null;
  /** Its ability publishes somewhere: it can be a channel. */
  can_publish?: boolean;
  last_learned?: string | null;
  /** Documents read the last time. */
  learned?: number;
  /** It signs in on a hosted program (a browser), and how that stands. */
  has_session?: boolean;
  session?: SessionStatus;
  /** Where its login lives; null when none is saved. */
  login?: LoginRef | null;
  /** Which values of a login saved in KIQR are there (names only). */
  login_set?: string[];
  /** A person can take the controls of its program. */
  live_view?: boolean;
};
export type LearnInfo = { label: string };
/** `unknown`, `checking`, `signed_in`, `signed_out`, `needs_hand`. */
export type SessionState = "unknown" | "checking" | "signed_in" | "signed_out" | "needs_hand";
/** How an account's sign-in on a hosted program stands. */
export type SessionStatus = { state: SessionState | ""; message: string; checked_at: string | null; signed_in_at: string | null };
/** Where an account's login lives: saved in KIQR (`store: "kiqr"`) or a password manager account and its item. */
export type LoginRef = { store: string; ref?: string };
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
export type Channel = Account;
/** Where KIQR listens, when a factory reset could keep it (it isn't the default). `remote`: reachable from other machines. */
export type ResetNetwork = { host: string; port: number; remote: boolean };
export type AccountActivity = { connection: string; service: string; today: number; daily_cap: number | null; activity: { at: string; ability: string; role: string; tool: string; target?: string; url?: string | null }[] };
/** A list of people the project emails. The addresses live outside git; this is the record and the counts. */
export type MailList = { id: string; label: string; description: string; contacts: number; unsubscribed: number; created_at: string; created_by?: string };
export type Contact = { email: string; name?: string; consent?: string; added?: string | null; unsubscribed?: string | null };
/** One send to a list: so many sent, so many failed, still running or not. */
export type ListSend = { key: string; sent: number; failed: number; at: string | null; running: boolean };
export type ChannelKindInfo = { kind: string; label: string; connection: string; connection_name: string; ready: boolean; place: Record<string, PlaceField>; needs_place?: boolean; takes_lists?: boolean; auth?: MarketAuth[]; connect?: ConnectInfo | null; name?: string; icon?: string; accounts?: { id: string; label: string }[]; options?: Record<string, { type?: string; enum?: string[]; description?: string }>; learn?: LearnInfo | null };

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
  dismissSuggestion: (slug: string, kind: string) => req<{ dismissed: string }>(`/api/projects/${slug}/suggestions/${encodeURIComponent(kind)}/dismiss`, { method: "POST", body: "{}" }),
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

  /** Start signing in, into one account: `connection` to sign one in again, else a new one is added first (with its `kind`, a first `place` and the permissions chosen). Saves what was asked first and returns the page to open; `callback: false` means the user pastes what it shows into the account. */
  startConnect: (slug: string, id: string, b: { connection?: string; label?: string; kind?: string; place?: Record<string, string>; allow?: Record<string, Allow>; origin: string; values: Record<string, string> }) =>
    req<{ url: string; callback: boolean; connection: string; fields: string[] }>(`/api/projects/${slug}/connect/${encodeURIComponent(id)}`, { method: "POST", body: JSON.stringify(b) }),

  // KIQR's own sign-ins: kiqr.cloud, and UnblockingAPI by account
  signIns: (slug?: string) => req<{ signins: SignIn[]; cloud_url: string }>(`/api/signins${slug ? `?project=${encodeURIComponent(slug)}` : ""}`),
  /** `redirect`: open `url`. `device`: show the code and where to enter it; the daemon waits for the approval. */
  startSignIn: (id: string, origin: string, project?: string) => req<{ mode: "redirect"; url: string } | { mode: "device"; user_code: string; verification_uri: string; verification_uri_complete?: string | null }>(`/api/signins/${id}/start`, { method: "POST", body: JSON.stringify({ origin, project }) }),
  signOut: (id: string, project?: string) => req<{ signed_in: boolean }>(`/api/signins/${id}${project ? `?project=${encodeURIComponent(project)}` : ""}`, { method: "DELETE" }),

  // Accounts: one setup of an ability in a project, with its own keys
  accounts: (slug: string, ability?: string) => req<{ connections: Account[] }>(`/api/projects/${slug}/connections${ability ? `?ability=${encodeURIComponent(ability)}` : ""}`),
  account: (slug: string, id: string) => req<Account>(`/api/projects/${slug}/connections/${id}`),
  /** A new account: its kind and first place when it publishes, its keys, what Seemo may do there (over the manifest's defaults). */
  createAccount: (slug: string, b: { ability: string; label?: string; kind?: string; place?: Record<string, string>; place_label?: string; secrets?: Record<string, string>; same_as?: string; allow?: Record<string, Allow> }) => req<Account>(`/api/projects/${slug}/connections`, { method: "POST", body: JSON.stringify(b) }),
  updateAccount: (slug: string, id: string, p: { label?: string; allow?: Record<string, Allow>; publisher?: string; kind?: string; place?: Record<string, unknown>; login?: LoginRef }) => req<Account>(`/api/projects/${slug}/connections/${id}`, { method: "PUT", body: JSON.stringify(p) }),
  /** What Seemo may do here: role → auto, ask or off (on/off for roles that never act). */
  setAllow: (slug: string, id: string, allow: Record<string, Allow>) => req<Account>(`/api/projects/${slug}/connections/${id}/allow`, { method: "PUT", body: JSON.stringify(allow) }),
  /** A place inside an account: the kind's fields, or a list of people (`list`, its id) on a mail account. */
  addPlace: (slug: string, id: string, b: { kind?: string; label?: string; fields?: Record<string, string>; default?: boolean; list?: string }) => req<Account & { place: Place }>(`/api/projects/${slug}/connections/${id}/places`, { method: "POST", body: JSON.stringify(b) }),
  updatePlace: (slug: string, id: string, place: string, b: { label?: string; fields?: Record<string, string>; default?: boolean }) => req<Account>(`/api/projects/${slug}/connections/${id}/places/${encodeURIComponent(place)}`, { method: "PUT", body: JSON.stringify(b) }),
  removePlace: (slug: string, id: string, place: string) => req<Account>(`/api/projects/${slug}/connections/${id}/places/${encodeURIComponent(place)}`, { method: "DELETE" }),
  deleteAccount: (slug: string, id: string) => req<void>(`/api/projects/${slug}/connections/${id}`, { method: "DELETE" }),
  /** One key of an account; an empty value clears it. Never read back. */
  accountChoices: (slug: string, id: string, name: string) => req<{ name: string; choices: { value: string; label: string }[] }>(`/api/projects/${slug}/connections/${id}/choices/${name}`),
  setAccountSecret: (slug: string, id: string, name: string, value: string) => req<{ name: string; set: boolean }>(`/api/projects/${slug}/connections/${id}/secrets/${name}`, { method: "PUT", body: JSON.stringify({ value }) }),
  /** Signing in on a hosted program: check and sign in now (in the background; `session_changed` says how it went), forget it, or read how it stands. */
  accountSignIn: (slug: string, id: string) => req<{ started: boolean; note?: string; session?: SessionStatus }>(`/api/projects/${slug}/connections/${id}/signin`, { method: "POST", body: "{}" }),
  accountSignOut: (slug: string, id: string) => req<{ session: SessionStatus }>(`/api/projects/${slug}/connections/${id}/signout`, { method: "POST", body: "{}" }),
  accountSession: (slug: string, id: string) => req<{ has_session: boolean; session: SessionStatus; login: LoginRef | null; live_view: boolean }>(`/api/projects/${slug}/connections/${id}/session`),
  /** Take the controls of the account's program (plain same-origin requests: the daemon checks where they come from). */
  startHands: (slug: string, id: string, url?: string) => req<{ hands: string; token: string; can: HandsCan }>(`/api/projects/${slug}/connections/${id}/hands`, { method: "POST", body: JSON.stringify(url ? { url } : {}) }),
  handsInput: (slug: string, hands: string, token: string, input: HandsInput) => req<{ ok: boolean }>(`/api/projects/${slug}/hands/${encodeURIComponent(hands)}/input`, { method: "POST", body: JSON.stringify({ token, ...input }) }),
  endHands: (slug: string, hands: string, token: string, signedIn: boolean) => req<{ ended: boolean }>(`/api/projects/${slug}/hands/${encodeURIComponent(hands)}/end`, { method: "POST", body: JSON.stringify({ token, signed_in: signedIn }), keepalive: true }),
  /** The account its ability uses when nothing names one. */
  defaultAccount: (slug: string, id: string) => req<Account>(`/api/projects/${slug}/connections/${id}/default`, { method: "POST", body: "{}" }),

  // Publishing. `/channels` is the older view: the accounts that publish, and the kinds with their per-publish options.
  channels: (slug: string) => req<{ channels: Channel[]; kinds: ChannelKindInfo[] }>(`/api/projects/${slug}/channels`),
  learnAccount: (slug: string, id: string) => req<{ started: boolean }>(`/api/projects/${slug}/connections/${id}/learn`, { method: "POST" }),
  accountActivity: (slug: string, id: string, limit = 20) => req<AccountActivity>(`/api/projects/${slug}/connections/${id}/activity?limit=${limit}`),
  /** Publish an asset now, to the account (and place) it was approved for or the one given. From the UI the click is the yes. */
  publishAsset: (slug: string, a: Pick<Asset, "type" | "id">, account = "", note = "", options: Record<string, unknown> = {}, place = "") =>
    req<{ published: boolean; url: string | null; detail: string; channel_label: string; place_label?: string | null; recipients?: number }>(`/api/projects/${slug}/assets/${a.type}/${a.id}/publish`, { method: "POST", body: JSON.stringify({ channel: account, place, note, options, by: "user" }) }),
  /** The user's yes to an asset, for an account and a place in it: it becomes Ready. Status is never set by hand; these change what it follows from. */
  approveAsset: (slug: string, a: Pick<Asset, "type" | "id">, account: string, place = "") => req<Asset>(`/api/projects/${slug}/assets/${a.type}/${a.id}/approve`, { method: "POST", body: JSON.stringify({ channel: account, place }) }),
  scheduleAsset: (slug: string, a: Pick<Asset, "type" | "id">, at: string, account = "", options: Record<string, unknown> = {}, place = "") => req<Asset>(`/api/projects/${slug}/assets/${a.type}/${a.id}/schedule`, { method: "POST", body: JSON.stringify({ at, channel: account, place, options }) }),
  unscheduleAsset: (slug: string, a: Pick<Asset, "type" | "id">) => req<Asset>(`/api/projects/${slug}/assets/${a.type}/${a.id}/schedule`, { method: "DELETE" }),
  unpublishAsset: (slug: string, a: Pick<Asset, "type" | "id">) => req<Asset>(`/api/projects/${slug}/assets/${a.type}/${a.id}/unpublish`, { method: "POST", body: "{}" }),
  archiveAsset: (slug: string, a: Pick<Asset, "type" | "id">, archived: boolean) => req<Asset>(`/api/projects/${slug}/assets/${a.type}/${a.id}`, { method: "PATCH", body: JSON.stringify({ archived }) }),
  sendEmail: (slug: string, m: { to: string; subject: string; text: string; asset?: { type: string; id: string } }) => req<{ sent: boolean; to: string; subject: string; connection: string }>(`/api/projects/${slug}/email`, { method: "POST", body: JSON.stringify(m) }),

  // Lists: the people the project emails (Audience)
  lists: (slug: string) => req<{ lists: MailList[] }>(`/api/projects/${slug}/lists`),
  list: (slug: string, id: string) => req<MailList>(`/api/projects/${slug}/lists/${encodeURIComponent(id)}`),
  createList: (slug: string, b: { label: string; description?: string }) => req<MailList>(`/api/projects/${slug}/lists`, { method: "POST", body: JSON.stringify(b) }),
  updateList: (slug: string, id: string, b: { label?: string; description?: string }) => req<MailList>(`/api/projects/${slug}/lists/${encodeURIComponent(id)}`, { method: "PUT", body: JSON.stringify(b) }),
  deleteList: (slug: string, id: string) => req<void>(`/api/projects/${slug}/lists/${encodeURIComponent(id)}`, { method: "DELETE" }),
  contacts: (slug: string, id: string) => req<{ list: string; contacts: Contact[] }>(`/api/projects/${slug}/lists/${encodeURIComponent(id)}/contacts`),
  /** Add people; `consent` is how they agreed to hear from the business, in the user's words. */
  addContacts: (slug: string, id: string, contacts: { email: string; name?: string }[], consent: string) => req<{ added: number; list: MailList }>(`/api/projects/${slug}/lists/${encodeURIComponent(id)}/contacts`, { method: "POST", body: JSON.stringify({ contacts, consent }) }),
  removeContact: (slug: string, id: string, email: string) => req<{ removed: boolean; list: MailList }>(`/api/projects/${slug}/lists/${encodeURIComponent(id)}/contacts`, { method: "DELETE", body: JSON.stringify({ email }) }),
  /** A CSV file's text: an email column (or a column of addresses), a name column when there is one. */
  importContacts: (slug: string, id: string, csv: string, consent: string) => req<{ found: number; added: number; list: MailList }>(`/api/projects/${slug}/lists/${encodeURIComponent(id)}/import`, { method: "POST", body: JSON.stringify({ csv, consent }) }),
  unsubscribe: (slug: string, id: string, email: string) => req<{ unsubscribed: boolean; list: MailList }>(`/api/projects/${slug}/lists/${encodeURIComponent(id)}/unsubscribe`, { method: "POST", body: JSON.stringify({ email }) }),
  listSends: (slug: string, id: string) => req<{ list: string; sends: ListSend[] }>(`/api/projects/${slug}/lists/${encodeURIComponent(id)}/sends`),
};

let demoTelemetry = true;
let demoConfig: Config = {
  daemon: { host: "127.0.0.1", port: 4040 },
  crawl: { max_pages: 200, concurrency: 5, ai_concurrency: 3 },
  models: { extract: "claude-cli/haiku", synthesize: "claude-cli/sonnet", chat: "claude-cli/sonnet" },
  agent: { max_budget_usd: 2, max_concurrent: 3, timezone: "Europe/Stockholm" },
  abilities: {},
};

let demoIntake: Intake = { goal: "", goal_note: "", customers: "", time: "", networks: [], skipped: [] };

function demoRequest<T>(path: string, init?: RequestInit): Promise<T> {
  if (path.startsWith("/api/signins")) return Promise.resolve({ signins: [{ id: "kiqr-cloud", name: "kiqr.cloud", signed_in: false, account: null, waiting: null }, { id: "unblockingapi", name: "UnblockingAPI", signed_in: false, account: null, waiting: null }], cloud_url: "https://kiqr.cloud" } as T);
  if (/\/(chats|approvals|tasks|calendar|channels|connections|email|secrets|abilities|connect|networks|suggestions|todo|plan|campaigns|performance|lists|hands)(\/|\?|$)/.test(path) || path === "/api/tools" || path.startsWith("/api/abilities") || path.startsWith("/api/secrets")) return demoSeemoRequest<T>(path, init);
  if (path === "/api/projects" && init?.method === "POST") {
    demoState.created = true;
    return Promise.resolve(demoProject as T);
  }
  if (path === "/api/projects") return Promise.resolve((demoState.created ? [demoProject] : []) as T);
  if (path.startsWith("/api/projects/") && path.split("/").length === 4) return Promise.resolve(demoProject as T);
  if (path.endsWith("/brand/kit")) return Promise.resolve({ site_name: "UnblockingAPI", palette: { dark: "#0b1220", light: "#f8fafc", accent: "#e11d74" }, logo: null, html_available: true } as T);
  if (path === "/api/asset-types" || path.includes("/assets")) return demoAssetRequest<T>(path, init);
  if (path.endsWith("/brief")) return Promise.resolve((init?.method === "PUT" ? JSON.parse(String(init.body)) : demoState.created ? demoBrief : null) as T);
  if (path.endsWith("/build")) return Promise.resolve({ stage: "crawling", pct: 0, eta_secs: null, max_pages: 200, found: 0, fetched: 0, queued: 0, read: 0, facts: 0, sections_done: 0, sections_total: 12, running: true, started_at: null, last_at: null, problem: null } as T);
  if (path.endsWith("/intake")) {
    if (init?.method === "PUT") demoIntake = JSON.parse(String(init.body));
    return Promise.resolve(demoIntake as T);
  }
  if (path.endsWith("/setup")) {
    const avatar = (id: string) => `https://raw.githubusercontent.com/kiqr/marketplace/main/${id}/avatar.svg`;
    const s: Setup = {
      suggestions: [
        { id: "google-analytics", name: "Read Google Analytics", summary: "Sessions, users, page views and key events, once a day into Performance.", icon: "chart", avatar: avatar("google-analytics"), why: [{ kind: "site" }], installed: true, on: true, ready: true },
        { id: "search-console", name: "Read Google Search Console", summary: "Clicks, impressions and the queries people find you with, once a day into Performance.", icon: "search", avatar: avatar("search-console"), why: [{ kind: "site" }, ...(demoIntake.goal === "customers" ? [{ kind: "goal" as const, goal: "customers" }] : [])], installed: false, on: false, ready: false },
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
