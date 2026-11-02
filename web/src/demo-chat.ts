// Demo mode for Seemo: a saved conversation, a task, and a scripted reply
// when you send something, so the chat screens work without a daemon.
import type {Ability, AbilitiesView, AbilitySetting, CalendarItem, Campaign, Channel, ChannelKind, ChannelMode, Chat, ChatView, Contact, Credential, CredentialTemplate, KiqrEvent, KindInfo, ListSend, MailList, Message, Performance, Plan, PlanView, Run, SessionStatus, SpecField, Spot, Task, Todo, AccessNeed, CredentialSession, Recommendation} from "./api";
import { demoAssetList, setDemoTargets, type DemoTarget } from "./demo-assets";

const slug = "unblockingapi-com";
const ago = (min: number) => new Date(Date.now() - min * 60_000).toISOString();
const DEMO_IMAGE =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif"><rect width="1600" height="900" fill="#0B1220"/><rect x="128" y="128" width="12" height="644" fill="#ef2f72"/><text x="188" y="330" fill="#FFFFFF" font-size="88" font-weight="700">One request.</text><text x="188" y="436" fill="#FFFFFF" font-size="88" font-weight="700">One credit.</text><text x="188" y="540" fill="#CBD5E1" font-size="36">No multipliers. Failed requests are free.</text><text x="188" y="744" fill="#ef2f72" font-size="30" font-weight="600">UnblockingAPI</text></svg>`,
  );

const key = (env: string, label: string, more: Partial<AccountKey> = {}): AccountKey => ({ env, label, help: "", optional: false, set: true, ...more });
// The demo still keeps its sign-ins the way the old accounts did; only the demo reads these shapes.
type AccountKey = { env: string; label: string; help: string; optional: boolean; options?: string[]; placeholder?: string; set: boolean; written?: boolean; choices?: boolean };
type AccountState = { state: string; ready: boolean; says: string; keys?: string[] };
type Account = { id: string; label: string; service: string; default: boolean; created_by: string; created_at: string; state: AccountState; ready: boolean; channels?: { id: string; label: string; domain: string; ownership: Channel["ownership"] }[]; identity?: string | null; label_set?: boolean; service_card?: { id: string; name?: string; icon?: string; group?: string }; auth: AccountKey[]; connect?: unknown; signed_in?: boolean; used_by?: { id: string; name: string; does: string[] }[]; has_session?: boolean; session?: SessionStatus; has_login?: boolean; secrets?: unknown; login_set?: string[]; live_view?: boolean; by_hand?: boolean; start_url?: string | null };
/** The demo's abilities carry what the daemon used to send too (a service, a connect block); the pages read only what `Ability` has. */
type DemoAbility = Ability & { service?: string | null; connect?: unknown };
type DemoAbilitiesView = Omit<AbilitiesView, "abilities" | "market"> & { abilities: DemoAbility[]; market: DemoAbility[] };
const IN_KIQR = { store: "kiqr", store_label: "KIQR", in_kiqr: true };
const GOOGLE_CARD = { id: "google", name: "Google", icon: "chart", group: "numbers" };
const GA_SETTINGS: Record<string, SpecField> = { property: { label: "GA4 property", help: "The property whose numbers Seemo reads", choices: { list: "/properties" } } };
const SC_SETTINGS: Record<string, SpecField> = { site: { label: "Search Console property", help: "A domain (sc-domain:…) or a URL prefix", choices: { list: "/siteEntry" } } };
const SUBREDDIT: Record<string, SpecField> = { subreddit: { label: "Subreddit", placeholder: "webscraping", help: "The name without r/", pattern: "^[A-Za-z0-9_]{2,21}$" } };
/** What a demo Google account has to pick from. */
const demoChoices: Record<string, { value: string; label: string }[]> = {
  property: [{ value: "properties/318204551", label: "unblockingapi.com" }, { value: "properties/318204777", label: "docs.unblockingapi.com" }],
  site: [{ value: "sc-domain:unblockingapi.com", label: "sc-domain:unblockingapi.com" }, { value: "https://docs.unblockingapi.com/", label: "https://docs.unblockingapi.com/" }],
};
const session = (state: SessionStatus["state"], message: string, signedIn: string | null = null): SessionStatus => ({ state, message, checked_at: ago(12), signed_in_at: signedIn });

/** A demo account before `settle` works out its state, readiness and channels, as the daemon does on every read. */
type Seed = Omit<Account, "state" | "ready" | "channels">;
const seeds: Seed[] = [
  // One Google sign-in, read by Google Analytics and Search Console.
  { id: "google", label: "Google", service: "google", default: true, created_by: "user", created_at: ago(4000), identity: "team@unblockingapi.com", auth: [key("GOOGLE_TOKEN", "Sign-in", { written: true })], connect: { label: "Sign in with Google", flow: "oauth2", needs: [], returns: ["GOOGLE_TOKEN"], https_only: false, via_cloud: true }, signed_in: true, service_card: GOOGLE_CARD, secrets: IN_KIQR, used_by: [{ id: "google-analytics", name: "Read Google Analytics", does: ["measure"] }, { id: "search-console", name: "Read Google Search Console", does: ["measure"] }] },
  // A mail account: it sends to the lists.
  { id: "hello-mail", label: "hello@unblockingapi.com", service: "kiqr-smtp", default: true, created_by: "user", created_at: ago(300), identity: "hello@unblockingapi.com", auth: [key("SMTP_HOST", "Mail server"), key("SMTP_USER", "User name"), key("SMTP_PASSWORD", "Password")], signed_in: true, service_card: { id: "kiqr-smtp", name: "Email", icon: "mail", group: "email" }, secrets: IN_KIQR, used_by: [{ id: "kiqr-smtp", name: "Send email from your mail account", does: ["publish"] }] },
  // Signed in on KIQR's own browser from a login saved in KIQR.
  { id: "x-unblockingapi", label: "@unblockingapi", service: "x", default: true, created_by: "user", created_at: ago(1500), identity: "@unblockingapi", auth: [], signed_in: true, service_card: { id: "x", name: "X", icon: "at-sign", group: "social" }, secrets: IN_KIQR, used_by: [{ id: "x-browser", name: "X in a browser", does: ["read", "reply", "publish"] }], has_session: true, session: session("signed_in", "signed in", ago(3000)), has_login: true, login_set: ["LOGIN_USERNAME", "LOGIN_PASSWORD", "LOGIN_TOTP_SECRET"], live_view: true, start_url: "https://x.com/login" },
  // Its login is saved, but the browser is signed out: the page offers Sign in.
  { id: "reddit-team", label: "u/unblockingapi_team", service: "reddit", default: true, created_by: "user", created_at: ago(2000), identity: "u/unblockingapi_team", auth: [], signed_in: false, service_card: { id: "reddit", name: "Reddit", icon: "message-square-text", group: "social" }, secrets: IN_KIQR, used_by: [{ id: "reddit-browser", name: "Reddit in a browser", does: ["read", "reply", "publish"] }], has_session: true, session: session("signed_out", "signed out"), has_login: true, login_set: ["LOGIN_USERNAME", "LOGIN_PASSWORD"], live_view: true, start_url: "https://www.reddit.com/login" },
  // The blog.
  { id: "wordpress-site", label: "unblockingapi.com", service: "wordpress", default: true, created_by: "user", created_at: ago(2500), identity: "unblockingapi.com", auth: [key("WP_URL", "Site address"), key("WP_USER", "User name"), key("WP_APP_PASSWORD", "Application password", { written: true })], signed_in: true, service_card: { id: "wordpress", name: "WordPress", icon: "globe", group: "website" }, secrets: IN_KIQR, used_by: [{ id: "wordpress", name: "Publish articles to WordPress", does: ["publish"] }] },
];

/** Whether an account works, by the daemon's one rule: keys, then the sign-in on its program. */
function stateOf(a: Seed): AccountState {
  const keys = a.auth.filter((k) => !k.optional && !k.set).map((k) => k.env);
  if (keys.length) return { state: "missing_keys", ready: false, says: `Add ${a.auth.filter((k) => keys.includes(k.env)).map((k) => k.label.toLowerCase()).join(", ")} to use it.`, keys };
  if (a.has_session) {
    const s = a.session?.state;
    if (s === "signed_in") return { state: "ready", ready: true, says: "Signed in." };
    if (s === "checking") return { state: "checking", ready: false, says: "Checking the sign-in…" };
    if (s === "needs_hand") return { state: "needs_hand", ready: false, says: `The site asks for a person: ${a.session?.message || "sign in once in the live view"}.` };
    return { state: "signed_out", ready: false, says: a.has_login ? "Signed out. Sign in with the saved login." : "Signed out. Save a login or sign in by hand." };
  }
  return { state: "ready", ready: true, says: "Ready." };
}

const demoAccounts: Account[] = [];
/** Work out what the daemon would say about an account now: its state and the channels it publishes to. */
function settle(a: Account | Seed): Account {
  const c = a as Account;
  c.state = stateOf(c);
  c.ready = c.state.ready;
  c.channels = allChannels().filter((e) => e.publisher === c.id && !e.suggested).map((e) => ({ id: e.id, label: e.label, domain: e.domain, ownership: e.ownership }));
  if (c.has_session) c.signed_in = c.session?.state === "signed_in";
  return c;
}

// ---- Channels: where the business's people are

/** What can be added here, by kind, with the service whose accounts publish it. */
const KINDS: (ChannelKind & { service?: string })[] = [
  { kind: "x", label: "X profile", name: "X", icon: "at-sign", domain: "x.com", ownership: "owned", target: {}, account_is_target: true, publishes: true, ability: "x-browser", ability_name: "X in a browser", service: "x" },
  { kind: "wordpress", label: "Blog", name: "WordPress", icon: "globe", domain: "", ownership: "owned", target: {}, account_is_target: true, publishes: true, ability: "wordpress", ability_name: "WordPress", service: "wordpress" },
  { kind: "reddit", label: "Subreddit", name: "Reddit", icon: "message-square-text", domain: "reddit.com", ownership: "third_party", target: SUBREDDIT, account_is_target: false, publishes: true, ability: "reddit-browser", ability_name: "Reddit in a browser", service: "reddit" },
  { kind: "list", label: "List", name: "Mailing list", icon: "mail", domain: "", ownership: "owned", target: {}, account_is_target: false, publishes: true },
];
/** The service whose accounts send to lists. */
const MAIL_SERVICE = "kiqr-smtp";
const kindsView = (): ChannelKind[] => KINDS.map(({ service: _s, ...k }) => k);

/** A channel as kept, before its publisher and readiness are looked at. Lists are kept with their people below. */
type Kept = Omit<Channel, "publisher_view" | "publishers" | "readiness">;
const BLOG_OPTIONS = { status: { type: "string", enum: ["draft", "publish"], description: "Save it as a draft on the site, or publish it" } };
const modes = (publish: ChannelMode, reply: ChannelMode) => ({ read: true, publish, reply, learn: false });
const sub = (name: string, publisher: string, more: Partial<Kept> = {}): Kept => ({ id: `r-${name.toLowerCase()}`, label: `r/${name}`, ownership: "third_party", domain: "reddit.com", kind: "reddit", publisher, fields: { subreddit: name }, url: `https://www.reddit.com/r/${name}/`, ...modes("ask", "auto"), added_by: "user", suggested: false, created_at: ago(3000), kind_name: "Reddit", kind_label: "Subreddit", icon: "message-square-text", publishes: true, network: true, target: SUBREDDIT, ...more });
const entries: Kept[] = [
  // Posts on its own: the card in the chat is the record.
  { id: "x-unblockingapi", label: "@unblockingapi", ownership: "owned", domain: "x.com", kind: "x", publisher: "cred_x01", fields: {}, url: "https://x.com/unblockingapi", ...modes("auto", "ask"), added_by: "account", suggested: false, created_at: ago(1500), publications: 4, last_published: ago(600), kind_name: "X", kind_label: "X profile", icon: "at-sign", publishes: true, network: true, ability_name: "X in a browser" },
  { id: "blog", label: "unblockingapi.com blog", ownership: "owned", domain: "unblockingapi.com", kind: "wordpress", publisher: "cred_w01", fields: {}, url: "https://unblockingapi.com/blog/", description: "Articles and guides.", ...modes("ask", "off"), added_by: "account", suggested: false, created_at: ago(2500), publications: 6, last_published: ago(4000), kind_name: "WordPress", kind_label: "Blog", icon: "globe", publishes: true, network: false, options: BLOG_OPTIONS, ability_name: "WordPress" },
  // Found on the website: no ability here publishes to LinkedIn yet.
  { id: "linkedin", label: "UnblockingAPI on LinkedIn", ownership: "owned", domain: "linkedin.com", kind: "", publisher: "", fields: {}, url: "https://www.linkedin.com/company/unblockingapi", ...modes("ask", "ask"), added_by: "crawl", suggested: false, created_at: ago(5000), publishes: false },
  // Its publisher is signed out: the row says so.
  sub("webscraping", "cred_r01", { publications: 2, last_published: ago(25) }),
  // Nobody to post as yet.
  sub("Python", "", { created_at: ago(1800) }),
  sub("datasets", "", { added_by: "seemo", suggested: true, reason: "People ask here where to get data from sites that block scrapers; three such threads this week went unanswered." }),
  sub("dataengineering", "", { added_by: "seemo", suggested: true, reason: "Pipelines that pull from websites come up weekly, and the template library answers most of them." }),
];

