// Demo mode for Seemo: a saved conversation, a task, and a scripted reply
// when you send something, so the chat screens work without a daemon.
import type { CalendarItem, Channel, Chat, ChatView, Extensions, KiqrEvent, Message, Run, Skill, Task } from "./api";

const slug = "unblockingapi-com";
const ago = (min: number) => new Date(Date.now() - min * 60_000).toISOString();
const DEMO_IMAGE =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif"><rect width="1600" height="900" fill="#0B1220"/><rect x="128" y="128" width="12" height="644" fill="#ef2f72"/><text x="188" y="330" fill="#FFFFFF" font-size="88" font-weight="700">One request.</text><text x="188" y="436" fill="#FFFFFF" font-size="88" font-weight="700">One credit.</text><text x="188" y="540" fill="#CBD5E1" font-size="36">No multipliers. Failed requests are free.</text><text x="188" y="744" fill="#ef2f72" font-size="30" font-weight="600">UnblockingAPI</text></svg>`,
  );

const demoSkills: Skill[] = [
  { name: "product-updates", title: "Product update posts", description: "When asked for a post about something we shipped.", abilities: [], channels: [], scope: { scope: "project", project: slug }, builtin: false, has_skill: true, connection: null, error: null, folder: "~/.kiqr/projects/unblockingapi-com/skills/product-updates" },
  { name: "make-images", title: "make-images", description: "How to make an on-brand image for a post, an article cover or a standalone visual with the built-in renderer, or with a generator when the user wants a photo or an illustration.", abilities: ["image"], channels: [], scope: { scope: "global" }, builtin: true, has_skill: true, connection: { id: "kiqr-render", name: "Built-in renderer", ready: true }, error: null, folder: "~/.kiqr/skills/make-images" },
  { name: "social-posts", title: "social-posts", description: "Write a few ready-to-publish social media posts from the business brief, in the brand's tone of voice.", abilities: [], channels: [], scope: { scope: "global" }, builtin: true, has_skill: true, connection: null, error: null, folder: "~/.kiqr/skills/social-posts" },
  { name: "fal-image", title: "fal-image", description: "", abilities: [], channels: [], scope: { scope: "global" }, builtin: true, has_skill: false, connection: { id: "fal-image", name: "fal.ai images", ready: false }, error: null, folder: "~/.kiqr/skills/fal-image" },
];
const demoSkillText: Record<string, string> = {
  "product-updates": "---\nname: product-updates\ndescription: When asked for a post about something we shipped.\n---\n\n1. Lead with what changed for the customer, in one sentence.\n2. One concrete example or number.\n3. End with where to try it. No hype words.",
};
const demoChannels: Channel[] = [
  { id: "me-by-email", label: "Me by email", extension: "kiqr-smtp", kind: "email", destination: { to: "you@example.com" }, created_by: "user", created_at: ago(300), asks_first: true, publications: 1, last_published: ago(60), connection_name: "Email (SMTP)", ready: true, destination_text: "you@example.com" },
];
const demoExtensions: Extensions = {
  connections: [
    { id: "kiqr-render", name: "Built-in renderer", version: "0.1.0", summary: "On-brand images designed by Seemo and rendered here. Free, always available.", transport: "builtin", homepage: "", builtin: true, folder: "make-images", scope: { scope: "global" }, ready: true, missing: [], auth: [], provides: [{ ability: "image", label: "Make images", cost: "free" }], tools: [], channels: [], hosts: [], command: null, env: [], program_found: null },
    { id: "fal-image", name: "fal.ai images", version: "0.1.0", summary: "Photos and illustrations from a text prompt. About $0.03 per image.", transport: "http", homepage: "https://fal.ai", builtin: true, folder: "fal-image", scope: { scope: "global" }, ready: false, missing: ["FAL_KEY"], auth: [{ env: "FAL_KEY", label: "fal.ai API key", help: "https://fal.ai/dashboard/keys", optional: false, set: false }], provides: [{ ability: "image", label: "Make images", cost: "about $0.03 per image" }], tools: [], channels: [], hosts: ["fal.run", "fal.media"], command: null, env: [], program_found: null },
    { id: "kiqr-smtp", name: "Email (SMTP)", version: "0.1.0", summary: "Send email from your own mail account. Every send asks you first.", transport: "builtin", homepage: "", builtin: true, folder: "email-smtp", scope: { scope: "global" }, ready: false, missing: ["SMTP_HOST", "SMTP_USER", "SMTP_PASSWORD"], auth: [{ env: "SMTP_HOST", label: "Mail server", help: "Gmail: smtp.gmail.com", optional: false, set: false }, { env: "SMTP_USER", label: "User name", help: "Usually your email address", optional: false, set: false }, { env: "SMTP_PASSWORD", label: "Password or app password", help: "https://myaccount.google.com/apppasswords", optional: false, set: false }], provides: [{ ability: "email", label: "Send email", cost: "free" }], tools: [], channels: [{ kind: "email", label: "Email" }], hosts: [], command: null, env: [], program_found: null },
  ],
  broken: [],
  abilities: [
    { id: "image", label: "Make images", tool: "create_image", configured: null, active: { id: "kiqr-render", name: "Built-in renderer" }, connections: [{ id: "kiqr-render", name: "Built-in renderer", ready: true }, { id: "fal-image", name: "fal.ai images", ready: false }] },
    { id: "video", label: "Make video", tool: "create_video", configured: null, active: null, connections: [] },
    { id: "music", label: "Make music", tool: "create_music", configured: null, active: null, connections: [] },
    { id: "email", label: "Send email", tool: "send_email", configured: null, active: null, connections: [{ id: "kiqr-smtp", name: "Email (SMTP)", ready: false }] },
  ],
};

