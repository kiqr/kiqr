// Demo mode for Seemo: a saved conversation, a task, and a scripted reply
// when you send something, so the chat screens work without a daemon.
import type { Ability, AbilitiesView, CalendarItem, Channel, Chat, ChatView, KiqrEvent, Message, Run, Task, Todo } from "./api";
import { demoAssetList } from "./demo-assets";

const slug = "unblockingapi-com";
const ago = (min: number) => new Date(Date.now() - min * 60_000).toISOString();
const DEMO_IMAGE =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif"><rect width="1600" height="900" fill="#0B1220"/><rect x="128" y="128" width="12" height="644" fill="#ef2f72"/><text x="188" y="330" fill="#FFFFFF" font-size="88" font-weight="700">One request.</text><text x="188" y="436" fill="#FFFFFF" font-size="88" font-weight="700">One credit.</text><text x="188" y="540" fill="#CBD5E1" font-size="36">No multipliers. Failed requests are free.</text><text x="188" y="744" fill="#ef2f72" font-size="30" font-weight="600">UnblockingAPI</text></svg>`,
  );

const demoChannels: Channel[] = [
  { id: "me-by-email", label: "Me by email", extension: "kiqr-smtp", kind: "email", destination: { to: "you@example.com" }, created_by: "user", created_at: ago(300), asks_first: true, publications: 1, last_published: ago(60), connection_name: "Email (SMTP)", ready: true, destination_text: "you@example.com" },
];
const demoAbilities: AbilitiesView = {"abilities": [{"id": "write", "name": "Write articles, posts and emails", "author": "kiqr", "author_url": "https://github.com/kiqr", "summary": "Seemo writes in the brand's tone of voice from the business brief; everything lands in the library as a draft.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "pen-tool", "cost": "model usage", "provides": [], "channels": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "business-profile", "name": "Crawl a website and build its business profile", "author": "kiqr", "author_url": "https://github.com/kiqr", "summary": "Crawl the site, extract what it says, and write the profile, the brand and the business brief Seemo works from.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["web", "writing"], "icon": "brain", "cost": "model usage", "provides": [], "channels": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "crawl-site", "label": "Reading the site", "project_copy": false, "machine_copy": false}, {"file": "extract-facts", "label": "Extracting facts", "project_copy": false, "machine_copy": false}, {"file": "build-profile", "label": "Building the profile", "project_copy": false, "machine_copy": false}, {"file": "business-brief", "label": "Writing the brief", "project_copy": false, "machine_copy": false}], "in_prompt": false, "error": null}, {"id": "read-web", "name": "Read the web", "author": "kiqr", "author_url": "https://github.com/kiqr", "summary": "Seemo reads one page at a time through KIQR's own fetcher.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["web"], "icon": "globe", "cost": "free", "provides": [], "channels": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "unblockingapi", "name": "Read blocked pages with UnblockingAPI", "author": "kiqr", "author_url": "https://github.com/kiqr", "summary": "A real browser and residential proxies for the pages KIQR could not read. One credit per success.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "https://unblockingapi.com", "categories": ["web"], "icon": "shield-check", "cost": "about $0.001 per page", "provides": [], "channels": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [{"env": "UNBLOCKINGAPI_KEY", "label": "UnblockingAPI key", "help": "https://unblockingapi.com", "optional": false, "set": "global"}], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "social-posts", "name": "Write social media posts", "author": "kiqr", "author_url": "https://github.com/kiqr", "summary": "Ready-to-publish posts in the brand's tone of voice, each with its image.", "description": "Write a few ready-to-publish social media posts from the business brief, in the brand's tone of voice.", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing", "publishing"], "icon": "message-square-text", "cost": "model usage", "provides": [], "channels": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "social-posts", "label": "Writing the posts", "project_copy": false, "machine_copy": false}], "in_prompt": true, "error": null}, {"id": "tasks", "name": "Run tasks on a schedule", "author": "kiqr", "author_url": "https://github.com/kiqr", "summary": "Seemo works later or repeatedly; every task is approved by you first.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "list-checks", "cost": "model usage", "provides": [], "channels": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [], "in_prompt": true, "error": null}, {"id": "kiqr-render", "name": "Create images with the built-in renderer", "author": "kiqr", "author_url": "https://github.com/kiqr", "summary": "On-brand images Seemo designs and KIQR renders here. Free, on from the start.", "description": "How to make an on-brand image for a post, an article cover or a standalone visual with the built-in renderer, or with a generator when the user wants a photo or an illustration.", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["images"], "icon": "image", "cost": "free", "provides": [{"ability": "image", "label": "Make images"}], "channels": [], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "shipped", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": "~/.kiqr/abilities/make-images", "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": ["image"], "instructions": [{"file": "make-images", "label": "Create images with the built-in renderer", "project_copy": false, "machine_copy": false}], "in_prompt": true, "error": null}, {"id": "kiqr-smtp", "name": "Send email from your mail account", "author": "kiqr", "author_url": "https://github.com/kiqr", "summary": "Email over SMTP. Each project connects its own account. Every send asks you first.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["email", "publishing"], "icon": "mail", "cost": "free", "provides": [{"ability": "email", "label": "Send email"}], "channels": ["Email"], "tools": 0, "trust": "official", "transport": "builtin", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "shipped", "repo": null, "core": false, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": "~/.kiqr/abilities/email-smtp", "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [{"env": "SMTP_HOST", "label": "Mail server", "help": "Gmail: smtp.gmail.com", "optional": false, "set": "project"}, {"env": "SMTP_USER", "label": "User name", "help": "Usually your email address", "optional": false, "set": "project"}, {"env": "SMTP_PASSWORD", "label": "Password or app password", "help": "https://myaccount.google.com/apppasswords", "optional": true, "set": "project"}], "active_for": ["email"], "instructions": [], "in_prompt": false, "error": null}, {"id": "product-update-posts", "name": "Write product update posts", "author": "you", "author_url": null, "summary": "When asked for a post about something we shipped.", "description": "When asked for a post about something we shipped.", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "book-open", "cost": "model usage", "provides": [], "channels": [], "tools": 0, "trust": "local", "transport": "instructions", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "yours", "repo": null, "core": false, "builtin": false, "installed": true, "scope": {"scope": "project", "project": "unblockingapi-com"}, "folder": "~/.kiqr/projects/unblockingapi-com/abilities/product-update-posts", "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "product-update-posts", "label": "Write product update posts", "project_copy": true, "machine_copy": false}], "in_prompt": true, "error": null}], "market": [{"id": "fal-image", "name": "Create images with fal.ai", "author": "kiqr", "author_url": "https://github.com/kiqr", "summary": "Photos and illustrations from a text prompt.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "https://fal.ai", "categories": ["images"], "icon": "image", "cost": "about $0.03 per image", "provides": [{"ability": "image", "label": "Make images"}], "channels": [], "tools": 0, "trust": "official", "transport": "http", "runs_program": false, "command": null, "program_found": null, "hosts": ["fal.run", "fal.media"], "source": "shipped", "repo": null, "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": ["FAL_KEY"], "auth": [{"env": "FAL_KEY", "label": "fal.ai API key", "help": "https://fal.ai/dashboard/keys", "optional": false, "set": null}], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "openai-image", "name": "Create images with OpenAI", "author": "kiqr", "author_url": "https://github.com/kiqr", "summary": "gpt-image-1 behind your OpenAI key.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "https://platform.openai.com", "categories": ["images"], "icon": "image", "cost": "about $0.04 per image", "provides": [{"ability": "image", "label": "Make images"}], "channels": [], "tools": 0, "trust": "official", "transport": "http", "runs_program": false, "command": null, "program_found": null, "hosts": ["api.openai.com"], "source": "shipped", "repo": null, "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": ["OPENAI_API_KEY"], "auth": [{"env": "OPENAI_API_KEY", "label": "OpenAI API key", "help": "https://platform.openai.com/api-keys", "optional": false, "set": null}], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "playwright-browser", "name": "Drive a browser with Playwright", "author": "kiqr-cloud", "author_url": "https://github.com/kiqr-cloud", "summary": "A real browser Seemo can drive: sites without an API, screenshots, logged-in sessions.", "description": "", "version": "0.2.0", "latest": "0.2.0", "homepage": "https://github.com/microsoft/playwright-mcp", "categories": ["browser", "tools", "publishing"], "icon": "", "cost": "free", "provides": [], "channels": [], "tools": 4, "trust": "verified", "transport": "mcp", "runs_program": true, "command": "npx @playwright/mcp@latest --headless", "program_found": null, "hosts": [], "source": "marketplace", "repo": "https://github.com/kiqr-cloud/ability-playwright.git", "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": [], "auth": [], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "fal-video", "name": "Create video with fal.ai", "author": "kiqr-cloud", "author_url": "https://github.com/kiqr-cloud", "summary": "Short clips from a prompt. Slow and paid; Seemo says what it costs first.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "https://fal.ai", "categories": ["video"], "icon": "", "cost": "about $0.50 per clip", "provides": [{"ability": "video", "label": "Make video"}], "channels": [], "tools": 0, "trust": "official", "transport": "http", "runs_program": false, "command": null, "program_found": null, "hosts": ["fal.run"], "source": "marketplace", "repo": "https://github.com/kiqr-cloud/ability-fal-video.git", "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": ["FAL_KEY"], "auth": [{"env": "FAL_KEY", "label": "fal.ai API key", "help": "https://fal.ai/dashboard/keys", "optional": false, "set": null}], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "wordpress", "name": "Publish articles to WordPress", "author": "acme-digital", "author_url": "https://github.com/acme-digital", "summary": "Publish articles to your WordPress site as drafts or posts.", "description": "", "version": "0.3.1", "latest": "0.3.1", "homepage": "https://wordpress.org", "categories": ["publishing"], "icon": "", "cost": "free", "provides": [], "channels": ["WordPress blog"], "tools": 1, "trust": "community", "transport": "http", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "marketplace", "repo": "https://github.com/acme-digital/kiqr-wordpress.git", "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": ["WP_USER", "WP_APP_PASSWORD"], "auth": [{"env": "WP_USER", "label": "User name", "help": "", "optional": false, "set": null}, {"env": "WP_APP_PASSWORD", "label": "Application password", "help": "", "optional": false, "set": null}], "active_for": [], "instructions": [], "in_prompt": false, "error": null}, {"id": "weekly-report", "name": "Write the Monday report", "author": "acme-digital", "author_url": "https://github.com/acme-digital", "summary": "What was added, what ran, what to write next: a short report every Monday, in plain words.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "book-open", "cost": "model usage", "provides": [], "channels": [], "tools": 0, "trust": "community", "transport": "instructions", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "marketplace", "repo": "https://github.com/acme-digital/kiqr-abilities.git", "core": false, "builtin": false, "installed": false, "scope": null, "folder": null, "git": false, "machine_off": false, "paused": false, "ready": false, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "weekly-report", "label": "Write the Monday report", "project_copy": false, "machine_copy": false}], "in_prompt": false, "error": null}], "contracts": [{"id": "image", "label": "Make images", "tool": "create_image", "configured": null, "active": {"id": "kiqr-render", "name": "Create images with the built-in renderer"}, "candidates": [{"id": "kiqr-render", "name": "Create images with the built-in renderer", "ready": true}]}, {"id": "video", "label": "Make video", "tool": "create_video", "configured": null, "active": null, "candidates": []}, {"id": "music", "label": "Make music", "tool": "create_music", "configured": null, "active": null, "candidates": []}, {"id": "email", "label": "Send email", "tool": "send_email", "configured": null, "active": {"id": "kiqr-smtp", "name": "Send email from your mail account"}, "candidates": [{"id": "kiqr-smtp", "name": "Send email from your mail account", "ready": true}]}], "broken": [], "guide": {"id": "seemo-agent", "name": "Seemo's own guide", "author": "kiqr", "author_url": "https://github.com/kiqr", "summary": "How Seemo works inside KIQR: the tools, when to act and when to make a task, how to ask for a yes, how to report.", "description": "", "version": "0.1.0", "latest": "0.1.0", "homepage": "", "categories": ["writing"], "icon": "book-open", "cost": "model usage", "provides": [], "channels": [], "tools": 0, "trust": "official", "transport": "instructions", "runs_program": false, "command": null, "program_found": null, "hosts": [], "source": "core", "repo": null, "core": true, "builtin": true, "installed": true, "scope": {"scope": "global"}, "folder": "~/.kiqr/abilities/seemo-agent", "git": false, "machine_off": false, "paused": false, "ready": true, "missing": [], "auth": [], "active_for": [], "instructions": [{"file": "seemo-agent", "label": "How Seemo works inside KIQR", "project_copy": false, "machine_copy": false}], "in_prompt": false, "error": null}, "index": "https://kiqr.cloud/marketplace/index.json", "env_file": "~/.kiqr/.env", "project_env_file": "~/.kiqr/projects/unblockingapi-com/.env"};
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
  if (/\/abilities\/[^/]+\/(install|update)$/.test(path) || (/^\/api\/abilities\/[^/?]+(\?|$)/.test(path) && method === "DELETE")) {
    const id = path.split("/")[3].split("?")[0];
    if (path.endsWith("/install")) {
      const i = demoAbilities.market.findIndex((x) => x.id === id);
      if (i >= 0) {
        const given = Object.keys(body.secrets ?? {}).length > 0;
        const a: Ability = { ...demoAbilities.market[i], installed: true, ready: demoAbilities.market[i].missing.length === 0 || given, missing: given ? [] : demoAbilities.market[i].missing, scope: { scope: "global" }, folder: `~/.kiqr/abilities/${id}` };
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
  if (/\/abilities(\?|$)/.test(path)) return Promise.resolve(demoAbilities as T);
  if (/\/secrets\//.test(path)) return Promise.resolve({ name: path.split("/").pop(), set: !!body.value } as T);
  if (path.endsWith("/channels")) {
    if (method === "POST") {
      const c: Channel = { id: `channel-${n++}`, label: body.label || `Email to ${body.destination?.to ?? ""}`, extension: "kiqr-smtp", kind: body.kind, destination: body.destination ?? {}, created_by: "user", created_at: new Date().toISOString(), asks_first: true, publications: 0, connection_name: "Email (SMTP)", ready: true, destination_text: String(body.destination?.to ?? "") };
      demoChannels.push(c);
      return Promise.resolve(c as T);
    }
    return Promise.resolve({ channels: demoChannels, kinds: [{ kind: "email", label: "Email", connection: "kiqr-smtp", connection_name: "Email (SMTP)", ready: true, destination: { to: { type: "string", label: "To", help: "One address, or several separated by commas" }, subject: { type: "string", label: "Subject", help: "Leave empty to use the asset's title", optional: true } } }] } as T);
  }
  const chM = /\/channels\/([^/]+)$/.exec(path);
  if (chM) {
    const i = demoChannels.findIndex((c) => c.id === chM[1]);
    if (method === "DELETE" && i >= 0) demoChannels.splice(i, 1);
    return Promise.resolve((method === "DELETE" ? undefined : demoChannels[i]) as T);
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
  const chPut = /\/channels\/([^/?]+)$/.exec(path);
  if (chPut && method === "PUT") {
    const c = demoChannels.find((x) => x.id === chPut[1]);
    if (c && typeof body.asks_first === "boolean") c.asks_first = body.asks_first;
    return Promise.resolve(c as T);
  }
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
      const c = demoChannels.find((c) => c.id === a.channel);
      items.push({ kind: "publish", asset: { type: a.type, id: a.id, title: a.title }, channel: a.channel, channel_label: c?.label ?? "", title: a.title, at: a.publish_at!, schedule_text: `to ${c?.label ?? a.channel}` });
    }
    items.sort((x, y) => x.at.localeCompare(y.at));
    return Promise.resolve(items as T);
  }
  if (path.endsWith("/todo")) {
    const all = demoAssetList();
    const todo: Todo = {
      approvals: [],
      drafts: all.filter((a) => a.status === "draft"),
      ready: all.filter((a) => a.status === "ready"),
      scheduled: all.filter((a) => a.status === "scheduled").sort((x, y) => (x.publish_at ?? "").localeCompare(y.publish_at ?? "")),
      attention: all.filter((a) => !!a.publish_error),
      count: 0,
    };
    todo.count = todo.drafts.length + todo.attention.length;
    return Promise.resolve(todo as T);
  }
  return Promise.resolve({} as T);
}