type DemoList = MailList & { people: Contact[]; publish?: ChannelMode };
const person = (i: number, unsub = false): Contact => ({ email: `${["ana", "ben", "chloe", "dev", "eli", "fay", "gus", "hana", "ivo", "jon", "kim", "lou"][i % 12]}${i > 11 ? i : ""}@example.com`, name: ["Ana Ruiz", "Ben Ode", "Chloe Park", "Dev Rao", "Eli Stone", "Fay Lin", "Gus Berg", "Hana Ito", "Ivo Petrov", "Jon Hale", "Kim Moss", "Lou Amar"][i % 12], consent: "Signed up on the website", added: ago(1000 + i * 600), unsubscribed: unsub ? ago(200) : null });
const demoLists: DemoList[] = [
  { id: "newsletter", label: "Newsletter", description: "Everyone who signed up on the website for product news.", publisher: "cred_m01", contacts: 412, unsubscribed: 3, created_at: ago(20_000), added_by: "user", people: Array.from({ length: 14 }, (_, i) => person(i, i === 4)) },
  { id: "customers", label: "Customers", description: "Paying customers, for plan and product changes.", publisher: "cred_m01", contacts: 88, unsubscribed: 0, created_at: ago(9000), added_by: "user", people: Array.from({ length: 6 }, (_, i) => person(i + 3)) },
];
const listView = ({ people: _p, publish: _m, ...l }: DemoList): MailList => l;
const demoSends: Record<string, ListSend[]> = { newsletter: [{ key: "email-october-news", sent: 409, failed: 3, at: ago(8000), running: false }] };
const listEntry = (l: DemoList): Kept => ({ id: l.id, label: l.label, ownership: "owned", domain: "", kind: "list", publisher: l.publisher, description: l.description, ...modes(l.publish ?? "ask", "off"), added_by: l.added_by ?? "user", suggested: false, created_at: l.created_at, contacts: l.contacts, unsubscribed: l.unsubscribed, kind_name: "Mailing list", icon: "mail", publishes: true, network: false });

/** The service whose accounts publish a channel's kind (a list: mail). */
const serviceOfKind = (kind: string) => (kind === "list" ? MAIL_SERVICE : KINDS.find((k) => k.kind === kind)?.service);

/** What publishing to a kind asks for: the template and the site a publisher's credential is for. */
const PUBLISHER_NEED: Record<string, { ability: string; ability_name: string; template: string; hosts: string[]; describe: string }> = {
  x: { ability: "x-browser", ability_name: "Read, reply and post on X", template: "login", hosts: ["x.com"], describe: "a login for x.com" },
  reddit: { ability: "reddit-browser", ability_name: "Read, reply and post on Reddit", template: "login", hosts: ["reddit.com"], describe: "a login for reddit.com" },
  wordpress: { ability: "wordpress", ability_name: "Publish articles to WordPress", template: "app_password", hosts: [], describe: "an app password" },
  list: { ability: "kiqr-smtp", ability_name: "Send email from your mail account", template: "smtp", hosts: [], describe: "a mail server" },
};
const fitsNeed = (c: Credential, n: { template: string; hosts: string[] }) => c.template === n.template && (!n.hosts.length || c.domains.some((d) => n.hosts.some((h) => d === h || d.endsWith(`.${h}`) || h.endsWith(`.${d}`))));
const credState = (c: Credential) => (c.missing.length ? { state: "missing_fields" as const, ready: false, says: `Missing ${c.missing.join(", ")}` } : c.id === "cred_r01" ? { state: "signed_out" as const, ready: false, says: "Signed out" } : { state: "ready" as const, ready: true, says: "Ready" });

/** Whether a channel can be published to now, as the daemon says it. */
function readinessOf(e: Kept, c: Credential | undefined): Channel["readiness"] {
  if (e.suggested) return { state: "suggested", ready: false, text: "Suggested by Seemo: add it, or dismiss it", fix: { action: "add", label: "Add" } };
  if (e.kind !== "list" && !serviceOfKind(e.kind)) return { state: "no_ability", ready: false, text: `No ability here publishes to ${e.domain || "this site"}`, fix: { action: "abilities", label: "Find one" } };
  if (e.publish === "off") return { state: "publish_off", ready: false, text: "Publishing is off", fix: null };
  if (!e.publisher) return { state: "no_publisher", ready: false, text: e.kind === "list" ? "No mail server to send from" : `No publisher: pick who Seemo posts as on ${e.domain || "it"}`, fix: { action: "publisher", label: "Pick a publisher" } };
  if (!c) return { state: "publisher_gone", ready: false, text: "The credential it published as is gone", fix: { action: "publisher", label: "Pick another" } };
  const st = credState(c);
  if (!st.ready) return { state: "publisher_not_ready", ready: false, text: `${c.label}: ${st.says.toLowerCase()}`, fix: { action: "credential", label: "See to it", credential: c.id } };
  return { state: "ready", ready: true, text: "", fix: null };
}

/** A channel as the daemon shows it: who publishes and whether that works, who could, and the one fix. */
function channelView(e: Kept): Channel {
  const c = demoCredentials.find((x) => x.id === e.publisher);
  const need = PUBLISHER_NEED[e.kind];
  const pub = (x: Credential) => ({ credential: x.id, label: x.identity ?? x.label, domain: x.domains[0], template: x.template, state: credState(x).state, ready: credState(x).ready });
  return { ...e, publisher_view: c ? pub(c) : null, publishers: need ? demoCredentials.filter((x) => fitsNeed(x, need)).map(pub) : [], publisher_need: need ? { ...need, template: [need.template], provider: "", scopes: [] } : null, readiness: readinessOf(e, c), can_learn: false };
}
const allChannels = (): Kept[] => [...entries, ...demoLists.map(listEntry)];
const channelsView = () => ({ channels: allChannels().map(channelView), kinds: kindsView() });
const findEntry = (id: string) => allChannels().find((e) => e.id === id);
const emitChannel = (e: Kept, contacts?: number) => emit({ type: "channel_saved", channel: e.id, label: e.label, ownership: e.ownership, suggested: e.suggested, contacts });

