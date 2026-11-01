// Demo mode for Seemo: a saved conversation, a task, and a scripted reply
// when you send something, so the chat screens work without a daemon.
import type { Ability, AbilitiesView, Account, Allow, CalendarItem, Campaign, ChannelKindInfo, Chat, ChatView, Contact, KiqrEvent, ListSend, MailList, Message, Performance, Permission, Place, PlaceField, PlaceKind, Plan, PlanView, Run, Task, Todo } from "./api";
import { demoAssetList } from "./demo-assets";

const slug = "unblockingapi-com";
const ago = (min: number) => new Date(Date.now() - min * 60_000).toISOString();
const DEMO_IMAGE =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif"><rect width="1600" height="900" fill="#0B1220"/><rect x="128" y="128" width="12" height="644" fill="#ef2f72"/><text x="188" y="330" fill="#FFFFFF" font-size="88" font-weight="700">One request.</text><text x="188" y="436" fill="#FFFFFF" font-size="88" font-weight="700">One credit.</text><text x="188" y="540" fill="#CBD5E1" font-size="36">No multipliers. Failed requests are free.</text><text x="188" y="744" fill="#ef2f72" font-size="30" font-weight="600">UnblockingAPI</text></svg>`,
  );

const perm = (pairs: [string, Allow][]): Permission[] => pairs.map(([role, allow]) => ({ role, allow, acts: ["reply", "publish", "manage"].includes(role) }));
const smtpKeys = [{ env: "SMTP_HOST", label: "Mail server", help: "", optional: false, set: true }, { env: "SMTP_USER", label: "User name", help: "", optional: false, set: true }, { env: "SMTP_PASSWORD", label: "Password", help: "", optional: false, set: true }];
const GOOGLE_CARD = { id: "google", name: "Google", icon: "chart", group: "numbers" };
const GA_FIELDS: Record<string, PlaceField> = { property: { label: "GA4 property", help: "The property whose numbers Seemo reads", choices: { list: "/properties" } } };
const SC_FIELDS: Record<string, PlaceField> = { site: { label: "Search Console property", help: "A domain (sc-domain:…) or a URL prefix", choices: { list: "/siteEntry" } } };
const GA_KIND: PlaceKind = { kind: "google-analytics", label: "Read Google Analytics", verb: "Measures", fields: GA_FIELDS, needs: true };
const SC_KIND: PlaceKind = { kind: "search-console", label: "Read Google Search Console", verb: "Measures", fields: SC_FIELDS, needs: true };
/** What a demo Google account has to pick from. */
const demoChoices: Record<string, { value: string; label: string }[]> = {
  property: [{ value: "properties/318204551", label: "unblockingapi.com" }, { value: "properties/318204777", label: "docs.unblockingapi.com" }, { value: "properties/290011342", label: "Old site (UA import)" }],
  site: [{ value: "sc-domain:unblockingapi.com", label: "sc-domain:unblockingapi.com" }, { value: "https://docs.unblockingapi.com/", label: "https://docs.unblockingapi.com/" }],
};
const demoAccounts: Account[] = [
  // A mail account: it sends, and its places are lists of people.
  { id: "hello-mail", label: "hello@unblockingapi.com", ability: "kiqr-smtp", ability_name: "Send email from your mail account", default: true, channel: true, publishes: true, takes_lists: true, missing: [], auth: smtpKeys, kind: "email", kind_label: "Email", created_by: "user", created_at: ago(300), allow: perm([["read", "auto"], ["publish", "ask"]]), places: [{ id: "list-newsletter", kind: "list", label: "Newsletter", fields: { list: "newsletter" }, default: true }], place_fields: { to: { label: "To", help: "One address, or several separated by commas" } }, needs_place: false, place_kinds: [{ kind: "email", label: "Email", verb: "Publishes to", fields: { to: { label: "To", help: "One address, or several separated by commas" } }, needs: false, lists: true }], publications: 1, last_published: ago(60), connection_name: "Send email from your mail account", ready: true, signed_in: true, destination_text: "Newsletter", service: "kiqr-smtp", service_card: { id: "kiqr-smtp", name: "Email", icon: "mail", group: "email" } },
  // One page, three abilities of the same service: two can publish, one reads and replies.
  { id: "company-page", label: "Company page", ability: "linkedin-api", ability_name: "Post to LinkedIn", default: true, channel: true, publishes: true, missing: [], auth: [{ env: "LINKEDIN_TOKEN", label: "Sign-in", help: "", optional: false, set: true, written: true }], kind: "linkedin", kind_label: "LinkedIn page", created_by: "user", created_at: ago(900), allow: perm([["read", "auto"], ["reply", "ask"], ["publish", "ask"], ["manage", "off"]]), places: [], place_fields: {}, needs_place: false, place_kinds: [{ kind: "linkedin", label: "LinkedIn page", verb: "Publishes to", fields: {}, needs: false }], publications: 4, last_published: ago(120), connection_name: "Post to LinkedIn", ready: true, signed_in: true, destination_text: "", service: "linkedin", service_card: { id: "linkedin", name: "LinkedIn", icon: "linkedin", group: "social" }, publisher: "linkedin-api", publisher_name: "Post to LinkedIn", publishers: [{ id: "linkedin-api", name: "Post to LinkedIn" }, { id: "linkedin-browser", name: "Post to LinkedIn with a browser" }], used_by: [{ id: "linkedin-api", name: "Post to LinkedIn", does: ["publish"] }, { id: "linkedin-browser", name: "Post to LinkedIn with a browser", does: ["publish", "manage"] }, { id: "linkedin-replies", name: "Reply to comments on LinkedIn", does: ["read", "reply"] }] },
  // A service with several places: the subreddits it posts to.
  { id: "u-unblockingapi", label: "u/unblockingapi", ability: "reddit", ability_name: "Post to Reddit", default: true, channel: true, publishes: true, missing: [], auth: [{ env: "REDDIT_TOKEN", label: "Sign-in", help: "", optional: false, set: true, written: true }], kind: "reddit", kind_label: "Subreddit", created_by: "user", created_at: ago(2000), allow: perm([["read", "auto"], ["reply", "auto"], ["publish", "auto"]]), places: [{ id: "webscraping", kind: "reddit", label: "r/webscraping", fields: { subreddit: "webscraping" }, default: true }, { id: "selfhosted", kind: "reddit", label: "r/selfhosted", fields: { subreddit: "selfhosted" } }], place_fields: { subreddit: { label: "Subreddit", placeholder: "webscraping", help: "The name without r/" } }, needs_place: true, place_kinds: [{ kind: "reddit", label: "Subreddit", verb: "Publishes to", fields: { subreddit: { label: "Subreddit", placeholder: "webscraping", help: "The name without r/" } }, needs: false }], publications: 2, last_published: ago(25), connection_name: "Post to Reddit", ready: true, signed_in: true, destination_text: "r/webscraping", service: "reddit", service_card: { id: "reddit", name: "Reddit", icon: "message-square-text", group: "social" } },
  // Numbers only: Seemo reads it into Performance.
  // One Google sign-in shared by Google Analytics and Search Console: a place of each kind on it.
  { id: "google", label: "Google", ability: "google-analytics", ability_name: "Read Google Analytics", default: true, channel: false, publishes: false, missing: [], auth: [{ env: "GOOGLE_TOKEN", label: "Sign-in", help: "", optional: false, set: true, written: true }], connect: { label: "Sign in with Google", flow: "oauth2", needs: [], returns: ["GOOGLE_TOKEN"], https_only: false, via_cloud: true }, kind: "", created_by: "user", created_at: ago(4000), allow: perm([["read", "auto"], ["measure", "auto"]]), places: [{ id: "ga-unblockingapi", kind: "google-analytics", label: "unblockingapi.com", fields: { property: "properties/318204551" }, default: true }, { id: "sc-unblockingapi", kind: "search-console", label: "sc-domain:unblockingapi.com", fields: { site: "sc-domain:unblockingapi.com" }, default: true }], place_kinds: [GA_KIND, SC_KIND], publications: 0, connection_name: "Read Google Analytics", ready: true, signed_in: true, destination_text: "", service: "google", service_card: GOOGLE_CARD, used_by: [{ id: "google-analytics", name: "Read Google Analytics", does: [] }, { id: "search-console", name: "Read Google Search Console", does: [] }] },
  // Signed in but not finished: the property is still to choose.
  { id: "kiqr-repo", label: "kiqr/kiqr", ability: "github", ability_name: "Publish to GitHub", default: true, channel: true, publishes: true, missing: ["GITHUB_REPO"], auth: [{ env: "GITHUB_TOKEN", label: "Sign-in", help: "", optional: false, set: true, written: true }, { env: "GITHUB_REPO", label: "Repository", help: "owner/name", optional: false, set: false, placeholder: "kiqr/kiqr" }], connect: { label: "Connect with GitHub", flow: "oauth2", needs: [], returns: ["GITHUB_TOKEN"], https_only: false, via_cloud: true }, kind: "github", kind_label: "Repository", created_by: "user", created_at: ago(30), allow: perm([["read", "auto"], ["publish", "ask"], ["learn", "auto"]]), places: [], place_fields: { branch: { label: "Branch", optional: true, placeholder: "main" }, path: { label: "Folder", optional: true, placeholder: "content/blog" } }, needs_place: false, place_kinds: [{ kind: "github", label: "Repository", verb: "Publishes to", fields: { branch: { label: "Branch", optional: true, placeholder: "main" }, path: { label: "Folder", optional: true, placeholder: "content/blog" } }, needs: false }], publications: 0, connection_name: "Publish to GitHub", ready: false, signed_in: true, destination_text: "", service: "github", service_card: { id: "github", name: "GitHub", icon: "globe", group: "website" }, can_learn: true, learn: { label: "Learn the product from its README and docs" }, source: true },
  // Signed in on KIQR's own browser from a login saved in KIQR; the site asked for a person.
  { id: "x-acme", label: "@unblockingapi", ability: "x-browser", ability_name: "X in a browser", default: true, channel: true, publishes: true, missing: [], auth: [], kind: "x", kind_label: "X account", created_by: "user", created_at: ago(1500), allow: perm([["read", "auto"], ["reply", "ask"], ["publish", "ask"]]), places: [], place_fields: {}, needs_place: false, place_kinds: [], publications: 4, last_published: ago(600), connection_name: "X in a browser", ready: true, signed_in: true, destination_text: "", identity: "@unblockingapi", service: "x", service_card: { id: "x", name: "X", icon: "at-sign", group: "social" }, has_session: true, session: { state: "needs_hand", message: "the site asks for a person: verify you are human", checked_at: ago(12), signed_in_at: ago(3000) }, login: { store: "kiqr" }, login_set: ["LOGIN_USERNAME", "LOGIN_PASSWORD", "LOGIN_TOTP_SECRET"], live_view: true },
];
type DemoList = MailList & { people: Contact[] };
const person = (i: number, unsub = false): Contact => ({ email: `${["ana", "ben", "chloe", "dev", "eli", "fay", "gus", "hana", "ivo", "jon", "kim", "lou"][i % 12]}${i > 11 ? i : ""}@example.com`, name: ["Ana Ruiz", "Ben Ode", "Chloe Park", "Dev Rao", "Eli Stone", "Fay Lin", "Gus Berg", "Hana Ito", "Ivo Petrov", "Jon Hale", "Kim Moss", "Lou Amar"][i % 12], consent: "Signed up on the website", added: ago(10_000 - i * 30), unsubscribed: unsub ? ago(400) : null });
const demoLists: DemoList[] = [
  { id: "newsletter", label: "Newsletter", description: "Everyone who signed up on the website for product news.", contacts: 412, unsubscribed: 3, created_at: ago(20_000), people: Array.from({ length: 14 }, (_, i) => person(i, i === 4)) },
  { id: "beta-testers", label: "Beta testers", description: "People trying the Visual Editor before launch.", contacts: 38, unsubscribed: 0, created_at: ago(3000), people: Array.from({ length: 5 }, (_, i) => person(i + 3)) },
];
const listView = ({ people: _p, ...l }: DemoList): MailList => l;
const demoSends: Record<string, ListSend[]> = { newsletter: [{ key: "email-october-news", sent: 409, failed: 3, at: ago(8000), running: false }] };
const demoKinds: ChannelKindInfo[] = [
  { kind: "email", label: "Email", connection: "kiqr-smtp", connection_name: "Send email from your mail account", ready: true, place: { to: { label: "To", help: "One address, or several separated by commas" } }, needs_place: false, takes_lists: true },
  { kind: "linkedin", label: "LinkedIn page", connection: "linkedin-api", connection_name: "Post to LinkedIn", ready: true, place: {}, needs_place: false },
  { kind: "reddit", label: "Subreddit", connection: "reddit", connection_name: "Post to Reddit", ready: true, place: { subreddit: { label: "Subreddit" } }, needs_place: true },
  { kind: "github", label: "Repository", connection: "github", connection_name: "Publish to GitHub", ready: true, place: { branch: { label: "Branch", optional: true } }, needs_place: false, options: { status: { type: "string", enum: ["draft", "publish"], description: "Open a pull request, or commit to the branch" } } },
];
/** An account with its default place's label, as the daemon keeps it. */
function settleAccount(a: Account) {
  a.destination_text = a.places?.find((p) => p.default)?.label ?? a.places?.[0]?.label ?? "";
  a.publishes = a.channel && (a.allow ?? []).find((p) => p.role === "publish")?.allow !== "off";
  a.place_kinds = a.place_kinds?.map((k) => ({ ...k, needs: k.needs && !(a.places ?? []).some((p) => p.kind === k.kind) }));
  return a;
}
demoAccounts.forEach(settleAccount);
const demoActivity = { connection: "company-page", service: "linkedin", today: 3, daily_cap: 20, activity: [
  { at: ago(12), ability: "linkedin-replies", role: "reply", tool: "reply", target: "comment 7231", url: "https://www.linkedin.com/feed/" },
  { at: ago(40), ability: "linkedin-replies", role: "reply", tool: "reply", target: "comment 7219", url: null },
  { at: ago(120), ability: "linkedin-api", role: "publish", tool: "post_update", target: "", url: "https://www.linkedin.com/feed/" },
  { at: ago(1500), ability: "linkedin-browser", role: "manage", tool: "update_profile", target: "headline", url: null },
] };
const demoAbilities: AbilitiesView = {"abilities": [{"id": "write", "name": "Write articles, posts and emails", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Seemo writes in the brand's tone of voice from the business brief; everything lands in the library as a draft.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "pen-tool", "cost": "model usage", "provides": [], "channels": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "business-profile", "name": "Crawl a website and build its business profile", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Crawl the site, extract what it says, and write the profile, the brand and the business brief Seemo works from.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["web", "writing"], "icon": "brain", "cost": "model usage", "provides": [], "channels": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "crawl-site", "label": "Reading the site", "project_copy": false, "machine_copy": false}, {"file": "extract-facts", "label": "Extracting facts", "project_copy": false, "machine_copy": false}, {"file": "build-profile", "label": "Building the profile", "project_copy": false, "machine_copy": false}, {"file": "business-brief", "label": "Writing the brief", "project_copy": false, "machine_copy": false}], "in_prompt": false, "error": null}, {"id": "read-web", "name": "Read the web", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Seemo reads one page at a time through KIQR's own fetcher.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["web"], "icon": "globe", "cost": "free", "provides": [], "channels": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "unblockingapi", "name": "Read blocked pages with UnblockingAPI", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "A real browser and residential proxies for the pages KIQR could not read. One credit per success.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "https://unblockingapi.com", "categories": ["web"], "icon": "shield-check", "cost": "about $0.001 per page", "provides": [], "channels": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [{"env": "UNBLOCKINGAPI_KEY", "label": "UnblockingAPI key", "help": "https://unblockingapi.com", "optional": false, "set": "global"}], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "social-posts", "name": "Write social media posts", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Ready-to-publish posts in the brand's tone of voice, each with its image.", "description": "Write a few ready-to-publish social media posts from the business brief, in the brand's tone of voice.", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing", "publishing"], "icon": "message-square-text", "cost": "model usage", "provides": [], "channels": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "social-posts", "label": "Writing the posts", "project_copy": false, "machine_copy": false}], "in_prompt": true, "error": null}, {"id": "tasks", "name": "Run tasks on a schedule", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Seemo works later or repeatedly; every task is approved by you first.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "list-checks", "cost": "model usage", "provides": [], "channels": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [], "in_prompt": true, "error": null}, {"id": "kiqr-render", "name": "Create images with the built-in renderer", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "On-brand images Seemo designs and KIQR renders here. Free, on from the start.", "description": "How to make an on-brand image for a post, an article cover or a standalone visual with the built-in renderer, or with a generator when the user wants a photo or an illustration.", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["images"], "icon": "image", "cost": "free", "provides": [{"ability": "image", "label": "Make images"}], "channels": [], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "shipped", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": "~/.kiqr/abilities/make-images", "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": ["image"], "instructions": [{"file": "make-images", "label": "Create images with the built-in renderer", "project_copy": false, "machine_copy": false}], "in_prompt": true, "error": null}, {"id": "kiqr-smtp", "name": "Send email from your mail account", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Email over SMTP. Each project connects its own account. Every send asks you first.", "description": "", "version": "0.1.0", "latest": "0.2.0", "homepage": "", "categories": ["email", "publishing"], "icon": "mail", "cost": "free", "provides": [{"ability": "email", "label": "Send email"}], "channels": ["Email"], "sources": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "shipped", "repo": null, "core": false, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": "~/.kiqr/abilities/email-smtp", "git": false, "updatable": true, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [{"env": "SMTP_HOST", "label": "Mail server", "help": "Gmail: smtp.gmail.com", "optional": false, "set": "project"}, {"env": "SMTP_USER", "label": "User name", "help": "Usually your email address", "optional": false, "set": "project"}, {"env": "SMTP_PASSWORD", "label": "Password or app password", "help": "https://myaccount.google.com/apppasswords", "optional": true, "set": "project"}], "active_for": ["email"], "instructions": [], "in_prompt": false, "error": null}, {"id": "product-update-posts", "name": "Write product update posts", "author": "you", "author_url": null, "summary": "When asked for a post about something we shipped.", "description": "When asked for a post about something we shipped.", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "book-open", "cost": "model usage", "provides": [], "channels": [], "sources": [], "tools": 0, "trust": "local", "transport": "instructions", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "yours", "repo": null, "core": false, "builtin": false, "installed": true, "scope": {"scope": "project", "project": "unblockingapi-com"}, "folder": "~/.kiqr/projects/unblockingapi-com/abilities/product-update-posts", "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "product-update-posts", "label": "Write product update posts", "project_copy": true, "machine_copy": false}], "in_prompt": true, "error": null}], "market": [{"id": "fal-image", "name": "Create images with fal.ai", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "Photos and illustrations from a text prompt.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "https://fal.ai", "categories": ["images"], "icon": "image", "cost": "about $0.03 per image", "provides": [{"ability": "image", "label": "Make images"}], "channels": [], "sources": [], "tools": 0, "trust": "official", "transport": "http", "runs_program": false, "command": null, "program_found": null, "hosts": ["fal.run", "fal.media"], "source": "shipped", "repo": null, "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": ["FAL_KEY"], "auth": [{"env": "FAL_KEY", "label": "fal.ai API key", "help": "https://fal.ai/dashboard/keys", "optional": false, "set": null}], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "openai-image", "name": "Create images with OpenAI", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "gpt-image-1 behind your OpenAI key.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "https://platform.openai.com", "categories": ["images"], "icon": "image", "cost": "about $0.04 per image", "provides": [{"ability": "image", "label": "Make images"}], "channels": [], "sources": [], "tools": 0, "trust": "official", "transport": "http", "runs_program": false, "command": null, "program_found": null, "hosts": ["api.openai.com"], "source": "shipped", "repo": null, "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": ["OPENAI_API_KEY"], "auth": [{"env": "OPENAI_API_KEY", "label": "OpenAI API key", "help": "https://platform.openai.com/api-keys", "optional": false, "set": null}], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "playwright-browser", "name": "Drive a browser with Playwright", "author": "kiqr-cloud", "author_url": "https://github.com/kiqr-cloud", "summary": "A real browser Seemo can drive: sites without an API, screenshots, logged-in sessions.", "description": "", "version": "0.2.0", "latest": "0.2.0", "homepage": "https://github.com/microsoft/playwright-mcp", "categories": ["browser", "tools", "publishing"], "icon": "", "cost": "free", "provides": [], "channels": [], "sources": [], "tools": 4, "trust": "verified", "transport": "mcp", "runs_program": true, "command": "npx @playwright/mcp@latest --headless", "program_found": null, "hosts": [], "source": "marketplace", "repo": "https://github.com/kiqr-cloud/ability-playwright.git", "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": [], "auth": [], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "fal-video", "name": "Create video with fal.ai", "author": "kiqr-cloud", "author_url": "https://github.com/kiqr-cloud", "summary": "Short clips from a prompt. Slow and paid; Seemo says what it costs first.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "https://fal.ai", "categories": ["video"], "icon": "", "cost": "about $0.50 per clip", "provides": [{"ability": "video", "label": "Make video"}], "channels": [], "sources": [], "tools": 0, "trust": "official", "transport": "http", "runs_program": false, "command": null, "program_found": null, "hosts": ["fal.run"], "source": "marketplace", "repo": "https://github.com/kiqr-cloud/ability-fal-video.git", "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": ["FAL_KEY"], "auth": [{"env": "FAL_KEY", "label": "fal.ai API key", "help": "https://fal.ai/dashboard/keys", "optional": false, "set": null}], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "wordpress", "name": "Publish articles to WordPress", "author": "acme-digital", "author_url": "https://github.com/acme-digital", "summary": "Publish articles to your WordPress site as drafts or posts.", "description": "", "version": "0.3.1", "latest": "0.3.1", "homepage": "https://wordpress.org", "categories": ["publishing"], "icon": "", "cost": "free", "provides": [], "channels": ["WordPress blog"], "sources": [], "tools": 1, "trust": "community", "transport": "http", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "marketplace", "repo": "https://github.com/acme-digital/kiqr-wordpress.git", "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": ["WP_USER", "WP_APP_PASSWORD"], "auth": [{"env": "WP_USER", "label": "User name", "help": "", "optional": false, "set": null}, {"env": "WP_APP_PASSWORD", "label": "Application password", "help": "", "optional": false, "set": null}], "connect": {"label": "Connect with WordPress", "flow": "redirect", "needs": [], "returns": ["WP_APP_PASSWORD", "WP_USER"], "https_only": true}, "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "weekly-report", "name": "Write the Monday report", "author": "acme-digital", "author_url": "https://github.com/acme-digital", "summary": "What was added, what ran, what to write next: a short report every Monday, in plain words.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "book-open", "cost": "model usage", "provides": [], "channels": [], "sources": [], "tools": 0, "trust": "community", "transport": "instructions", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "marketplace", "repo": "https://github.com/acme-digital/kiqr-abilities.git", "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "weekly-report", "label": "Write the Monday report", "project_copy": false, "machine_copy": false}], "in_prompt": false, "error": null}], "contracts": [{"id": "image", "label": "Make images", "tool": "create_image", "configured": null, "active": {"id": "kiqr-render", "name": "Create images with the built-in renderer"}, "candidates": [{"id": "kiqr-render", "name": "Create images with the built-in renderer", "ready": true}]}, {"id": "video", "label": "Make video", "tool": "create_video", "configured": null, "active": null, "candidates": []}, {"id": "music", "label": "Make music", "tool": "create_music", "configured": null, "active": null, "candidates": []}, {"id": "email", "label": "Send email", "tool": "send_email", "configured": null, "active": {"id": "kiqr-smtp", "name": "Send email from your mail account"}, "candidates": [{"id": "kiqr-smtp", "name": "Send email from your mail account", "ready": true}]}], "broken": [], "guide": {"id": "seemo-agent", "name": "Seemo's own guide", "author": "KIQR", "author_url": "https://github.com/kiqr", "summary": "How Seemo works inside KIQR: the tools, when to act and when to make a task, how to ask for a yes, how to report.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "book-open", "cost": "model usage", "provides": [], "channels": [], "sources": [], "tools": 0, "trust": "official", "transport": "instructions", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": "~/.kiqr/abilities/seemo-agent", "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "seemo-agent", "label": "How Seemo works inside KIQR", "project_copy": false, "machine_copy": false}], "in_prompt": false, "error": null}, "index": "https://kiqr.cloud/marketplace/index.json", "env_file": "~/.kiqr/.env", "project_env_file": "~/.kiqr/projects/unblockingapi-com/.env"};
// The services behind the demo's abilities, as the grid of things to connect shows them.
const DEMO_SERVICES: Record<string, { name: string; icon: string; group: string; allow?: Record<string, Allow> }> = {
  "kiqr-smtp": { name: "Email", icon: "mail", group: "email", allow: { publish: "ask" } },
  wordpress: { name: "WordPress", icon: "globe", group: "website", allow: { publish: "ask" } },
  "fal-image": { name: "fal.ai", icon: "image", group: "tools" },
  "openai-image": { name: "OpenAI", icon: "image", group: "tools" },
  "fal-video": { name: "fal.ai video", icon: "video", group: "tools" },
  "playwright-browser": { name: "A browser", icon: "globe", group: "sites" },
};
for (const a of [...demoAbilities.abilities, ...demoAbilities.market]) {
  const s = DEMO_SERVICES[a.id];
  if (s) Object.assign(a, { service: a.id, service_card: { id: a.id, name: s.name, icon: s.icon, group: s.group }, allow: s.allow ?? {} });
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
    demoAbilities.market.push({ ...base, id, name, summary, author: "KIQR", author_url: "https://github.com/kiqr", trust: "official", channels, sources: group === "numbers" ? ["traffic"] : [], categories: [group], homepage: "", repo: `https://github.com/kiqr/marketplace`, connect: { label: `Connect with ${name.replace(/^(Post to|Read|Passwords from) /, "")}`, flow: "oauth2", needs: [], returns: [], https_only: false, via_cloud: true }, auth: [{ env: `${service.toUpperCase().replace(/-/g, "_")}_TOKEN`, label: "Sign-in", help: "", optional: false, set: null }], missing: [], service, service_card: { id: service, name: name.replace(/^(Post to|Read|Passwords from) /, ""), icon: group === "numbers" ? "chart" : "globe", group }, allow: channels.length ? { publish: "ask", reply: "ask" } : {} });
}

{
  // Services that run on KIQR's own browser: KIQR installs the browser first, and they sign in there.
  const base = demoAbilities.market.find((x) => x.id === "wordpress")!;
  const programs = () => [
    { program: "chromium", version: "131.0", found: false, how: "download" },
    { program: "playwright-mcp", version: "0.0.41", found: false, how: "uv" },
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
      kinds: id === "reddit-browser" ? [{ kind: "reddit", label: "Subreddit", name: "Reddit", place: { subreddit: { label: "Subreddit", placeholder: "webscraping", help: "The name without r/" } } }] : [],
      missing: [],
      service,
      service_card: { id: service, name: card, icon: group === "sites" ? "globe" : service === "x" ? "at-sign" : "message-square-text", group },
      allow: channels.length ? { publish: "ask", reply: "ask" } : {},
      _programs: programs(),
    } as Ability);
}
/** The programs a demo browser ability needs, until "installed". */
const demoPrograms = (a: Ability) => (a as Ability & { _programs?: Ability["programs"] })._programs ?? null;

/** A new demo account of an ability, as creating one or starting a sign-in adds it. */
function demoNewAccount(ab: Ability | undefined, body: Record<string, unknown>): Account {
  const card = ab?.service_card ?? { id: ab?.id ?? "service", name: ab?.name ?? "Account", group: "tools" };
  const roles = ["read", ...(ab?.channels.length ? ["reply", "publish"] : []), ...(ab?.sources.length ? ["measure"] : [])];
  const chosen = { ...(ab?.allow ?? {}), ...((body.allow as Record<string, Allow>) ?? {}) };
  const place = (body.place as Record<string, string>) ?? {};
  const c: Account = {
    id: `account-${n++}`,
    label: String(body.label || card.name || "Account"),
    place_kinds: [
      ...(ab?.kinds ?? []).slice(0, 1).map((k) => ({ kind: k.kind, label: k.label, verb: "Publishes to", fields: k.place, needs: false })),
      ...[...demoAbilities.abilities, ...demoAbilities.market].filter((x) => x.service_card?.id === card.id && Object.keys(x.places ?? {}).length).map((x) => ({ kind: x.id, label: x.name, verb: x.sources.length ? "Measures" : "Works on", fields: x.places ?? {}, needs: true })),
    ],
    used_by: [...demoAbilities.abilities, ...demoAbilities.market].filter((x) => x.service_card?.id === card.id).map((x) => ({ id: x.id, name: x.name, does: [] })),
    ability: ab?.id ?? "",
    ability_name: ab?.name ?? "",
    default: !demoAccounts.some((x) => x.service === card.id),
    channel: !!ab?.channels.length,
    kind: ab?.channels.length ? card.id : "",
    created_by: "user",
    created_at: ago(0),
    allow: roles.map((role) => ({ role, allow: chosen[role] ?? (["reply", "publish", "manage"].includes(role) ? "ask" : "auto"), acts: ["reply", "publish", "manage"].includes(role) })),
    places: Object.keys(place).length ? [{ id: `place-${n++}`, kind: card.id, label: String(Object.values(place)[0]), fields: place, default: true }] : [],
    place_fields: {},
    publications: 0,
    connection_name: ab?.name ?? "",
    ready: true,
    signed_in: true,
    missing: [],
    auth: (ab?.auth ?? []).map((k) => ({ ...k, set: true })),
    connect: ab?.connect ?? null,
    destination_text: "",
    service: card.id,
    service_card: card,
  };
  if (ab && demoPrograms(ab)) Object.assign(c, { has_session: true, session: { state: "unknown", message: "", checked_at: null, signed_in_at: null }, login: null, login_set: [], live_view: true, identity: String((body.secrets as Record<string, string> | undefined)?.SITE_URL ?? "") || undefined });
  demoAccounts.push(settleAccount(c));
  return c;
}
// Seemo suggests Search Console in the welcome flow; it opens from there to sign in.
demoAbilities.market.push({ ...demoAbilities.market[0], id: "search-console", name: "Read Google Search Console", author: "KIQR", summary: "Searches, clicks and positions for your site, read into Performance.", homepage: "https://search.google.com/search-console", categories: ["analytics"], icon: "search", cost: "free", provides: [], sources: ["search"], hosts: ["searchconsole.googleapis.com"], source: "marketplace", missing: [] });
// Google Analytics and Search Console are one service: one sign-in (a shared GOOGLE_TOKEN), and each works on a place of its own kind.
for (const a of demoAbilities.market.filter((x) => x.id === "google-analytics" || x.id === "search-console"))
  Object.assign(a, { service: "google", service_card: GOOGLE_CARD, allow: { measure: "auto" }, auth: [{ env: "GOOGLE_TOKEN", label: "Sign-in", help: "", optional: false, set: null }], connect: { label: "Sign in with Google", flow: "oauth2", needs: [], returns: ["GOOGLE_TOKEN"], https_only: false, via_cloud: true }, places: a.id === "google-analytics" ? GA_FIELDS : SC_FIELDS });

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
const chats: Chat[] = [
  { id: "demo-chat-1", title: "Write three social media posts for the library", created_at: ago(42), updated_at: ago(36), session_id: "demo", model: "claude-cli/sonnet", last_message: "The three LinkedIn drafts are in the library.", messages: messages.length },
  { id: "demo-chat-0", title: "What should we focus on this quarter?", created_at: ago(600), updated_at: ago(590), session_id: "demo", model: "claude-cli/sonnet", last_message: "Lead with the MCP server: it is the one thing competitors don't have.", messages: 2 },
];
const transcripts: Record<string, Message[]> = {
  "demo-chat-1": messages,
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

export function demoSeemoRequest<T>(path: string, init?: RequestInit): Promise<T> {
  const method = init?.method ?? "GET";
  const body = init?.body ? JSON.parse(String(init.body)) : {};
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
  // A fresh object each time, as from the daemon, so a reload after an install is seen.
  if (/\/abilities(\?|$)/.test(path)) return Promise.resolve({ ...demoAbilities, abilities: [...demoAbilities.abilities], market: [...demoAbilities.market] } as T);
  // Demo fixtures: the networks a demo project's abilities would declare.
  if (path.endsWith("/networks")) return Promise.resolve({ networks: [
    { kind: "linkedin", name: "LinkedIn", icon: "", network: true, installed: true, text_limit: 3000, image: { width: 1200, height: 627 }, preview: "feed" },
    { kind: "x", name: "X", icon: "", network: true, installed: true, text_limit: 280, image: { width: 1600, height: 900 }, preview: "feed" },
    { kind: "instagram", name: "Instagram", icon: "", network: true, installed: true, text_limit: 2200, image: { width: 1080, height: 1080 }, preview: "feed" },
    { kind: "email", name: "Email", icon: "mail", network: false, installed: true },
  ] } as T);
  if (/\/suggestions\//.test(path)) return Promise.resolve({ dismissed: path.split("/").slice(-2, -1)[0] } as T);
  if (/\/connect\//.test(path)) {
    // A sign-in adds the account as it starts, as the daemon does.
    const ab = [...demoAbilities.abilities, ...demoAbilities.market].find((x) => path.endsWith(`/connect/${x.id}`));
    if (body.connection) return Promise.resolve({ url: "about:blank", callback: true, connection: body.connection, fields: [] } as T);
    const c = demoNewAccount(ab, body);
    return Promise.resolve({ url: "about:blank", callback: true, connection: c.id, fields: [] } as T);
  }
  // Signing in on KIQR's browser: the demo has no browser, so a sign-in always needs a hand, and the live view waits for its first picture.
  const sessM = /\/connections\/([^/]+)\/(signin|signout|session|hands)$/.exec(path);
  if (sessM) {
    const c = demoAccounts.find((x) => x.id === sessM[1]);
    if (!c) return Promise.reject(new Error("no such account"));
    const now = new Date().toISOString();
    if (sessM[2] === "signin") {
      c.session = { state: "needs_hand", message: c.service === "browser" ? "signed out; sign in once in the live view" : "the site asks for a person: verify you are human", checked_at: now, signed_in_at: c.session?.signed_in_at ?? null };
      return Promise.resolve({ started: true } as T);
    }
    if (sessM[2] === "signout") {
      c.session = { state: "signed_out", message: "signed out", checked_at: now, signed_in_at: null };
      return Promise.resolve({ session: c.session } as T);
    }
    if (sessM[2] === "hands") return Promise.resolve({ hands: `demo-${c.id}`, token: "demo", can: { click: true, key: true, type: true, navigate: true } } as T);
    return Promise.resolve({ has_session: !!c.has_session, session: c.session, login: c.login ?? null, live_view: !!c.live_view } as T);
  }
  if (/\/hands\/[^/]+\/(input|end)$/.test(path)) return Promise.resolve((path.endsWith("/end") ? { ended: true } : { ok: true }) as T);
  const progM = /^\/api\/abilities\/([^/]+)\/programs$/.exec(path);
  if (progM) {
    const a = demoAbilities.abilities.find((x) => x.id === progM[1]);
    if (a?.programs) a.programs = a.programs.map((p) => ({ ...p, found: true }));
    if (a) a.program_found = true;
    return Promise.resolve({ id: progM[1], programs: a?.programs ?? [] } as T);
  }
  const loginSecretM = /\/connections\/([^/]+)\/secrets\/(LOGIN_[A-Z_]+)$/.exec(path);
  if (loginSecretM) {
    const c = demoAccounts.find((x) => x.id === loginSecretM[1]);
    if (c) c.login_set = [...(c.login_set ?? []).filter((k) => k !== loginSecretM[2]), ...(body.value ? [loginSecretM[2]] : [])];
    return Promise.resolve({ name: loginSecretM[2], set: !!body.value } as T);
  }
  if (/\/secrets\//.test(path)) return Promise.resolve({ name: path.split("/").pop(), set: !!body.value } as T);
  if (/\/connections(\?|$)/.test(path)) {
    if (method === "POST") {
      const ab = [...demoAbilities.abilities, ...demoAbilities.market].find((x) => x.id === body.ability);
      return Promise.resolve(demoNewAccount(ab, body) as T);
    }
    const ability = /ability=([^&]+)/.exec(path)?.[1];
    return Promise.resolve({ connections: demoAccounts.filter((c) => !ability || c.ability === decodeURIComponent(ability)) } as T);
  }
  if (/\/connections\/[^/]+\/(default|secrets)/.test(path)) {
    const id = path.split("/")[5];
    if (path.endsWith("/default")) {
      const c = demoAccounts.find((x) => x.id === id);
      if (c) demoAccounts.filter((x) => x.service === c.service).forEach((x) => (x.default = x.id === id));
      return Promise.resolve(c as T);
    }
    return Promise.resolve({ set: true } as T);
  }
  const allowM = /\/connections\/([^/]+)\/allow$/.exec(path);
  if (allowM) {
    const c = demoAccounts.find((x) => x.id === allowM[1]);
    if (c) for (const [role, v] of Object.entries(body as Record<string, Allow>)) c.allow = (c.allow ?? []).map((p) => (p.role === role ? { ...p, allow: v } : p));
    return Promise.resolve(settleAccount(c!) as T);
  }
  const choicesM = /\/connections\/[^/]+\/choices\/([^/?]+)$/.exec(path);
  if (choicesM) return Promise.resolve({ name: choicesM[1], choices: demoChoices[choicesM[1]] ?? [] } as T);
  const placeM = /\/connections\/([^/]+)\/places(?:\/([^/]+))?$/.exec(path);
  if (placeM) {
    const c = demoAccounts.find((x) => x.id === placeM[1])!;
    c.places = c.places ?? [];
    if (method === "POST") {
      const list = demoLists.find((l) => l.id === body.list);
      const place: Place = list ? { id: `list-${list.id}`, kind: "list", label: body.label || list.label, fields: { list: list.id } } : { id: `place-${n++}`, kind: body.kind || c.kind, label: body.label || String(Object.values(body.fields ?? {})[0] ?? c.label), fields: body.fields ?? {} };
      if (!c.places.some((p) => p.kind === place.kind) || body.default) c.places.forEach((p) => p.kind === place.kind && (p.default = false)), (place.default = true);
      c.places.push(place);
      return Promise.resolve({ ...settleAccount(c), place } as T);
    }
    if (method === "DELETE") c.places = c.places.filter((p) => p.id !== decodeURIComponent(placeM[2]));
    if (method === "PUT") c.places = c.places.map((p) => (p.id === decodeURIComponent(placeM[2]) ? { ...p, label: body.label || p.label, default: body.default || p.default } : body.default ? { ...p, default: false } : p));
    return Promise.resolve(settleAccount(c) as T);
  }
  const connM = /\/connections\/([^/?]+)$/.exec(path);
  if (connM) {
    const i = demoAccounts.findIndex((c) => c.id === connM[1]);
    const c = demoAccounts[i];
    if (method === "DELETE" && i >= 0) demoAccounts.splice(i, 1);
    if (method === "PUT" && c) {
      if (typeof body.label === "string" && body.label.trim()) c.label = body.label.trim();
      if (typeof body.publisher === "string") Object.assign(c, { publisher: body.publisher, publisher_name: c.publishers?.find((p) => p.id === body.publisher)?.name ?? null });
      if (body.allow) for (const [role, v] of Object.entries(body.allow as Record<string, Allow>)) c.allow = (c.allow ?? []).map((p) => (p.role === role ? { ...p, allow: v } : p));
      if (body.login) c.login = body.login.store ? body.login : null;
      settleAccount(c);
    }
    return Promise.resolve((method === "DELETE" ? undefined : c) as T);
  }
  if (path.endsWith("/channels")) return Promise.resolve({ channels: demoAccounts.filter((c) => c.publishes), kinds: demoKinds } as T);
  const listM = /\/lists(?:\/([^/?]+))?(?:\/(contacts|import|unsubscribe|sends))?$/.exec(path);
  if (listM) {
    const [, id, sub] = listM;
    if (!id) {
      if (method === "POST") {
        const l: DemoList = { id: String(body.label).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, ""), label: body.label, description: body.description ?? "", contacts: 0, unsubscribed: 0, created_at: ago(0), people: [] };
        demoLists.push(l);
        return Promise.resolve(listView(l) as T);
      }
      return Promise.resolve({ lists: demoLists.map(listView) } as T);
    }
    const l = demoLists.find((x) => x.id === decodeURIComponent(id));
    if (!l) return Promise.reject(new Error(`no list '${id}'`));
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
    if (method === "DELETE") {
      demoLists.splice(demoLists.indexOf(l), 1);
      for (const c of demoAccounts) c.places = c.places?.filter((p) => p.fields?.list !== l.id);
      return Promise.resolve(undefined as T);
    }
    if (method === "PUT") Object.assign(l, { label: body.label || l.label, description: body.description ?? l.description });
    return Promise.resolve(listView(l) as T);
  }
  if (/\/publish$/.test(path)) return Promise.resolve({ published: true, url: null, detail: "sent", channel_label: "Me by email" } as T);
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
  if (/\/connections\/[^/]+\/activity/.test(path)) return Promise.resolve({ ...demoActivity, activity: path.includes("/company-page/") ? demoActivity.activity : [], today: path.includes("/company-page/") ? 3 : 0, daily_cap: path.includes("/company-page/") ? 20 : null } as T);
  if (/\/approvals\//.test(path)) return Promise.resolve({ id: "a", status: body.decision === "approve" ? "approved" : "declined", result: {} } as T);
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
      const c = demoAccounts.find((c) => c.id === a.channel);
      const place = c?.places?.find((p) => p.id === a.place) ?? null;
      items.push({ kind: "publish", asset: { type: a.type, id: a.id, title: a.title }, channel: a.channel, channel_label: c?.label ?? "", place: place?.id ?? null, place_label: place?.label ?? null, title: a.title, at: a.publish_at!, schedule_text: `to ${c?.label ?? a.channel}` });
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
        "kiqr.published.channel": [{ key: "me-by-email", value: 5 }],
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
      approvals: [],
      drafts: all.filter((a) => a.status === "draft"),
      ready: all.filter((a) => a.status === "ready"),
      scheduled: all.filter((a) => a.status === "scheduled").sort((x, y) => (x.publish_at ?? "").localeCompare(y.publish_at ?? "")),
      attention: all.filter((a) => !!a.publish_error),
      accounts_needing_hand: demoAccounts.filter((c) => c.session?.state === "needs_hand").map((c) => ({ connection: c.id, label: c.label, service: c.service, message: c.session!.message, since: c.session!.checked_at })),
      count: 0,
    };
    todo.count = todo.drafts.length + todo.attention.length + (todo.accounts_needing_hand?.length ?? 0);
    return Promise.resolve(todo as T);
  }
  return Promise.resolve({} as T);
}