const task: Task = {
  id: "write-three-social-posts",
  title: "Write three social posts",
  instruction: "Read the social-posts skill, then the brief. Write three LinkedIn posts: flat pricing, the MCP server, the Visual Editor. Save each as a draft post asset and report what you made.",
  schedule: { kind: "once", at: null, rrule: null, tz: "Europe/Stockholm", text: "right away" },
  schedule_text: "once, right away",
  allowed: ["read", "write_assets"],
  allowed_labels: ["Read the profile, brief, brand, assets and tasks", "Create and edit assets"],
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
  summary: "I saved three LinkedIn posts to the library as drafts. I published nothing.\n\n- Flat pricing (post/flat-one-credit-pricing-explained)\n- MCP server (post/giving-ai-apps-a-real-browser)\n- Visual Editor (post/tap-a-field-get-json)",
  steps: [
    { tool: "read_skill", label: "Reading a skill: social-posts", ok: true },
    { tool: "get_section", label: "Reading a profile section: voice", ok: true },
    { tool: "create_asset", label: "Adding to the library", ok: true },
    { tool: "create_asset", label: "Adding to the library", ok: true },
    { tool: "create_asset", label: "Adding to the library", ok: true },
  ],
  assets: ["post/flat-one-credit-pricing-explained", "post/giving-ai-apps-a-real-browser", "post/tap-a-field-get-json"],
  session_id: null,
  cost_usd: 0.08,
};

const messages: Message[] = [
  { id: "m1", at: ago(42), role: "user", text: "Write three social media posts for the library as draft post assets, and tell me when they are in." },
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
      { tool: "read_skill", label: "Reading a skill: make-images", ok: true },
      { tool: "create_image", label: "Making an image", ok: true, image: DEMO_IMAGE },
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
  if (path.startsWith("/api/extensions")) return Promise.resolve(demoExtensions as T);
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
  const skillM = /\/skills(?:\/([^/]+))?$/.exec(path);
  if (skillM) {
    const name = skillM[1];
    if (!name) {
      if (method === "POST") return Promise.resolve(demoSkills[0] as T);
      return Promise.resolve(demoSkills as T);
    }
    if (method === "PUT") {
      const text = String(body.text ?? "");
      const d = /description:\s*(.*)/.exec(text);
      const existing = demoSkills.find((s) => s.name === name);
      if (existing) existing.description = d?.[1] ?? existing.description;
      else demoSkills.unshift({ name, title: name, description: d?.[1] ?? "", abilities: [], channels: [], scope: { scope: "project", project: slug }, builtin: false, has_skill: true, connection: null, error: null, folder: `~/.kiqr/projects/${slug}/skills/${name}` });
      demoSkillText[name] = text;
      return Promise.resolve(demoSkills.find((s) => s.name === name) as T);
    }
    if (method === "DELETE") {
      const i = demoSkills.findIndex((s) => s.name === name);
      if (i >= 0) demoSkills.splice(i, 1);
      return Promise.resolve(undefined as T);
    }
    return Promise.resolve((demoSkillText[name] ?? `---\nname: ${name}\ndescription: ${demoSkills.find((s) => s.name === name)?.description ?? ""}\n---\n\nBuilt-in skill text is not included in the demo.`) as T);
  }
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
      items.push({ task: weekly.id, title: weekly.title, at: new Date(d.getTime() + i * 7 * 86_400_000).toISOString(), schedule_text: weekly.schedule_text });
    }
    return Promise.resolve(items as T);
  }
  return Promise.resolve({} as T);
}