/** Add a channel, as `POST /channels` does: a list, a kind with its fields, or a link or site. Checked, not twice. */
function addChannel(b: { kind?: string; url?: string; domain?: string; fields?: Record<string, unknown>; label?: string; description?: string; ownership?: string; publisher?: string }, by: string): Channel {
  let kind = String(b.kind ?? "").trim().toLowerCase();
  if (kind === "list") {
    if (!String(b.label ?? "").trim()) throw new Error("a list needs a name");
    const id = String(b.label).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
    if (demoLists.some((l) => l.id === id)) throw new Error(`there is a list called '${b.label}' already`);
    const l: DemoList = { id, label: String(b.label).trim(), description: b.description ?? "", publisher: b.publisher || demoAccounts.find((c) => c.service === MAIL_SERVICE)?.id || "", contacts: 0, unsubscribed: 0, created_at: ago(0), added_by: by, people: [] };
    demoLists.push(l);
    emitChannel(listEntry(l), 0);
    return channelView(listEntry(l));
  }
  const url = String(b.url ?? "").trim();
  const host = (url ? url.replace(/^[a-z]+:\/\//i, "").split(/[/?#]/)[0] : String(b.domain ?? "")).replace(/^www\./, "").toLowerCase();
  let fields: Record<string, unknown> = { ...(b.fields ?? {}) };
  // A link names its kind and fields: reddit.com/r/<name>, x.com/<handle>.
  const r = /reddit\.com\/r\/([A-Za-z0-9_]{2,21})/i.exec(url);
  if (!kind && r) (kind = "reddit"), (fields = { subreddit: r[1] });
  else if (!kind && host === "x.com") kind = "x";
  const k = KINDS.find((x) => x.kind === kind);
  if (kind && !k) throw new Error(`nothing installed here declares '${kind}'`);
  if (!k && !host) throw new Error("name the channel's site or give a link to it");
  if (k?.kind === "reddit") {
    const v = String(fields.subreddit ?? "").trim().replace(/^r\//i, "");
    if (!/^[A-Za-z0-9_]{2,21}$/.test(v)) throw new Error(v ? `'${v}' is not a valid subreddit` : "Reddit needs subreddit");
    fields = { subreddit: v };
  } else fields = {};
  const had = entries.find((e) => e.kind === (k?.kind ?? "") && JSON.stringify(e.fields ?? {}).toLowerCase() === JSON.stringify(fields).toLowerCase() && (k?.kind === "reddit" || (e.url ?? "") === url));
  if (had && !had.suggested) throw new Error(`this is already the channel ${had.label}`);
  const label = String(b.label ?? "").trim() || (k?.kind === "reddit" ? `r/${String(fields.subreddit)}` : url ? url.replace(/^[a-z]+:\/\/(www\.)?/i, "").replace(/\/$/, "") : host);
  const ownership = (b.ownership === "owned" || b.ownership === "third_party" ? b.ownership : k?.ownership ?? "owned") as Kept["ownership"];
  const e: Kept = had
    ? Object.assign(had, { suggested: false, reason: undefined })
    : { id: k?.kind === "reddit" ? `r-${String(fields.subreddit).toLowerCase()}` : label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, ""), label, ownership, domain: k?.domain || host, kind: k?.kind ?? "", publisher: b.publisher ?? "", fields, url: url || (k?.kind === "reddit" ? `https://www.reddit.com/r/${String(fields.subreddit)}/` : undefined), description: b.description, ...modes("ask", "ask"), added_by: by, suggested: false, created_at: ago(0), kind_name: k?.name, kind_label: k?.label, icon: k?.icon, publishes: !!k?.publishes, network: k?.kind === "x" || k?.kind === "reddit", target: k?.target, ability_name: k?.ability_name };
  if (!had) entries.push(e);
  emitChannel(e);
  return channelView(e);
}

/** Where a publish goes: a channel, or a spot that becomes one with the yes. Used by the demo library. */
function resolveTarget(where: { target?: string; spot?: Spot }): DemoTarget | null {
  let e = where.target ? findEntry(where.target) : undefined;
  if (!e && where.spot?.kind) {
    const fields = where.spot.fields ?? {};
    e = entries.find((x) => x.kind === where.spot!.kind && JSON.stringify(x.fields ?? {}).toLowerCase() === JSON.stringify(fields).toLowerCase() && !x.suggested);
    if (!e) {
      const made = addChannel({ kind: where.spot.kind, fields, publisher: demoAccounts.find((x) => x.service === serviceOfKind(where.spot!.kind))?.id }, "user");
      e = findEntry(made.id);
    }
  }
  if (!e) return null;
  const ability = PUBLISHER_NEED[e.kind]?.ability ?? "";
  return { target: e.id, label: e.label, account: e.publisher, ability, url: e.url ?? null };
}

const demoActivity = { account: "x-unblockingapi", service: "x", today: 3, daily_cap: 20, activity: [
  { at: ago(12), ability: "x-browser", role: "reply", tool: "reply", target: "a reply to @scrapingdev", channel: "x-unblockingapi", url: "https://x.com/unblockingapi" },
  { at: ago(40), ability: "x-browser", role: "reply", tool: "reply", target: "a reply to @datawrangler", channel: "x-unblockingapi", url: null },
  { at: ago(600), ability: "x-browser", role: "publish", tool: "post", target: "Templates launch thread", channel: "x-unblockingapi", url: "https://x.com/unblockingapi" },
] };
const demoAbilities: DemoAbilitiesView = {"abilities": [{"id": "write", "name": "Write articles, posts and emails", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Seemo writes in the brand's tone of voice from the business brief; everything lands in the library as a draft.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "pen-tool", "cost": "model usage", "provides": [], "publishes_to": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "business-profile", "name": "Crawl a website and build its business profile", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Crawl the site, extract what it says, and write the profile, the brand and the business brief Seemo works from.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["web", "writing"], "icon": "brain", "cost": "model usage", "provides": [], "publishes_to": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "crawl-site", "label": "Reading the site", "project_copy": false, "machine_copy": false}, {"file": "extract-facts", "label": "Extracting facts", "project_copy": false, "machine_copy": false}, {"file": "build-profile", "label": "Building the profile", "project_copy": false, "machine_copy": false}, {"file": "business-brief", "label": "Writing the brief", "project_copy": false, "machine_copy": false}], "in_prompt": false, "error": null}, {"id": "read-web", "name": "Read the web", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Seemo reads one page at a time through KIQR's own fetcher.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["web"], "icon": "globe", "cost": "free", "provides": [], "publishes_to": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "unblockingapi", "name": "Read blocked pages with UnblockingAPI", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "A real browser and residential proxies for the pages KIQR could not read. One credit per success.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "https://unblockingapi.com", "categories": ["web"], "icon": "shield-check", "cost": "about $0.001 per page", "provides": [], "publishes_to": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [{"env": "UNBLOCKINGAPI_KEY", "label": "UnblockingAPI key", "help": "https://unblockingapi.com", "optional": false, "set": "global"}], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "social-posts", "name": "Write social media posts", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Ready-to-publish posts in the brand's tone of voice, each with its image.", "description": "Write a few ready-to-publish social media posts from the business brief, in the brand's tone of voice.", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing", "publishing"], "icon": "message-square-text", "cost": "model usage", "provides": [], "publishes_to": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "social-posts", "label": "Writing the posts", "project_copy": false, "machine_copy": false}], "in_prompt": true, "error": null}, {"id": "tasks", "name": "Run tasks on a schedule", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Seemo works later or repeatedly; every task is approved by you first.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "list-checks", "cost": "model usage", "provides": [], "publishes_to": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [], "in_prompt": true, "error": null}, {"id": "kiqr-render", "name": "Create images with the built-in renderer", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "On-brand images Seemo designs and KIQR renders here. Free, on from the start.", "description": "How to make an on-brand image for a post, an article cover or a standalone visual with the built-in renderer, or with a generator when the user wants a photo or an illustration.", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["images"], "icon": "image", "cost": "free", "provides": [{"ability": "image", "label": "Make images"}], "publishes_to": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "shipped", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": "~/.kiqr/abilities/make-images", "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": ["image"], "instructions": [{"file": "make-images", "label": "Create images with the built-in renderer", "project_copy": false, "machine_copy": false}], "in_prompt": true, "error": null}, {"id": "kiqr-smtp", "name": "Send email from your mail account", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Email over SMTP. Each project connects its own account. Every send asks you first.", "description": "", "version": "0.1.0", "latest": "0.2.0", "homepage": "", "categories": ["email", "publishing"], "icon": "mail", "cost": "free", "provides": [{"ability": "email", "label": "Send email"}], "publishes_to": ["Email"], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "shipped", "repo": null, "core": false, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": "~/.kiqr/abilities/email-smtp", "git": false, "updatable": true, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [{"env": "SMTP_HOST", "label": "Mail server", "help": "Gmail: smtp.gmail.com", "optional": false, "set": "project"}, {"env": "SMTP_USER", "label": "User name", "help": "Usually your email address", "optional": false, "set": "project"}, {"env": "SMTP_PASSWORD", "label": "Password or app password", "help": "https://myaccount.google.com/apppasswords", "optional": true, "set": "project"}], "active_for": ["email"], "instructions": [], "in_prompt": false, "error": null}, {"id": "product-update-posts", "name": "Write product update posts", "author": "you", "author_url": null, "summary": "When asked for a post about something we shipped.", "description": "When asked for a post about something we shipped.", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "book-open", "cost": "model usage", "provides": [], "publishes_to": [], "sources": [], "tools": 0, "trust": "local", "transport": "instructions", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "yours", "repo": null, "core": false, "builtin": false, "installed": true, "scope": {"scope": "project", "project": "unblockingapi-com"}, "folder": "~/.kiqr/projects/unblockingapi-com/abilities/product-update-posts", "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "product-update-posts", "label": "Write product update posts", "project_copy": true, "machine_copy": false}], "in_prompt": true, "error": null}], "market": [{"id": "fal-image", "name": "Create images with fal.ai", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Photos and illustrations from a text prompt.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "https://fal.ai", "categories": ["images"], "icon": "image", "cost": "about $0.03 per image", "provides": [{"ability": "image", "label": "Make images"}], "publishes_to": [], "sources": [], "tools": 0, "trust": "official", "transport": "http", "runs_program": false, "command": null, "program_found": null, "hosts": ["fal.run", "fal.media"], "source": "shipped", "repo": null, "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": ["FAL_KEY"], "auth": [{"env": "FAL_KEY", "label": "fal.ai API key", "help": "https://fal.ai/dashboard/keys", "optional": false, "set": null}], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "openai-image", "name": "Create images with OpenAI", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "gpt-image-1 behind your OpenAI key.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "https://platform.openai.com", "categories": ["images"], "icon": "image", "cost": "about $0.04 per image", "provides": [{"ability": "image", "label": "Make images"}], "publishes_to": [], "sources": [], "tools": 0, "trust": "official", "transport": "http", "runs_program": false, "command": null, "program_found": null, "hosts": ["api.openai.com"], "source": "shipped", "repo": null, "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": ["OPENAI_API_KEY"], "auth": [{"env": "OPENAI_API_KEY", "label": "OpenAI API key", "help": "https://platform.openai.com/api-keys", "optional": false, "set": null}], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "playwright-browser", "name": "Drive a browser with Playwright", "author": "kiqr-cloud", "author_url": "https://github.com/kiqr-cloud", "summary": "A real browser Seemo can drive: sites without an API, screenshots, logged-in sessions.", "description": "", "version": "0.2.0", "latest": "0.2.0", "homepage": "https://github.com/microsoft/playwright-mcp", "categories": ["browser", "tools", "publishing"], "icon": "", "cost": "free", "provides": [], "publishes_to": [], "sources": [], "tools": 4, "trust": "verified", "transport": "mcp", "runs_program": true, "command": "npx @playwright/mcp@latest --headless", "program_found": null, "hosts": [], "source": "marketplace", "repo": "https://github.com/kiqr-cloud/ability-playwright.git", "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": [], "auth": [], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "fal-video", "name": "Create video with fal.ai", "author": "kiqr-cloud", "author_url": "https://github.com/kiqr-cloud", "summary": "Short clips from a prompt. Slow and paid; Seemo says what it costs first.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "https://fal.ai", "categories": ["video"], "icon": "", "cost": "about $0.50 per clip", "provides": [{"ability": "video", "label": "Make video"}], "publishes_to": [], "sources": [], "tools": 0, "trust": "official", "transport": "http", "runs_program": false, "command": null, "program_found": null, "hosts": ["fal.run"], "source": "marketplace", "repo": "https://github.com/kiqr-cloud/ability-fal-video.git", "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": ["FAL_KEY"], "auth": [{"env": "FAL_KEY", "label": "fal.ai API key", "help": "https://fal.ai/dashboard/keys", "optional": false, "set": null}], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "wordpress", "name": "Publish articles to WordPress", "author": "acme-digital", "author_url": "https://github.com/acme-digital", "summary": "Publish articles to your WordPress site as drafts or posts.", "description": "", "version": "0.3.1", "latest": "0.3.1", "homepage": "https://wordpress.org", "categories": ["publishing"], "icon": "", "cost": "free", "provides": [], "publishes_to": ["WordPress blog"], "sources": [], "tools": 1, "trust": "community", "transport": "http", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "marketplace", "repo": "https://github.com/acme-digital/kiqr-wordpress.git", "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": ["WP_USER", "WP_APP_PASSWORD"], "auth": [{"env": "WP_USER", "label": "User name", "help": "", "optional": false, "set": null}, {"env": "WP_APP_PASSWORD", "label": "Application password", "help": "", "optional": false, "set": null}], "connect": {"label": "Connect with WordPress", "flow": "redirect", "needs": [], "returns": ["WP_APP_PASSWORD", "WP_USER"], "https_only": true}, "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "weekly-report", "name": "Write the Monday report", "author": "acme-digital", "author_url": "https://github.com/acme-digital", "summary": "What was added, what ran, what to write next: a short report every Monday, in plain words.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "book-open", "cost": "model usage", "provides": [], "publishes_to": [], "sources": [], "tools": 0, "trust": "community", "transport": "instructions", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "marketplace", "repo": "https://github.com/acme-digital/kiqr-abilities.git", "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "weekly-report", "label": "Write the Monday report", "project_copy": false, "machine_copy": false}], "in_prompt": false, "error": null}], "contracts": [{"id": "image", "label": "Make images", "tool": "create_image", "configured": null, "active": {"id": "kiqr-render", "name": "Create images with the built-in renderer"}, "candidates": [{"id": "kiqr-render", "name": "Create images with the built-in renderer", "ready": true}]}, {"id": "video", "label": "Make video", "tool": "create_video", "configured": null, "active": null, "candidates": []}, {"id": "music", "label": "Make music", "tool": "create_music", "configured": null, "active": null, "candidates": []}, {"id": "email", "label": "Send email", "tool": "send_email", "configured": null, "active": {"id": "kiqr-smtp", "name": "Send email from your mail account"}, "candidates": [{"id": "kiqr-smtp", "name": "Send email from your mail account", "ready": true}]}], "broken": [], "guide": {"id": "seemo-agent", "name": "Seemo's own guide", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "How Seemo works inside KIQR: the tools, when to act and when to make a task, how to ask for a yes, how to report.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "book-open", "cost": "model usage", "provides": [], "publishes_to": [], "sources": [], "tools": 0, "trust": "official", "transport": "instructions", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": "~/.kiqr/abilities/seemo-agent", "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "seemo-agent", "label": "How Seemo works inside KIQR", "project_copy": false, "machine_copy": false}], "in_prompt": false, "error": null}, "index": "https://kiqr.cloud/marketplace/index.json", "env_file": "~/.kiqr/.env", "project_env_file": "~/.kiqr/projects/unblockingapi-com/.env"};
// The services behind the demo's abilities, as the grid of things to connect shows them.
const DEMO_SERVICES: Record<string, { name: string; icon: string; group: string }> = {
  "kiqr-smtp": { name: "Email", icon: "mail", group: "email" },
  wordpress: { name: "WordPress", icon: "globe", group: "website" },
  "fal-image": { name: "fal.ai", icon: "image", group: "tools" },
  "openai-image": { name: "OpenAI", icon: "image", group: "tools" },
  "fal-video": { name: "fal.ai video", icon: "video", group: "tools" },
  "playwright-browser": { name: "A browser", icon: "globe", group: "sites" },
};
for (const a of [...demoAbilities.abilities, ...demoAbilities.market]) {
  const s = DEMO_SERVICES[a.id];
  if (s) Object.assign(a, { service: a.id, service_card: { id: a.id, name: s.name, icon: s.icon, group: s.group } });
}
{
  // More services in the marketplace, so the grid of things to connect has something in every group.
  const base = demoAbilities.market.find((x) => x.id === "wordpress")!;
  const more: [string, string, string, string, string, string[]][] = [
    ["x-posts", "Post to X", "Posts and threads on X from your account.", "x", "social", ["X"]],
    ["mastodon", "Post to Mastodon", "Toots on your Mastodon account.", "mastodon", "social", ["Mastodon"]],
    ["google-analytics", "Read Google Analytics", "Sessions, users, page views and key events, once a day into Performance.", "google", "numbers", []],
    ["bitwarden", "Passwords from Bitwarden", "Seemo signs in to sites with logins from your vault, never seeing them.", "bitwarden", "passwords", []],
  ];
  for (const [id, name, summary, service, group, channels] of more)
    demoAbilities.market.push({ ...base, id, name, summary, author: "KIQR", author_url: "https://github.com/kiqr", trust: "official", publishes_to: channels, sources: group === "numbers" ? ["traffic"] : [], categories: [group], homepage: "", repo: `https://github.com/kiqr/marketplace`, connect: { label: `Connect with ${name.replace(/^(Post to|Read|Passwords from) /, "")}`, flow: "oauth2", needs: [], returns: [], https_only: false, via_cloud: true }, auth: [{ env: `${service.toUpperCase().replace(/-/g, "_")}_TOKEN`, label: "Sign-in", help: "", optional: false, set: null }], missing: [], service, service_card: { id: service, name: name.replace(/^(Post to|Read|Passwords from) /, ""), icon: group === "numbers" ? "chart" : "globe", group } });
}

{
  // Services that run on KIQR's own browser: KIQR installs the browser first, and they sign in there.
  const base = demoAbilities.market.find((x) => x.id === "wordpress")!;
  const programs = () => [
    { program: "chromium", version: "131.0", found: false, how: "download" },
    { program: "playwright-mcp", version: "0.0.41", found: false, how: "uv" },
    { program: "Xvfb", version: "21.1", found: false, how: "download" },
    // The fallback: a package from the system, which KIQR can install through the desktop's password dialog.
    { program: "xauth", version: "1.1", found: false, how: "system", system: { apt: "xauth", dnf: "xorg-x11-xauth", pacman: "xorg-xauth" }, command: "sudo apt install xauth", pkexec: true },
  ];
  const browser: [string, string, string, string, string, string, string[]][] = [
    ["browser", "A site behind a login", "Seemo reads and works on a site you sign in to by hand, in KIQR's own browser.", "browser", "A site behind a login", "sites", []],
    ["x-browser", "X in a browser", "Posts, replies and reads on X through KIQR's own browser, signed in with your login.", "x", "X", "social", ["x"]],
    ["reddit-browser", "Reddit in a browser", "Posts to subreddits and replies through KIQR's own browser, signed in with your login.", "reddit", "Reddit", "social", ["reddit"]],
  ];
  for (const [id, name, summary, service, card, group, channels] of browser)
    demoAbilities.market.push({
      ...base,
      id,
      name,
      summary,
      author: "KIQR",
      author_url: "https://github.com/kiqr",
      trust: "official",
      channels,
      sources: [],
      categories: [group],
      homepage: "",
      repo: "https://github.com/kiqr/marketplace",
      connect: null,
      runs_program: true,
      program_found: false,
      programs: null,
      platforms: ["linux", "windows"],
      auth: id === "browser" ? [{ env: "SITE_URL", label: "The site's address", help: "The page you sign in on, or the site's front page", optional: false, placeholder: "https://example.com/login", set: null }] : [],
      kinds: [],
      missing: [],
      service,
      service_card: { id: service, name: card, icon: group === "sites" ? "globe" : service === "x" ? "at-sign" : "message-square-text", group },
      _programs: programs(),
    } as DemoAbility);
}
/** The programs a demo browser ability needs, until "installed". */
const demoPrograms = (a: Ability) => (a as Ability & { _programs?: Ability["programs"] })._programs ?? null;

/** A new demo account of an ability, as creating one or a sign-in coming back adds it. */
function demoNewAccount(ab: DemoAbility | undefined, body: Record<string, unknown>): Account {
  const card = ab?.service_card ?? { id: ab?.id ?? "service", name: ab?.name ?? "Account", group: "tools" };
  const roles = ["read", ...(ab?.publishes_to.length || ab?.kinds?.some((k) => k.publishes) ? ["reply", "publish"] : []), ...(ab?.sources.length ? ["measure"] : [])];
  const program = !!ab && !!demoPrograms(ab);
  const c = settle({
    id: `account-${n++}`,
    label: String(body.label || card.name || "Account"),
    service: card.id,
    default: !demoAccounts.some((x) => x.service === card.id),
    created_by: "user",
    created_at: ago(0),
    auth: (ab?.auth ?? []).map((k) => ({ env: k.env, label: k.label, help: k.help, optional: k.optional, options: k.options, placeholder: k.placeholder, set: true })),
    connect: ab?.connect ?? null,
    signed_in: !program,
    service_card: card,
    secrets: IN_KIQR,
    used_by: [...demoAbilities.abilities, ...demoAbilities.market].filter((x) => x.service_card?.id === card.id).map((x) => ({ id: x.id, name: x.name, does: roles })),
    ...(program ? { has_session: true, session: session("signed_out", "signed out"), has_login: false, login_set: [], live_view: true, identity: String((body.secrets as Record<string, string> | undefined)?.SITE_URL ?? "") || undefined } : {}),
  });
  demoAccounts.push(c);
  // A kind whose account is the spot (an X profile): the channel waiting on its site gets it as its publisher, else a new one.
  for (const k of KINDS.filter((k) => k.account_is_target && k.service === c.service)) {
    const waiting = entries.find((e) => !e.publisher && e.ownership === "owned" && (e.kind === k.kind || (!e.kind && !!k.domain && e.domain === k.domain)));
    if (waiting) Object.assign(waiting, { publisher: c.id, kind: k.kind }), emitChannel(waiting);
    else addChannel({ kind: k.kind, publisher: c.id, label: c.label, url: k.kind === "x" ? `https://x.com/${c.label.replace(/^@/, "")}` : undefined }, "account");
  }
  settle(c);
  emit({ type: "account_saved", account: c.id, label: c.label });
  return c;
}
// Seemo suggests Search Console in the welcome flow; it opens from there to sign in.
demoAbilities.market.push({ ...demoAbilities.market[0], id: "search-console", name: "Read Google Search Console", author: "KIQR", summary: "Searches, clicks and positions for your site, read into Performance.", homepage: "https://search.google.com/search-console", categories: ["analytics"], icon: "search", cost: "free", provides: [], sources: ["search"], hosts: ["searchconsole.googleapis.com"], source: "marketplace", missing: [] });
// Google Analytics and Search Console are one service: one sign-in (a shared GOOGLE_TOKEN); each picks once what it reads (a property, a site).
for (const a of demoAbilities.market.filter((x) => x.id === "google-analytics" || x.id === "search-console"))
  Object.assign(a, {
    service: "google",
    service_card: GOOGLE_CARD,
    auth: [{ env: "GOOGLE_TOKEN", label: "Sign-in", help: "", optional: false, set: null }],
    connect: { label: "Sign in with Google", flow: "oauth2", needs: [], returns: ["GOOGLE_TOKEN"], https_only: false, via_cloud: true },
    settings: a.id === "google-analytics" ? GA_SETTINGS : SC_SETTINGS,
    setting: (a.id === "google-analytics" ? { credential: "cred_g01", property: "properties/318204551" } : { credential: "cred_g01", site: "sc-domain:unblockingapi.com" }) as AbilitySetting,
  });
/** The kinds the demo's publishing abilities declare. */
const ABILITY_KINDS: Record<string, KindInfo[]> = {
  "kiqr-smtp": [{ kind: "email", label: "Email", name: "Email", section: "list", target: {}, publishes: true }],
  "x-browser": [{ kind: "x", label: "X profile", name: "X", section: "page", target: {}, publishes: true }],
  "reddit-browser": [{ kind: "reddit", label: "Subreddit", name: "Reddit", section: "community", target: SUBREDDIT, publishes: true }],
  wordpress: [{ kind: "wordpress", label: "Blog", name: "WordPress", section: "page", target: {}, publishes: true }],
};
for (const a of [...demoAbilities.abilities, ...demoAbilities.market]) if (ABILITY_KINDS[a.id]) a.kinds = ABILITY_KINDS[a.id];
// The demo project has accounts of these: they are installed and on.
for (const id of ["google-analytics", "search-console", "x-browser", "reddit-browser", "wordpress"]) {
  const i = demoAbilities.market.findIndex((x) => x.id === id);
  if (i < 0) continue;
  const a = demoAbilities.market[i];
  demoAbilities.market.splice(i, 1);
  demoAbilities.abilities.push({ ...a, installed: true, ready: true, missing: [], updatable: true, scope: { scope: "global" }, folder: `~/.kiqr/abilities/${id}`, programs: demoPrograms(a)?.map((p) => ({ ...p, found: true })) ?? null, program_found: demoPrograms(a) ? true : null });
}
seeds.forEach((s) => demoAccounts.push(settle(s)));
/** What the demo's abilities ask for from the Vault. */
const DEMO_NEEDS: Record<string, { describe: string; template: string; hosts: string[]; provider?: string; for: string; optional?: boolean; connect?: { label: string; needs: string[] } }[]> = {
  "x-browser": [{ describe: "a login for x.com", template: "login", hosts: ["x.com"], for: "Sign in to read, reply and post as you" }],
  "reddit-browser": [{ describe: "a login for reddit.com", template: "login", hosts: ["reddit.com"], for: "Sign in to read, comment and post as you" }],
  "google-analytics": [{ describe: "a sign-in for googleapis.com", template: "oauth", hosts: ["googleapis.com"], provider: "google", for: "Read your traffic" }],
  "search-console": [{ describe: "a sign-in for googleapis.com", template: "oauth", hosts: ["googleapis.com"], provider: "google", for: "Read your searches" }],
  wordpress: [{ describe: "an app password", template: "app_password", hosts: [], for: "Publish posts on your site", connect: { label: "Connect with WordPress", needs: ["wp_site"] } }],
  "kiqr-smtp": [{ describe: "a mail server", template: "smtp", hosts: [], for: "Send email as you" }],
  "fal-image": [{ describe: "an API key for fal.ai", template: "api_key", hosts: ["fal.ai", "fal.run"], for: "Make images" }],
  browser: [{ describe: "a login", template: "login", hosts: [], for: "Sign in to the site", optional: true }],
};
/** An ability's Access here, as the daemon sends it: what it asks for, what is allowed, what else fits. */
function accessOf(a: Ability): AccessNeed[] {
  const one = (c: Credential) => ({ credential: c.id, label: c.label, identity: c.identity, domain: c.domains[0], template: c.template, complete: c.missing.length === 0 });
  return (DEMO_NEEDS[a.id] ?? []).map((n, i) => {
    const fit = demoCredentials.filter((c) => fitsNeed(c, n));
    const given = demoGrants[a.id] ?? [];
    return { id: i ? `need${i + 1}` : "main", describe: n.describe, template: [n.template], hosts: n.hosts, provider: n.provider ?? "", scopes: [], for: n.for, optional: !!n.optional, connect: n.connect ?? null, granted: fit.filter((c) => given.includes(c.id)).map(one), fitting: fit.filter((c) => !given.includes(c.id)).map(one) };
  });
}
/** Ready by the Access rule: every required need has a complete credential allowed. */
function withAccess(a: DemoAbility): DemoAbility {
  const access = accessOf(a);
  if (!access.length) return a;
  const missing = a.installed ? access.filter((n) => !n.optional && !n.granted.some((c) => c.complete)).map((n) => n.describe) : [];
  return { ...a, access, missing, ready: a.installed && !missing.length && a.program_found !== false };
}

const demoInstructions: Record<string, string> = {
  "product-update-posts": "---\nname: Write product update posts\ndescription: When asked for a post about something we shipped.\n---\n\n1. Lead with what changed for the customer, in one sentence.\n2. One concrete example or number.\n3. End with where to try it. No hype words.",
  "social-posts": "---\nname: Write social media posts\ndescription: Write a few ready-to-publish social media posts from the business brief.\n---\n\nYou are the social media lead for a business. You write posts the owner would happily publish today, from what the business brief says about it. Nothing else.\n\n- One idea per post. Lead with it.\n- The brand's words, not marketing words.\n- Each post names an image idea: a headline of a few words and one supporting line.",
};

const task: Task = {
  id: "write-three-social-posts",
  title: "Write three social posts",
  instruction: "Read the instructions for social media posts and images, then the brief. Write three LinkedIn posts: flat pricing, the MCP server, the Visual Editor. Save each as a draft post asset, give two of them an on-brand image, and report what you made.",
  schedule: { kind: "once", at: null, rrule: null, tz: "Europe/Stockholm", text: "right away" },
  schedule_text: "once, right away",
  allowed: ["read", "write_assets", "images"],
  allowed_labels: ["Read the profile, brief, brand, assets and tasks", "Create and edit assets", "Make images"],
  status: "done",
  created_by: "chat:demo-chat-1",
  follow_up_chat: "demo-chat-1",
  created_at: ago(40),
  next_run: null,
  last_run: ago(39),
  last_status: "ok",
  runs: 1,
  running: false,
};
const weekly: Task = {
  id: "monday-linkedin-post",
  title: "Monday LinkedIn post",
  instruction: "Every Monday, write one LinkedIn post about a different offering, in the brand's voice, and save it as a draft.",
  schedule: { kind: "rrule", rrule: "FREQ=WEEKLY;BYDAY=MO;BYHOUR=9;BYMINUTE=0", tz: "Europe/Stockholm", text: "every Monday at 09:00" },
  schedule_text: "every Monday at 09:00",
  allowed: ["read", "write_assets"],
  allowed_labels: ["Read the profile, brief, brand, assets and tasks", "Create and edit assets"],
  status: "active",
  created_by: "chat:demo-chat-1",
  created_at: ago(30),
  next_run: nextMonday(),
  last_run: null,
  last_status: null,
  runs: 0,
  running: false,
};
function nextMonday() {
  const d = new Date();
  d.setDate(d.getDate() + ((8 - d.getDay()) % 7 || 7));
  d.setHours(9, 0, 0, 0);
  return d.toISOString();
}
const run: Run = {
  id: "20261024T091500Z",
  task: task.id,
  started: ago(39),
  finished: ago(37),
  status: "ok",
  trigger: "approval",
  summary: "I saved three LinkedIn posts to the library as drafts, two with an image in the brand's pink. I published nothing.\n\n- Flat pricing (post/flat-one-credit-pricing-explained), with a card\n- MCP server (post/giving-ai-apps-a-real-browser), with a card\n- Visual Editor (post/tap-a-field-get-json)",
  steps: [
    { tool: "read_instructions", label: "Reading how to: Write social media posts", ok: true },
    { tool: "read_instructions", label: "Reading how to: Create images with the built-in renderer", ok: true },
    { tool: "get_section", label: "Reading a profile section: voice", ok: true },
    { tool: "brand_kit", label: "Reading the brand kit", ok: true },
    { tool: "create_asset", label: "Adding to the library", ok: true, asset: { type: "post", id: "flat-one-credit-pricing-explained" } },
    { tool: "create_image", label: "Making an image", ok: true },
    { tool: "create_asset", label: "Adding to the library", ok: true },
    { tool: "create_image", label: "Making an image", ok: true },
    { tool: "create_asset", label: "Adding to the library", ok: true },
  ],
  assets: ["post/flat-one-credit-pricing-explained", "post/giving-ai-apps-a-real-browser", "post/tap-a-field-get-json"],
  session_id: null,
  cost_usd: 0.08,
};

const messages: Message[] = [
  { id: "m1", at: ago(42), role: "user", text: "Write three social media posts for the library as draft post assets, with an on-brand image on most of them, and tell me when they are in." },
  {
    id: "m2",
    at: ago(41),
    role: "card",
    text: "",
    card: {
      kind: "approval",
      approval: "a1",
      approval_kind: "task",
      status: "approved",
      task: task.id,
      payload: { title: task.title, instruction: task.instruction, schedule_text: "once, right away", allowed: task.allowed, allowed_labels: task.allowed_labels, follow_up: true, next: [ago(40)], timezone: "Europe/Stockholm" },
      result: { task: task.id, running: true },
    },
  },
  { id: "m3", at: ago(40), role: "seemo", text: "The task is running. It will write three LinkedIn drafts on flat credit pricing, the MCP server, and the Visual Editor. I'll tell you when they're in the library.", steps: [{ tool: "create_task", label: "Proposing a task", ok: true }] },
  { id: "m4", at: ago(37), role: "note", kind: "followup", text: `Task "Write three social posts" finished: ok. Assets: ${run.assets.join(", ")}.` },
  { id: "m5", at: ago(36), role: "seemo", text: "The three LinkedIn drafts are in the library: flat one-credit pricing, the MCP server giving AI apps a real browser, and the Visual Editor. Nothing was published. Two things to check when you review them: the length of the pricing post, and whether you want the Scout plan named." },
  { id: "m6", at: ago(20), role: "user", text: "Give the pricing post an image." },
  {
    id: "m7",
    at: ago(19),
    role: "seemo",
    text: "Done: a 1600×900 card on the pricing post, dark with the brand's pink accent, reading \"One request. One credit.\" It is on the asset as cover.png. I would try a lighter variant next if the feed it goes into is mostly dark.",
    steps: [
      { tool: "brand_kit", label: "Opening the brand kit", ok: true },
      { tool: "read_instructions", label: "Reading how to: Create images with the built-in renderer", ok: true },
      { tool: "create_image", label: "Making an image", ok: true, image: DEMO_IMAGE, asset: { type: "post", id: "flat-one-credit-pricing-explained" } },
    ],
  },
];

const demoPlan: Plan = {
  version: 1,
  period: "Q4 2026",
  summary: "UnblockingAPI sells to developers who hit blocked pages. This quarter is about being found for the problem (scraping blocked sites, Cloudflare, rendering) and turning the template library into sign-ups. Not this quarter: paid ads.",
  objectives: [
    { id: "rank-for-the-blocked-page-queries", title: "Rank for the blocked-page queries", why: "The content gaps name five informational queries the site does not answer yet; the templates cluster already ranks.", kpi: "organic_traffic", target: 4000, unit: "sessions/month", by: "2026-12-31", status: "active" },
    { id: "turn-templates-into-sign-ups", title: "Turn templates into sign-ups", why: "The template library is the differentiator the brief leads with and the free tier starts at 500 credits.", kpi: "signups", target: 120, unit: "per month", by: "2026-12-31", status: "active" },
    { id: "be-present-where-scrapers-ask", title: "Be present where scrapers ask", why: "r/webscraping and Hacker News threads ask the questions the product answers; replies are the cheapest way in we have.", kpi: "brand_mentions", target: null, unit: "", by: null, status: "active" },
  ],
  channels: ["email", "reddit", "linkedin", "blog"],
  updated_by: "chat:demo-chat-1",
  updated_at: ago(600),
};
const demoCampaigns: Campaign[] = [
  { id: "launch-visual-editor", title: "Launch: Visual Editor", goal: "Fifty sign-ups from the launch week and one thread on Hacker News that lives.", objective: "turn-templates-into-sign-ups", objective_title: "Turn templates into sign-ups", channels: ["linkedin", "reddit", "email"], start: "2026-11-03", end: "2026-11-14", status: "active", created_by: "chat:demo-chat-1", created_at: ago(500), updated_at: ago(40), assets: [{ type: "post", id: "tap-a-field-get-json", title: "Tap a field, get JSON", status: "draft", updated_at: ago(36) }], tasks: [{ id: "monday-linkedin-post", title: "Monday LinkedIn post", status: "active", next_run: nextMonday(), runs: 0 }] },
];
/** Replies on Reddit: one in a channel where Seemo may reply on its own (the card is the record), and one in a community that is no channel yet, which asks first. */
const REPLY_TEXT = "A 403 on every page with requests usually means the site checks the TLS fingerprint and headers, not just the IP. Try a real browser (Playwright) first; if that passes, it's fingerprinting. (I work on UnblockingAPI, which does this for you, but Playwright alone may be enough here.)";
const replies: Message[] = [
  { id: "r1", at: ago(30), role: "user", text: "Find threads where people are stuck on blocked pages and reply where it helps." },
  { id: "r2", at: ago(28), role: "seemo", text: "I found two. The r/webscraping channel lets me reply on my own, so I posted it. The other is in r/learnpython, which is not one of your channels yet, so I'm asking first.", steps: [{ tool: "read_url", label: "Reading r/webscraping", ok: true }, { tool: "read_url", label: "Reading r/learnpython", ok: true }, { tool: "publish_asset", label: "Publishing a reply", ok: true, asset: { type: "reply", id: "r-webscraping-cloudflare" } }] },
  {
    id: "r3",
    at: ago(27),
    role: "card",
    text: "",
    card: {
      kind: "approval",
      approval: "a-reply-webscraping",
      approval_kind: "publish",
      status: "approved",
      payload: {
        asset: { type: "reply", id: "r-webscraping-cloudflare", title: "r/webscraping: 'Cloudflare blocks my scraper after 50 requests'", status: "ready" },
        target: { channel: "r-webscraping", label: "r/webscraping", kind: "reddit", ownership: "third_party", fields: { subreddit: "webscraping" }, url: "https://www.reddit.com/r/webscraping/", account: "reddit-team", account_label: "u/unblockingapi_team", new: false },
        excerpt: "If it's Cloudflare's managed challenge, rotating datacenter proxies won't help; it scores the browser, not just the IP.",
        asks_first: false,
        auto: true,
      },
      result: { url: "https://www.reddit.com/r/webscraping/" },
    },
  },
  {
    id: "r4",
    at: ago(26),
    role: "card",
    text: "",
    card: {
      kind: "approval",
      approval: "a-reply-learnpython",
      approval_kind: "action",
      status: "pending",
      payload: {
        ability: "reddit-browser",
        ability_name: "Reddit in a browser",
        tool: "reply",
        label: "Reply to a post",
        description: "Replies to a post or a comment as the account.",
        role: "reply",
        target: "r/learnpython: 'requests gets 403 on every page, what am I missing?'",
        connection: "reddit-team",
        account: "u/unblockingapi_team",
        args: { subreddit: "learnpython", post: "https://www.reddit.com/r/learnpython/comments/1h2x3y4/", text: REPLY_TEXT },
        fields: [{ key: "post", value: "https://www.reddit.com/r/learnpython/comments/1h2x3y4/" }, { key: "text", value: REPLY_TEXT }],
        spot: { kind: "reddit", kind_label: "Subreddit", fields: { subreddit: "learnpython" }, label: "r/learnpython", channel: null, new: true },
        asks_first: true,
      },
    },
  },
];
const chats: Chat[] = [
  { id: "demo-chat-2", title: "Reply where people are stuck on blocked pages", created_at: ago(30), updated_at: ago(26), session_id: "demo", model: "claude-cli/sonnet", last_message: "The other is in r/learnpython, which is not in your Audience yet, so I'm asking first.", messages: replies.length },
  { id: "demo-chat-1", title: "Write three social media posts for the library", created_at: ago(42), updated_at: ago(36), session_id: "demo", model: "claude-cli/sonnet", last_message: "The three LinkedIn drafts are in the library.", messages: messages.length },
  { id: "demo-chat-0", title: "What should we focus on this quarter?", created_at: ago(600), updated_at: ago(590), session_id: "demo", model: "claude-cli/sonnet", last_message: "Lead with the MCP server: it is the one thing competitors don't have.", messages: 2 },
];
const transcripts: Record<string, Message[]> = {
  "demo-chat-1": messages,
  "demo-chat-2": replies,
  "demo-chat-0": [
    { id: "q1", at: ago(600), role: "user", text: "What should we focus on this quarter?" },
    {
      id: "q2",
      at: ago(590),
      role: "seemo",
      text: "Three options, from the brief:\n\n1. **Developers who hit blocks.** The docs and the flat one-credit pricing speak to them already. Cheapest to win.\n2. **AI builders.** The MCP server is the one thing competitors don't have. Smallest audience today, fastest growing.\n3. **Data teams.** Templates and the Visual Editor, but the site barely mentions the JSON output.\n\nI'd lead with the MCP server: write for AI builders first and keep the developer content steady underneath.",
      steps: [{ tool: "get_section", label: "Reading a profile section: positioning", ok: true }],
    },
  ],
};

const listeners = new Set<(e: KiqrEvent) => void>();
export function onDemoSeemo(cb: (e: KiqrEvent) => void) {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}
const emit = (e: Record<string, unknown>) => {
  const ev = { at: new Date().toISOString(), project: slug, ...e } as KiqrEvent;
  for (const l of listeners) l(ev);
};
setDemoTargets({ resolve: resolveTarget, emit });

/** The user's answer to a card: the card changes in its chat, and a new spot Seemo acted in becomes a channel. */
function resolveCard(id: string, decision: string) {
  const status = decision === "approve" ? "approved" : "declined";
  for (const [chat, list] of Object.entries(transcripts)) {
    const m = list.find((x) => x.card?.approval === id);
    if (!m?.card) continue;
    const p = m.card.payload as { spot?: { kind: string; fields: Record<string, unknown>; new?: boolean }; connection?: string; args?: { post?: string }; target?: { kind: string; fields: Record<string, unknown>; account: string; new?: boolean } };
    let result: Record<string, unknown> = {};
    if (status === "approved") {
      const spot = p.spot?.new ? { ...p.spot, account: p.connection } : p.target?.new ? p.target : null;
      if (spot) {
        try {
          const e = addChannel({ kind: spot.kind, publisher: spot.account, fields: spot.fields }, "seemo");
          result = { channel: e.id, url: p.args?.post ?? null };
        } catch {
          // Already there: nothing to add.
        }
      }
    }
    m.card = { ...m.card, status, result };
    emit({ type: "approval_resolved", chat, approval: id, decision });
    emit({ type: "chat_message", chat, message: m });
    return { id, status, result };
  }
  return { id, status, result: {} };
}

let n = 10;
/** A scripted reply: a tool step, streamed words, then the saved message. */
function reply(chatId: string, text: string) {
  const user: Message = { id: `d${n++}`, at: new Date().toISOString(), role: "user", text };
  transcripts[chatId] = [...(transcripts[chatId] ?? []), user];
  const c = chats.find((x) => x.id === chatId);
  if (c && c.title === "New chat") {
    c.title = text.slice(0, 60);
    emit({ type: "chat_renamed", chat: chatId, title: c.title });
  }
  emit({ type: "chat_message", chat: chatId, message: user });
  const id = `d${n++}`;
  const answer =
    "I read the brief again. UnblockingAPI sells a web unblocking API: real browsers and rotating residential proxies behind one call, one credit per successful request. If you want, I can turn that into a task: three posts, or a weekly one on Mondays.";
  const words = answer.split(" ");
  let t = 300;
  window.setTimeout(() => emit({ type: "chat_turn_started", chat: chatId, message: id }), t);
  t += 300;
  window.setTimeout(() => emit({ type: "chat_tool", chat: chatId, tool: "get_brief", label: "Reading the business brief", status: "started", note: "" }), t);
  t += 900;
  window.setTimeout(() => emit({ type: "chat_tool", chat: chatId, tool: "get_brief", label: "Reading the business brief", status: "done", note: "" }), t);
  words.forEach((w, i) => {
    t += 40 + (w.endsWith(".") ? 120 : 0);
    window.setTimeout(() => emit({ type: "chat_delta", chat: chatId, text: (i ? " " : "") + w }), t);
  });
  t += 200;
  window.setTimeout(() => {
    const m: Message = { id, at: new Date().toISOString(), role: "seemo", text: answer, steps: [{ tool: "get_brief", label: "Reading the business brief", ok: true }] };
    transcripts[chatId] = [...(transcripts[chatId] ?? []), m];
    if (c) {
      c.last_message = answer.slice(0, 80);
      c.updated_at = m.at;
    }
    emit({ type: "chat_message", chat: chatId, message: m });
    emit({ type: "chat_turn_finished", chat: chatId });
  }, t);
}

// The Vault in the demo: credentials (no secret ever leaves, as with the daemon), the providers kiqr.cloud offers, the grants.
const credField = (name: string, label: string, kind: Credential["fields"][number]["kind"], set: boolean, value?: string, optional = false) => ({ name, label, kind, set, value, optional });
const demoNow = new Date().toISOString();
let demoCredentials: Credential[] = [
  { id: "cred_x01", label: "Our X", template: "login", domains: ["x.com", "twitter.com"], identity: "unblockingapi", provider: null, scopes: [], login_url: null, notes: "", missing: [], fields: [credField("username", "Username or email", "text", true, "unblockingapi"), credField("password", "Password", "secret", true, undefined, true), credField("totp", "Two-factor key", "totp", true, undefined, true)], created_at: demoNow, updated_at: demoNow },
  { id: "cred_g01", label: "Google", template: "oauth", domains: ["google.com", "googleapis.com"], identity: null, provider: "google", scopes: ["https://www.googleapis.com/auth/analytics.readonly"], login_url: null, notes: "", missing: [], fields: [credField("account", "Account", "text", false, undefined, true), credField("token", "Access token", "secret", true), credField("refresh", "Refresh token", "secret", true, undefined, true), credField("expires_at", "Expires", "number", true, "0", true), credField("scopes", "Access", "text", true, "https://www.googleapis.com/auth/analytics.readonly", true)], created_at: demoNow, updated_at: demoNow },
  { id: "cred_r01", label: "u/acme_dev", template: "login", domains: ["reddit.com"], identity: "acme_dev", provider: null, scopes: [], login_url: null, notes: "", missing: [], fields: [credField("username", "Username or email", "text", true, "acme_dev"), credField("password", "Password", "secret", false, undefined, true), credField("totp", "Two-factor key", "totp", false, undefined, true)], created_at: demoNow, updated_at: demoNow },
  { id: "cred_w01", label: "unblockingapi.com blog", template: "app_password", domains: ["unblockingapi.com"], identity: "editor", provider: null, scopes: [], login_url: null, notes: "", missing: [], fields: [credField("username", "Username", "text", true, "editor"), credField("password", "App password", "secret", true)], created_at: ago(9000), updated_at: ago(9000) },
  { id: "cred_m01", label: "hello@unblockingapi.com", template: "smtp", domains: ["unblockingapi.com"], identity: "hello@unblockingapi.com", provider: null, scopes: [], login_url: null, notes: "", missing: ["host"], fields: [credField("host", "Server", "text", false), credField("port", "Port", "number", true, "587", true), credField("security", "Security", "text", true, "starttls", true), credField("username", "Username", "text", true, "hello@unblockingapi.com", true), credField("password", "Password", "secret", true, undefined, true), credField("from", "Send as", "text", true, "hello@unblockingapi.com")], created_at: demoNow, updated_at: demoNow },
];
const demoTemplates: CredentialTemplate[] = [
  { id: "login", label: "Login", fields: [{ name: "username", label: "Username or email", kind: "text", optional: false }, { name: "password", label: "Password", kind: "secret", optional: true }, { name: "totp", label: "Two-factor key", kind: "totp", optional: true }] },
  { id: "api_key", label: "API key", fields: [{ name: "key", label: "API key", kind: "secret", optional: false }] },
  { id: "oauth", label: "Signed-in app", fields: [] },
  { id: "smtp", label: "Mail server", fields: [{ name: "host", label: "Server", kind: "text", optional: false }, { name: "port", label: "Port", kind: "number", optional: true }, { name: "security", label: "Security", kind: "text", optional: true }, { name: "username", label: "Username", kind: "text", optional: true }, { name: "password", label: "Password", kind: "secret", optional: true }, { name: "from", label: "Send as", kind: "text", optional: false }] },
  { id: "custom", label: "Something else", fields: [] },
];
const demoProviders = [
  { id: "google", name: "Google", domains: ["google.com", "googleapis.com"], scopes: [{ scope: "https://www.googleapis.com/auth/analytics.readonly", label: "Read Google Analytics" }, { scope: "https://www.googleapis.com/auth/webmasters.readonly", label: "Read Search Console" }] },
  { id: "github", name: "GitHub", domains: ["github.com", "api.github.com"], scopes: [{ scope: "public_repo", label: "Public repositories" }, { scope: "read:user", label: "Your profile" }] },
  { id: "rankzon", name: "Rankzon", domains: ["rankzon.com", "mcp.rankzon.com"], scopes: [{ scope: "rankzon:read", label: "Read rankings" }] },
];
let demoGrants: Record<string, string[]> = { "x-browser": ["cred_x01"], "google-analytics": ["cred_g01"], "search-console": ["cred_g01"], wordpress: ["cred_w01"], "kiqr-smtp": ["cred_m01"], "reddit-browser": ["cred_r01"] };

function demoVault<T>(path: string, method: string, body: Record<string, unknown>): Promise<T> | null {
  if (path === "/api/connect/providers") return Promise.resolve({ providers: demoProviders, cloud_signed_in: true } as T);
  if (path.startsWith("/api/vault/connect/")) return Promise.reject(new Error("The demo cannot sign in anywhere."));
  if (path === "/api/vault" && method === "GET") return Promise.resolve({ status: { unlocked: true, key: "local_file", key_text: "a file on this machine, next to the vault", problem: null, count: demoCredentials.length }, credentials: demoCredentials, templates: demoTemplates } as T);
  if (path === "/api/vault" && method === "POST") {
    const t = demoTemplates.find((x) => x.id === body.template) ?? demoTemplates[4];
    const values = (body.values ?? {}) as Record<string, string>;
    const fields = [...t.fields, ...((body.fields ?? []) as { name: string; label: string; kind: Credential["fields"][number]["kind"] }[])].map((f) => credField(f.name, f.label, f.kind, !!values[f.name], f.kind === "text" || f.kind === "number" ? values[f.name] : undefined, "optional" in f ? !!f.optional : false));
    const c: Credential = { id: `cred_${Math.random().toString(16).slice(2, 8)}`, label: String(body.label ?? values.username ?? (body.domains as string[])[0]), template: t.id, domains: (body.domains as string[]) ?? [], identity: values.username ?? values.from ?? null, provider: null, scopes: [], login_url: null, notes: "", missing: fields.filter((f) => !f.optional && !f.set).map((f) => f.name), fields, created_at: new Date().toISOString(), updated_at: new Date().toISOString() };
    demoCredentials = [...demoCredentials, c];
    return Promise.resolve(c as T);
  }
  const one = /^\/api\/vault\/([^/?]+)$/.exec(path);
  if (one) {
    const id = decodeURIComponent(one[1]);
    const c = demoCredentials.find((x) => x.id === id);
    if (!c) return Promise.reject(new Error(`no credential ${id}`));
    if (method === "DELETE") {
      demoCredentials = demoCredentials.filter((x) => x.id !== id);
      demoGrants = Object.fromEntries(Object.entries(demoGrants).map(([a, cs]) => [a, cs.filter((x) => x !== id)]).filter(([, cs]) => cs.length));
      return Promise.resolve({ ok: true } as T);
    }
    if (method === "PUT") {
      const values = (body.values ?? {}) as Record<string, string>;
      const fields = c.fields.map((f) => (f.name in values ? { ...f, set: !!values[f.name], value: f.kind === "text" || f.kind === "number" ? values[f.name] || undefined : undefined } : f));
      const next: Credential = { ...c, label: (body.label as string) ?? c.label, domains: (body.domains as string[]) ?? c.domains, fields, missing: fields.filter((f) => !f.optional && !f.set).map((f) => f.name) };
      demoCredentials = demoCredentials.map((x) => (x.id === id ? next : x));
      return Promise.resolve(next as T);
    }
    return Promise.resolve(c as T);
  }
  if (/\/access(\/grant)?$/.test(path)) {
    const ability = String(body.ability ?? "");
    const cred = String(body.credential ?? "");
    if (method === "POST") demoGrants = { ...demoGrants, [ability]: [...(demoGrants[ability] ?? []).filter((x) => x !== cred), cred] };
    if (method === "DELETE") demoGrants = Object.fromEntries(Object.entries({ ...demoGrants, [ability]: (demoGrants[ability] ?? []).filter((x) => x !== cred) }).filter(([, cs]) => cs.length));
    return Promise.resolve({ grants: demoGrants } as T);
  }
  return null;
}

export function demoSeemoRequest<T>(path: string, init?: RequestInit): Promise<T> {
  const method = init?.method ?? "GET";
  const body = init?.body ? JSON.parse(String(init.body)) : {};
  const vault = demoVault<T>(path, method, body);
  if (vault) return vault;
  if (path === "/api/tools")
    return Promise.resolve({
      groups: [
        { id: "read", label: "Read the profile, brief, brand, assets and tasks", tools: [] },
        { id: "web", label: "Read pages on the web", tools: [] },
        { id: "images", label: "Make images", tools: [] },
        { id: "media", label: "Make video and music", tools: [] },
        { id: "write_profile", label: "Edit the business profile and brief", tools: [] },
        { id: "write_assets", label: "Create and edit assets", tools: [] },
        { id: "jobs", label: "Start crawls and profile builds", tools: [] },
      ],
    } as T);
  const insM = /\/abilities\/([^/]+)\/instructions/.exec(path);
  if (insM) {
    const id = decodeURIComponent(insM[1]);
    const all = [...demoAbilities.abilities, ...demoAbilities.market, demoAbilities.guide!];
    const forProject = /\/projects\//.test(path);
    if (method === "PUT") {
      const text = String(body.text ?? "");
      const d = /description:\s*(.*)/.exec(text)?.[1]?.replace(/^"|"$/g, "");
      const nm = /name:\s*(.*)/.exec(text)?.[1]?.replace(/^"|"$/g, "");
      let a = all.find((x) => x.id === id);
      if (!a) {
        a = { ...demoAbilities.abilities[8], id, name: nm ?? id, description: d ?? "", folder: `~/.kiqr/projects/${slug}/abilities/${id}`, instructions: [{ file: id, label: nm ?? id, project_copy: forProject, machine_copy: !forProject }], scope: forProject ? { scope: "project", project: slug } : { scope: "global" } };
        demoAbilities.abilities.unshift(a);
      } else {
        if (d) a.description = d;
        if (a.source !== "yours") a.instructions = a.instructions.map((f) => ({ ...f, project_copy: forProject ? true : f.project_copy, machine_copy: forProject ? f.machine_copy : true }));
      }
      demoInstructions[id] = text;
      return Promise.resolve(a as T);
    }
    if (method === "DELETE") {
      const a = all.find((x) => x.id === id);
      if (a?.source === "yours") demoAbilities.abilities.splice(demoAbilities.abilities.indexOf(a), 1);
      else if (a) a.instructions = a.instructions.map((f) => (forProject ? { ...f, project_copy: false } : { ...f, machine_copy: false }));
      delete demoInstructions[id];
      return Promise.resolve(undefined as T);
    }
    const a = all.find((x) => x.id === id);
    return Promise.resolve((demoInstructions[id] ?? `---\nname: ${a?.name ?? id}\ndescription: ${a?.description ?? ""}\n---\n\nThe shipped text is not included in the demo.`) as T);
  }
  if (/\/abilities\/[^/]+\/(pause|resume)$/.test(path)) {
    const id = path.split("/").slice(-2)[0];
    const a = demoAbilities.abilities.find((x) => x.id === id);
    const off = path.endsWith("/pause");
    if (a) {
      if (/\/projects\//.test(path)) a.paused = off;
      else a.machine_off = off;
    }
    return Promise.resolve({ paused: off } as T);
  }
  if (/\/abilities\/[^/]+\/(install|update)(\?|$)/.test(path) || (/^\/api\/abilities\/[^/?]+(\?|$)/.test(path) && method === "DELETE")) {
    const id = path.split("/")[3].split("?")[0];
    if (/\/update(\?|$)/.test(path)) {
      const a = demoAbilities.abilities.find((x) => x.id === id);
      if (a) a.version = a.latest;
    }
    if (path.endsWith("/install")) {
      const i = demoAbilities.market.findIndex((x) => x.id === id);
      if (i >= 0) {
        const given = Object.keys(body.secrets ?? {}).length > 0;
        const a: Ability = { ...demoAbilities.market[i], programs: demoPrograms(demoAbilities.market[i]), installed: true, updatable: true, ready: demoAbilities.market[i].missing.length === 0 || given, missing: given ? [] : demoAbilities.market[i].missing, scope: { scope: "global" }, folder: `~/.kiqr/abilities/${id}` };
        demoAbilities.market.splice(i, 1);
        demoAbilities.abilities.push(a);
      }
    }
    if (method === "DELETE") {
      const i = demoAbilities.abilities.findIndex((x) => x.id === id);
      if (i >= 0) {
        demoAbilities.market.push({ ...demoAbilities.abilities[i], installed: false, scope: null, folder: null, ready: false });
        demoAbilities.abilities.splice(i, 1);
      }
    }
    return Promise.resolve({ id, folder: id, detail: "pulled" } as T);
  }
  if (path === "/api/abilities/install" && method === "POST") return Promise.resolve(demoAbilities.abilities[8] as T);
  // What the project picks once for an ability (a GA4 property), through an account of its service.
  const settingsM = /\/abilities\/([^/]+)\/settings$/.exec(path);
  if (settingsM && method === "PUT") {
    const a = demoAbilities.abilities.find((x) => x.id === decodeURIComponent(settingsM[1]));
    if (!a) return Promise.reject(new Error(`the ability '${settingsM[1]}' is not installed here`));
    a.setting = { credential: String(body.credential ?? ""), ...(body.values ?? {}) } as AbilitySetting;
    emit({ type: "ability_settings_saved", ability: a.id, credential: a.setting.credential });
    return Promise.resolve({ ability: a.id, setting: a.setting } as T);
  }
  // A fresh object each time, as from the daemon, so a reload after an install is seen; each ability with its accounts here.
  if (/\/abilities(\?|$)/.test(path)) return Promise.resolve({ ...demoAbilities, abilities: demoAbilities.abilities.map(withAccess), market: demoAbilities.market.map(withAccess) } as T);
  // A setting's choices, read through the ability's credential.
  const choiceM = /\/abilities\/[^/]+\/choices\/([^/?]+)/.exec(path);
  if (choiceM) return Promise.resolve({ name: choiceM[1], choices: demoChoices[choiceM[1]] ?? [] } as T);
  // Credentials at work: a login's browser sign-in, what it did, recommendations by host.
  const credSessM = /\/credentials\/([^/]+)\/(session|signin|signout)$/.exec(path);
  if (credSessM) {
    const c = demoCredentials.find((x) => x.id === decodeURIComponent(credSessM[1]));
    const login = c?.template === "login";
    const st: CredentialSession = { has_session: login, session: { state: c?.id === "cred_r01" ? "signed_out" : "signed_in", message: c?.id === "cred_r01" ? "signed out" : "", checked_at: ago(30), signed_in_at: ago(600) }, has_login: login, live_view: login };
    if (credSessM[2] === "signin") return Promise.resolve({ started: true } as T);
    if (credSessM[2] === "signout") return Promise.resolve({ session: { ...st.session, state: "signed_out" } } as T);
    return Promise.resolve(st as T);
  }
  if (/\/recommendations/.test(path)) {
    const want = /credential=([^&]+)/.exec(path)?.[1];
    const all = [...demoAbilities.abilities.map((a) => ({ a, installed: true })), ...demoAbilities.market.map((a) => ({ a, installed: false }))];
    const recs: Recommendation[] = demoCredentials
      .filter((c) => !want || c.id === decodeURIComponent(want))
      .map((c) => ({
        credential: c.id,
        label: c.label,
        domain: c.domains[0],
        provider: c.provider,
        abilities: all.flatMap(({ a, installed }) => {
          const n = (DEMO_NEEDS[a.id] ?? []).find((n) => fitsNeed(c, n) && (n.hosts.length > 0 || n.template === "smtp"));
          if (!n) return [];
          const status = (demoGrants[a.id] ?? []).includes(c.id) ? "allowed" : installed ? "installed" : "available";
          return [{ id: a.id, name: a.name, icon: a.icon, summary: a.summary, status, need: n.describe, for: n.for, more_access: false } as Recommendation["abilities"][number]];
        }),
      }));
    return Promise.resolve({ recommendations: recs } as T);
  }
  // Demo fixtures: the networks a demo project's abilities would declare.
  if (path.endsWith("/networks")) return Promise.resolve({ networks: [
    { kind: "linkedin", name: "LinkedIn", icon: "", network: true, installed: true, text_limit: 3000, image: { width: 1200, height: 627 }, preview: "feed" },
    { kind: "x", name: "X", icon: "", network: true, installed: true, text_limit: 280, image: { width: 1600, height: 900 }, preview: "feed" },
    { kind: "instagram", name: "Instagram", icon: "", network: true, installed: true, text_limit: 2200, image: { width: 1080, height: 1080 }, preview: "feed" },
    { kind: "reddit", name: "Reddit", icon: "", network: true, installed: true, text_limit: 40000, image: null, preview: "feed" },
    { kind: "wordpress", name: "WordPress", icon: "globe", network: false, installed: true, preview: "article" },
    { kind: "email", name: "Email", icon: "mail", network: false, installed: true },
  ] } as T);
  if (/\/suggestions\//.test(path)) return Promise.resolve({ dismissed: path.split("/").slice(-2, -1)[0] } as T);
  if (/\/connect\//.test(path)) {
    // Signing an account in again keeps it; a new sign-in makes the account when it comes back (here: at once).
    const ab = [...demoAbilities.abilities, ...demoAbilities.market].find((x) => path.endsWith(`/connect/${x.id}`));
    if (body.account) return Promise.resolve({ url: "about:blank", callback: true, account: body.account, fields: [] } as T);
    const c = demoNewAccount(ab, body);
    window.setTimeout(() => emit({ type: "connect_finished", ability: ab?.id ?? "", ok: true, error: null, account: c.id }), 600);
    return Promise.resolve({ url: "about:blank", callback: true, account: c.id, fields: [] } as T);
  }
  // Signing in on KIQR's browser: the demo has no browser, so a sign-in always needs a hand, and the live view waits for its first picture.
  const sessM = /\/accounts\/([^/]+)\/(signin|signout|session|hands)$/.exec(path);
  if (sessM) {
    const c = demoAccounts.find((x) => x.id === sessM[1]);
    if (!c) return Promise.reject(new Error("no such account"));
    const now = new Date().toISOString();
    const changed = () => {
      settle(c);
      emit({ type: "account_state_changed", account: c.id, state: c.session?.state ?? "unknown", message: c.session?.message ?? "" });
    };
    if (sessM[2] === "signin") {
      c.session = { state: "checking", message: "checking the sign-in", checked_at: now, signed_in_at: c.session?.signed_in_at ?? null };
      changed();
      window.setTimeout(() => {
        c.session = { state: "needs_hand", message: c.service === "browser" ? "sign in once in the live view" : "verify you are human", checked_at: new Date().toISOString(), signed_in_at: c.session?.signed_in_at ?? null };
        changed();
      }, 1200);
      return Promise.resolve({ started: true, session: c.session } as T);
    }
    if (sessM[2] === "signout") {
      c.session = { state: "signed_out", message: "signed out", checked_at: now, signed_in_at: null };
      changed();
      return Promise.resolve({ session: c.session } as T);
    }
    if (sessM[2] === "hands") {
      window.setTimeout(() => emit({ type: "hands_started", account: c.id, hands: `demo-${c.id}` }), 100);
      return Promise.resolve({ hands: `demo-${c.id}`, token: "demo", can: { click: true, key: true, type: true, navigate: true } } as T);
    }
    return Promise.resolve({ has_session: !!c.has_session, session: c.session, has_login: !!c.has_login, live_view: !!c.live_view } as T);
  }
  const handsM = /\/hands\/([^/]+)\/(input|end)$/.exec(path);
  if (handsM) {
    if (handsM[2] === "input") return Promise.resolve({ ok: true } as T);
    // The person says they signed in: the account works.
    const c = demoAccounts.find((x) => `demo-${x.id}` === decodeURIComponent(handsM[1]));
    if (c) {
      if (body.signed_in) c.session = { state: "signed_in", message: "signed in by hand", checked_at: new Date().toISOString(), signed_in_at: new Date().toISOString(), by_user: true };
      settle(c);
      emit({ type: "hands_ended", account: c.id, hands: handsM[1], message: body.signed_in ? "signed in" : "" });
      emit({ type: "account_state_changed", account: c.id, state: c.session?.state ?? "unknown", message: c.session?.message ?? "" });
    }
    return Promise.resolve({ ended: true } as T);
  }
  const progM = /^\/api\/abilities\/([^/]+)\/programs$/.exec(path);
  if (progM) {
    const a = demoAbilities.abilities.find((x) => x.id === progM[1]);
    // KIQR's own downloads arrive; a system package waits for the user.
    if (a?.programs) a.programs = a.programs.map((p) => ({ ...p, found: p.found || p.how !== "system" }));
    if (a) a.program_found = !!a.programs?.every((p) => p.found);
    return Promise.resolve({ id: progM[1], programs: a?.programs ?? [] } as T);
  }
  const sysM = /^\/api\/abilities\/([^/]+)\/programs\/system$/.exec(path);
  if (sysM) {
    const a = demoAbilities.abilities.find((x) => x.id === sysM[1]);
    const pkgs = (a?.programs ?? []).filter((p) => !p.found && p.how === "system").map((p) => p.system?.apt ?? p.program);
    if (a?.programs) a.programs = a.programs.map((p) => (p.how === "system" ? { ...p, found: true } : p));
    if (a) a.program_found = !!a.programs?.every((p) => p.found);
    return new Promise((ok) => setTimeout(() => ok({ installed: pkgs, programs: a?.programs ?? [] } as T), 1200));
  }
  const secretM = /\/accounts\/([^/]+)\/secrets\/([A-Z0-9_]+)$/.exec(path);
  if (secretM) {
    const c = demoAccounts.find((x) => x.id === secretM[1]);
    const name = secretM[2];
    if (c && name.startsWith("LOGIN_")) {
      c.login_set = [...(c.login_set ?? []).filter((k) => k !== name), ...(body.value ? [name] : [])];
      c.has_login = c.login_set.includes("LOGIN_USERNAME") && c.login_set.includes("LOGIN_PASSWORD");
    } else if (c) c.auth = c.auth.map((k) => (k.env === name ? { ...k, set: !!body.value } : k));
    if (c) {
      settle(c);
      emit({ type: "secret_set", account: c.id, name });
    }
    return Promise.resolve({ name, set: !!body.value } as T);
  }
  if (/\/secrets\//.test(path)) return Promise.resolve({ name: path.split("/").pop(), set: !!body.value } as T);
  // Channels: the list, what can be added, one channel, a suggestion's Add and Dismiss; a list's people.
  const chanM = /\/channels(?:\/([^/?]+))?(?:\/(contacts|import|unsubscribe|sends|learn))?(?:\?|$)/.exec(path);
  if (chanM) {
    const id = chanM[1] ? decodeURIComponent(chanM[1]) : null;
    const sub = chanM[2];
    try {
      if (!id && method === "POST") return Promise.resolve(addChannel(body, "user") as T);
      if (!id) return Promise.resolve(channelsView() as T);
      const e = findEntry(id);
      if (!e) return Promise.reject(new Error(`there is no channel '${id}'`));
      const list = demoLists.find((l) => l.id === id);
      if (sub === "learn") return Promise.resolve({ started: true } as T);
      if (sub && list) {
        const l = list;
        const count = () => ((l.contacts = Math.max(l.contacts, l.people.length)), (l.unsubscribed = l.people.filter((p) => p.unsubscribed).length), listView(l));
        if (sub === "contacts") {
          if (method === "POST") {
            const fresh = (body.contacts as Contact[]).filter((c) => c.email.includes("@") && !l.people.some((p) => p.email === c.email)).map((c) => ({ ...c, consent: body.consent, added: ago(0), unsubscribed: null }));
            l.people.unshift(...fresh);
            l.contacts += fresh.length;
            return Promise.resolve({ added: fresh.length, list: count() } as T);
          }
          if (method === "DELETE") {
            const had = l.people.length;
            l.people = l.people.filter((p) => p.email !== body.email);
            if (l.people.length < had) l.contacts -= 1;
            return Promise.resolve({ removed: l.people.length < had, list: count() } as T);
          }
          return Promise.resolve({ list: l.id, contacts: l.people } as T);
        }
        if (sub === "import") {
          const rows = String(body.csv).split(/\r?\n/).map((r) => r.split(",").map((x) => x.trim().replace(/^"|"$/g, ""))).filter((r) => r.some((x) => x.includes("@")));
          const fresh = rows.map((r) => ({ email: r.find((x) => x.includes("@"))!, name: r.find((x) => x && !x.includes("@")) ?? "", consent: body.consent, added: ago(0), unsubscribed: null })).filter((c) => !l.people.some((p) => p.email === c.email));
          l.people.unshift(...fresh);
          l.contacts += fresh.length;
          return Promise.resolve({ found: rows.length, added: fresh.length, list: count() } as T);
        }
        if (sub === "unsubscribe") {
          const p = l.people.find((x) => x.email === body.email);
          if (p) p.unsubscribed = ago(0);
          return Promise.resolve({ unsubscribed: !!p, list: count() } as T);
        }
        if (sub === "sends") return Promise.resolve({ list: l.id, sends: demoSends[l.id] ?? [] } as T);
      }
      if (method === "DELETE") {
        if (list) demoLists.splice(demoLists.indexOf(list), 1);
        else entries.splice(entries.indexOf(e), 1);
        demoAccounts.forEach(settle);
        emit({ type: "channel_deleted", channel: id });
        return Promise.resolve(undefined as T);
      }
      if (method === "PUT") {
        const kept: Kept | DemoList = list ?? e;
        if (typeof body.label === "string" && body.label.trim()) kept.label = body.label.trim();
        if (typeof body.description === "string") kept.description = body.description;
        if (typeof body.publisher === "string") {
          const svc = serviceOfKind(e.kind);
          if (body.publisher && !demoAccounts.some((c) => c.id === body.publisher && c.service === svc)) return Promise.reject(new Error(`that account cannot publish to ${e.label}`));
          kept.publisher = body.publisher;
        }
        if (list && typeof body.publish === "string") list.publish = body.publish;
        if (!list) {
          for (const f of ["publish", "reply", "ownership"] as const) if (typeof body[f] === "string") Object.assign(e, { [f]: body[f] });
          for (const f of ["read", "learn"] as const) if (typeof body[f] === "boolean") Object.assign(e, { [f]: body[f] });
          if (body.add) Object.assign(e, { suggested: false, reason: undefined, created_at: ago(0) });
        }
        demoAccounts.forEach(settle);
        const v = channelView(list ? listEntry(list) : e);
        emitChannel(v, v.contacts);
        return Promise.resolve(v as T);
      }
      return Promise.resolve(channelView(e) as T);
    } catch (err) {
      return Promise.reject(err);
    }
  }
  // Accounts: who Seemo signs in as.
  const choicesM = /\/accounts\/[^/]+\/choices\/([^/?]+)$/.exec(path);
  if (choicesM) return Promise.resolve({ name: choicesM[1], choices: demoChoices[choicesM[1]] ?? [] } as T);
  const activityM = /\/accounts\/([^/]+)\/activity/.exec(path);
  if (activityM) {
    const c = demoAccounts.find((x) => x.id === activityM[1]);
    const mine = activityM[1] === demoActivity.account;
    return Promise.resolve({ account: activityM[1], service: c?.service ?? "", today: mine ? demoActivity.today : 0, daily_cap: mine ? demoActivity.daily_cap : null, activity: mine ? demoActivity.activity : [] } as T);
  }
  const acctM = /\/accounts\/([^/?]+)(?:\/(default))?$/.exec(path);
  if (acctM) {
    const i = demoAccounts.findIndex((x) => x.id === acctM[1]);
    const c = demoAccounts[i];
    if (!c) return Promise.reject(new Error(`no account '${acctM[1]}'`));
    if (acctM[2] === "default") demoAccounts.filter((x) => x.service === c.service).forEach((x) => (x.default = x.id === c.id));
    if (!acctM[2] && method === "DELETE") {
      demoAccounts.splice(i, 1);
      // Its channels stay (they are the business's people) and say they need a publisher.
      for (const e of allChannels().filter((e) => e.publisher === c.id)) (demoLists.find((l) => l.id === e.id) ?? entries.find((x) => x.id === e.id))!.publisher = "";
      emit({ type: "account_deleted", account: c.id, label: c.label });
      return Promise.resolve(undefined as T);
    }
    if (!acctM[2] && method === "PUT") {
      if (typeof body.label === "string" && body.label.trim()) Object.assign(c, { label: body.label.trim(), label_set: true });
      if (body.secrets?.store) c.secrets = { ...body.secrets, store_label: body.secrets.store === "kiqr" ? "KIQR" : String(body.secrets.item ?? body.secrets.store), in_kiqr: body.secrets.store === "kiqr" };
    }
    if (method !== "GET") emit({ type: "account_saved", account: c.id, label: c.label });
    return Promise.resolve(settle(c) as T);
  }
  if (/\/accounts(\?|$)/.test(path)) {
    if (method === "POST") {
      const ab = [...demoAbilities.abilities, ...demoAbilities.market].find((x) => x.service === body.service || x.id === body.service);
      return Promise.resolve(demoNewAccount(ab, body) as T);
    }
    const service = /service=([^&]+)/.exec(path)?.[1];
    return Promise.resolve({ accounts: demoAccounts.map(settle).filter((c) => !service || c.service === decodeURIComponent(service)) } as T);
  }
  if (path.endsWith("/email")) return Promise.resolve({ sent: true, to: body.to, subject: body.subject, connection: "Email (SMTP)" } as T);
  if (path.startsWith("/api/secrets/")) return Promise.resolve({ name: path.split("/").pop(), set: !!body.value } as T);
  if (path.endsWith("/chats") && method === "POST") {
    const c: Chat = { id: `demo-chat-${n++}`, title: body.title || "New chat", created_at: new Date().toISOString(), updated_at: new Date().toISOString(), session_id: null, model: "claude-cli/sonnet", last_message: "", messages: 0 };
    chats.unshift(c);
    transcripts[c.id] = [];
    window.setTimeout(() => emit({ type: "chat_created", chat: c.id, title: c.title }), 0);
    return Promise.resolve(c as T);
  }
  if (path.endsWith("/chats")) return Promise.resolve([...chats].sort((a, b) => b.updated_at.localeCompare(a.updated_at)) as T);
  const chatM = /\/chats\/([^/]+)(\/(messages|stop))?$/.exec(path);
  if (chatM) {
    const id = chatM[1];
    const c = chats.find((x) => x.id === id);
    if (!c) return Promise.reject(new Error(`no chat '${id}'`));
    if (chatM[3] === "messages") {
      reply(id, body.text);
      return Promise.resolve({ message: "x", reply: "y" } as T);
    }
    if (chatM[3] === "stop") return Promise.resolve({ stopping: false } as T);
    if (method === "DELETE") {
      chats.splice(chats.indexOf(c), 1);
      return Promise.resolve(undefined as T);
    }
    if (method === "PUT") {
      c.title = body.title;
      return Promise.resolve(c as T);
    }
    return Promise.resolve({ ...c, messages: transcripts[id] ?? [], streaming: null } as ChatView as T);
  }
  const apprM = /\/approvals\/([^/?]+)$/.exec(path);
  if (apprM) return Promise.resolve(resolveCard(decodeURIComponent(apprM[1]), String(body.decision)) as T);
  if (path.endsWith("/tasks")) return Promise.resolve([weekly, task] as T);
  const runsM = /\/tasks\/([^/]+)\/runs$/.exec(path);
  if (runsM) return Promise.resolve((runsM[1] === task.id ? [run] : []) as T);
  const taskM = /\/tasks\/([^/]+)(\/(run|pause|resume))?$/.exec(path);
  if (taskM) {
    const t = [weekly, task].find((x) => x.id === taskM[1]);
    if (!t) return Promise.reject(new Error(`no task '${taskM[1]}'`));
    if (taskM[3] === "pause") t.status = "paused";
    if (taskM[3] === "resume") t.status = "active";
    if (taskM[3] === "run") return Promise.resolve({ started: true, run: "demo" } as T);
    if (method === "DELETE") return Promise.resolve(undefined as T);
    return Promise.resolve(t as T);
  }
  if (path.includes("/calendar")) {
    const items: CalendarItem[] = [];
    const d = new Date(weekly.next_run!);
    for (let i = 0; i < 5; i++) {
      items.push({ kind: "task", task: weekly.id, title: weekly.title, at: new Date(d.getTime() + i * 7 * 86_400_000).toISOString(), schedule_text: weekly.schedule_text });
    }
    for (const a of demoAssetList().filter((a) => a.status === "scheduled" && a.publish_at)) {
      const e = a.target ? findEntry(a.target) : undefined;
      const c = demoAccounts.find((c) => c.id === e?.publisher);
      items.push({ kind: "publish", asset: { type: a.type, id: a.id, title: a.title }, target: a.target ?? null, target_label: e?.label ?? a.target ?? "", via: c?.label ?? null, title: a.title, at: a.publish_at!, schedule_text: `to ${e?.label ?? a.target}${c && c.label !== e?.label ? ` via ${c.label}` : ""}` });
    }
    items.sort((x, y) => x.at.localeCompare(y.at));
    return Promise.resolve(items as T);
  }
  if (/\/performance(\/refresh)?$/.test(path)) {
    const days = Array.from({ length: 30 }, (_, i) => new Date(Date.now() - (29 - i) * 86_400_000).toISOString().slice(0, 10));
    const series = (base: number, slope: number, noise = 0.15) => days.map((date, i) => ({ date, value: Math.max(0, Math.round(base + i * slope + (Math.sin(i * 1.7) + Math.cos(i * 0.6)) * base * noise)) }));
    const sum = (sr: { value: number }[], from: number, to: number) => sr.slice(from, to).reduce((a, b) => a + b.value, 0);
    const met = (source: string, metric: string, label: string, sr: { date: string; value: number }[], lower = false) => ({ source, metric, label, lower_is_better: lower, last7: sum(sr, 23, 30), prev7: sum(sr, 16, 23), last30: sum(sr, 0, 30), series: sr });
    const sessions = series(92, 1.1);
    const organic = series(58, 1.3);
    const clicks = series(31, 0.6);
    const impressions = series(1150, 18);
    const published = days.map((date, i) => ({ date, value: i % 6 === 0 ? 1 : 0 }));
    const v: Performance = {
      sources: [
        { id: "traffic", label: "Traffic", provider: "google-analytics", candidates: [{ id: "google-analytics", name: "Read Google Analytics", ready: true }], taken_at: ago(300), from: days[0], to: days[29], rows: 180 },
        { id: "search", label: "Search", provider: "search-console", candidates: [{ id: "search-console", name: "Read Google Search Console", ready: true }], taken_at: ago(300), from: days[0], to: days[29], rows: 170 },
        { id: "conversions", label: "Conversions", provider: "google-analytics", candidates: [{ id: "google-analytics", name: "Read Google Analytics", ready: true }], taken_at: ago(300), from: days[0], to: days[29], rows: 60 },
        { id: "ranks", label: "Ranks", provider: null, candidates: [], taken_at: null, from: null, to: null, rows: 0 },
        { id: "mentions", label: "Mentions", provider: null, candidates: [], taken_at: null, from: null, to: null, rows: 0 },
        { id: "kiqr", label: "Published by KIQR", provider: "kiqr", candidates: [], taken_at: ago(300), from: days[0], to: days[29], rows: 30 },
      ],
      metrics: [
        met("traffic", "sessions", "Sessions", sessions),
        met("traffic", "users", "Users", series(80, 0.9)),
        met("traffic", "organic_sessions", "Organic sessions", organic),
        met("traffic", "pageviews", "Page views", series(210, 2.4)),
        met("search", "clicks", "Clicks", clicks),
        met("search", "impressions", "Impressions", impressions),
        { ...met("search", "average_position", "Average position", series(18, -0.12, 0.05), true), last7: 14.9, prev7: 16.2, last30: 15.8 },
        met("conversions", "key_events", "Key events", series(3, 0.05, 0.4)),
        met("kiqr", "published", "Pieces published", published),
      ],
      tops: {
        "search.clicks.query": [
          { key: "scrape idealista", value: 61 },
          { key: "residential proxies vs datacenter", value: 44 },
          { key: "cloudflare challenge scraping", value: 38 },
          { key: "unblockingapi", value: 35 },
          { key: "scraping api pricing", value: 19 },
        ],
        "search.clicks.page": [
          { key: "https://unblockingapi.com/templates", value: 120 },
          { key: "https://unblockingapi.com/pricing", value: 74 },
          { key: "https://unblockingapi.com/blog/scrape-idealista", value: 52 },
        ],
        "conversions.key_events.event": [
          { key: "sign_up", value: 48 },
          { key: "api_key_created", value: 31 },
        ],
        "kiqr.published.channel": [{ key: "x-unblockingapi", value: 3 }, { key: "blog", value: 2 }],
      },
      kpis: [
        { objective: "rank-for-the-blocked-page-queries", title: "Rank for the blocked-page queries", kpi: "organic_traffic", label: "Organic traffic", target: 4000, unit: "sessions/month", by: "2026-12-31", source: "traffic", measured: true, value: sum(organic, 0, 30), prev: sum(organic, 0, 30) * 0.84, window_days: 30 },
        { objective: "turn-templates-into-sign-ups", title: "Turn templates into sign-ups", kpi: "signups", label: "Sign-ups", target: 120, unit: "per month", by: "2026-12-31", source: "conversions", measured: true, value: 79, prev: 64, window_days: 30 },
        { objective: "be-present-where-scrapers-ask", title: "Be present where scrapers ask", kpi: "brand_mentions", label: "Brand mentions", target: null, unit: "", by: null, source: "mentions", measured: false, value: null, prev: null, window_days: 30 },
      ],
      plan_period: "Q4 2026",
      today: days[29],
      refreshed: path.endsWith("/refresh") ? [{ note: "traffic: 180 rows via google-analytics", ok: true }, { note: "search: 170 rows via search-console", ok: true }] : undefined,
    };
    return Promise.resolve(v as T);
  }
  if (path.endsWith("/plan")) {
    const v: PlanView = { plan: demoPlan, kpis: [["organic_traffic", "Organic traffic"], ["qualified_leads", "Qualified leads"], ["signups", "Sign-ups"], ["conversion_rate", "Conversion rate"], ["retention", "Retention"], ["brand_mentions", "Brand mentions"], ["rank_positions", "Rank positions"], ["revenue", "Revenue"], ["cac", "Customer acquisition cost"], ["published", "Pieces published"]].map(([id, label]) => ({ id, label })), campaigns: demoCampaigns };
    return Promise.resolve(v as T);
  }
  const campM = /\/campaigns(\/([^/?]+))?(\/attach)?$/.exec(path);
  if (campM) {
    if (method === "POST" && !campM[2]) {
      const c: Campaign = { id: String(body.title ?? "campaign").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, ""), title: body.title, goal: body.goal ?? "", objective: body.objective ?? null, objective_title: demoPlan.objectives.find((o) => o.id === body.objective)?.title ?? null, channels: body.channels ?? [], start: null, end: null, status: "active", created_by: "user", created_at: ago(0), updated_at: ago(0), assets: [], tasks: [] };
      demoCampaigns.push(c);
      return Promise.resolve(c as T);
    }
    const c = demoCampaigns.find((x) => x.id === campM[2]);
    if (method === "DELETE" && c) demoCampaigns.splice(demoCampaigns.indexOf(c), 1);
    if (method === "PUT" && c) Object.assign(c, body, { updated_at: ago(0) });
    return Promise.resolve((campM[2] ? c : demoCampaigns) as T);
  }
  if (path.endsWith("/todo")) {
    const all = demoAssetList();
    const todo: Todo = {
      // Cards still waiting for a yes, from every chat.
      approvals: Object.entries(transcripts).flatMap(([chat, list]) => list.filter((m) => m.card?.status === "pending" && m.card.approval).map((m) => ({ id: m.card!.approval!, kind: m.card!.approval_kind ?? "", chat, chat_title: chats.find((c) => c.id === chat)?.title, message: m.id, created: m.at, payload: m.card!.payload }))),
      drafts: all.filter((a) => a.status === "draft"),
      ready: all.filter((a) => a.status === "ready"),
      scheduled: all.filter((a) => a.status === "scheduled").sort((x, y) => (x.publish_at ?? "").localeCompare(y.publish_at ?? "")),
      attention: all.filter((a) => !!a.publish_error),
      credentials_needing_hand: [],
      channels_waiting: channelsView().channels.filter((c) => !c.suggested && !["ready", "publish_off", "suggested"].includes(c.readiness.state)).map((c) => ({ id: c.id, label: c.label, domain: c.domain, kind: c.kind, readiness: c.readiness })),
      count: 0,
    };
    todo.count = todo.approvals.length + todo.drafts.length + todo.attention.length + (todo.credentials_needing_hand?.length ?? 0);
    return Promise.resolve(todo as T);
  }
  return Promise.resolve({} as T);
}
