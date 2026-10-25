// The library the demo shows for unblockingapi.com: a few things Seemo has made from the brief.
import type { Asset, AssetType, NewAsset } from "./api";
import { demoBrand } from "./demo-brief";

const slot = (role: string, ...accept: string[]) => ({ role, accept });

export const demoTypes: AssetType[] = [
  { id: "article", name: "Article", plural: "Articles", icon: "newspaper", description: "Long-form pieces for your blog, written from the business profile and the content gaps.", preview: "article", files: [slot("body", "md"), slot("cover", "png", "jpg", "webp")], image: "always", builtin: true },
  { id: "post", name: "Post", plural: "Posts", icon: "message-square-text", description: "Social posts and threads, in the brand's tone of voice.", preview: "post", files: [slot("body", "md"), slot("media", "png", "mp4")], image: "usually", builtin: true },
  { id: "reply", name: "Reply", plural: "Replies", icon: "reply", description: "Replies to conversations on Reddit, forums and comment threads.", preview: "post", files: [slot("body", "md")], image: "none", builtin: true },
  { id: "image", name: "Image", plural: "Images", icon: "image", description: "Illustrations, banners and graphics.", preview: "image", files: [slot("image", "png", "webp")], image: "always", builtin: true },
  { id: "meme", name: "Meme", plural: "Memes", icon: "laugh", description: "Memes and other quick, shareable images.", preview: "image", files: [slot("image", "png"), slot("caption", "md")], image: "always", builtin: true },
  { id: "video", name: "Video", plural: "Videos", icon: "clapperboard", description: "Videos and reels, made with video APIs.", preview: "video", files: [slot("video", "mp4"), slot("script", "md")], image: "none", builtin: true },
  { id: "audio", name: "Audio", plural: "Audio", icon: "audio-lines", description: "Songs, jingles, voice-overs and podcast clips.", preview: "audio", files: [slot("audio", "mp3")], image: "none", builtin: true },
  { id: "ad-campaign", name: "Ad campaign", plural: "Ad campaigns", icon: "megaphone", description: "Ad campaigns with ad groups, keywords and copy, for Google Ads, Meta and others.", preview: "campaign", files: [slot("campaign", "yaml")], image: "none", builtin: true },
  { id: "email", name: "Email", plural: "Emails", icon: "mail", description: "Newsletters and email campaigns.", preview: "article", files: [slot("body", "md")], image: "none", builtin: true },
];

const ago = (h: number) => new Date(Date.now() - h * 36e5).toISOString();
const file = (role: string, path: string, bytes = 2048) => ({ role, path, bytes, sha256: "" });
const img = (i: number) => demoBrand.images[i % demoBrand.images.length].url;

const texts: Record<string, string> = {
  "article/scrape-idealista-without-getting-blocked/article.md":
    "# How to scrape Idealista without getting blocked\n\nIdealista answers automated requests with a challenge page within a few hundred requests. Here is what works in 2026, and what it costs.\n\n## Why plain requests fail\n\nThe site fingerprints TLS and checks that a real browser ran its scripts…",
  "article/residential-vs-datacenter-proxies/article.md":
    "# Residential or datacenter proxies?\n\nDatacenter IPs are cheap and fast, and most large sites block them on sight. Residential IPs cost more and pass. The choice depends on the site, not the budget…",
  "post/templates-launch-thread/post.md":
    "We just shipped 40 templates.\n\nPoint UnblockingAPI at a Reddit post, an Idealista search or a Hetzner auction and get clean JSON back. No selectors to maintain.\n\nunblockingapi.com/templates",
  "post/one-credit-per-success/post.md": "Failures are free. Every successful fetch costs one credit, rendered or not.\n\nThat's the whole pricing page.",
  "post/linkedin-render-reason/post.md":
    "Most scraping APIs make you guess when to render a page in a real browser.\n\nWe tell you why we did: blocked, challenge wall or JavaScript shell. Learned per site, so the next fetch is faster.",
  "reply/r-webscraping-cloudflare/reply.md":
    "If it's Cloudflare's managed challenge, rotating datacenter proxies won't help; it scores the browser, not just the IP. A real browser with residential IPs gets through. (Disclosure: I work on UnblockingAPI, which does exactly that, but Playwright plus a residential pool works too.)",
  "meme/its-a-javascript-shell/caption.md": "When the page is 200 OK and 4 KB of `<div id=\"root\"></div>`",
};

let assets: Asset[] = [
  {
    id: "scrape-idealista-without-getting-blocked",
    type: "article",
    title: "How to scrape Idealista without getting blocked",
    status: "ready",
    tags: ["templates", "real-estate"],
    summary: "From the content gap 'Scraping property portals'. 1,640 words, with the Idealista template as the worked example.",
    created: { at: ago(30), by: "chat:demo" },
    updated_at: ago(2),
    files: [file("body", "article.md", 9800), file("cover", "cover.png", 182000)],
    source: { gap: "Scraping property portals" },
    publications: [],
  },
  {
    id: "templates-launch-thread",
    type: "post",
    title: "Templates launch thread",
    status: "published",
    tags: ["x", "launch"],
    summary: "Three-post thread for X.",
    created: { at: ago(80), by: "user" },
    updated_at: ago(70),
    files: [file("body", "post.md", 380), file("media", "card.png", 120000)],
    publications: [{ connector: "x", account: "@unblockingapi", url: "https://x.com/unblockingapi", at: ago(70) }],
  },
  {
    id: "its-a-javascript-shell",
    type: "meme",
    title: "It's a JavaScript shell",
    status: "scheduled",
    tags: ["x", "humour"],
    created: { at: ago(20), by: "task:weekly-meme" },
    updated_at: ago(20),
    files: [file("image", "meme.png", 240000), file("caption", "caption.md", 80)],
    publications: [],
  },
  {
    id: "r-webscraping-cloudflare",
    type: "reply",
    title: "r/webscraping: 'Cloudflare blocks my scraper after 50 requests'",
    status: "published",
    tags: ["reddit"],
    created: { at: ago(26), by: "task:reddit-replies" },
    updated_at: ago(25),
    files: [file("body", "reply.md", 420)],
    publications: [{ connector: "reddit", account: "u/unblockingapi", url: "https://www.reddit.com/r/webscraping/", at: ago(25), run: "reddit-replies/2026-10-22T10-00" }],
  },
  {
    id: "residential-vs-datacenter-proxies",
    type: "article",
    title: "Residential or datacenter proxies?",
    status: "draft",
    tags: ["proxies"],
    summary: "Outline and first two sections.",
    created: { at: ago(6), by: "chat:demo" },
    updated_at: ago(1),
    files: [file("body", "article.md", 3100)],
    source: { gap: "Choosing a proxy type" },
    publications: [],
  },
  {
    id: "one-credit-per-success",
    type: "post",
    title: "Failures are free",
    status: "ready",
    tags: ["x", "pricing"],
    created: { at: ago(12), by: "user" },
    updated_at: ago(12),
    files: [file("body", "post.md", 120)],
    publications: [],
  },
  {
    id: "linkedin-render-reason",
    type: "post",
    title: "Why we tell you when we render",
    status: "draft",
    tags: ["linkedin"],
    created: { at: ago(4), by: "user" },
    updated_at: ago(4),
    files: [file("body", "post.md", 260)],
    publications: [],
  },
  {
    id: "template-cards",
    type: "image",
    title: "Template cards for the docs",
    status: "ready",
    tags: ["templates"],
    created: { at: ago(50), by: "user" },
    updated_at: ago(48),
    files: [file("image", "card.png", 140000)],
    publications: [],
  },
  {
    id: "q4-scraping-api-search",
    type: "ad-campaign",
    title: "Q4 search: scraping API",
    status: "draft",
    tags: ["google-ads"],
    summary: "3 ad groups, 42 keywords, 9 responsive search ads.",
    created: { at: ago(9), by: "chat:demo" },
    updated_at: ago(9),
    files: [file("campaign", "campaign.yaml", 5400)],
    publications: [],
  },
];

// `?demo&empty` shows the empty library.
if (typeof location !== "undefined" && /[?&]empty\b/.test(location.search)) assets = [];

const images: Record<string, string> = {
  "article/scrape-idealista-without-getting-blocked/cover.png": img(1),
  "post/templates-launch-thread/card.png": img(2),
  "meme/its-a-javascript-shell/meme.png": img(0),
  "image/template-cards/card.png": img(3),
};

export function demoAssetFile(a: Pick<Asset, "type" | "id">, name: string) {
  return images[`${a.type}/${a.id}/${name}`] ?? "";
}

export function demoAssetText(a: Pick<Asset, "type" | "id">, name: string) {
  return texts[`${a.type}/${a.id}/${name}`] ?? "";
}

export function demoAssetRequest<T>(path: string, init?: RequestInit): Promise<T> {
  const ok = (v: unknown) => Promise.resolve(v as T);
  if (path === "/api/asset-types") return ok(demoTypes);
  const [, rest = ""] = path.split("/assets");
  const [route, query = ""] = rest.split("?");
  if (route === "/summary") {
    const counts = new Map<string, number>();
    assets.forEach((a) => counts.set(a.type, (counts.get(a.type) ?? 0) + 1));
    return ok([...counts].map(([type, count]) => ({ type, count })).sort((a, b) => b.count - a.count));
  }
  if (route === "/generate") {
    const made: Asset[] = [
      ["Templates instead of selectors", "linkedin", "Most scraping breaks the day a site changes its markup.\n\nOur templates turn a page into JSON without selectors you maintain: a Reddit post, an Idealista search, a Hetzner auction. When the site changes, we update the template.\n\nunblockingapi.com/templates"],
      ["Failures cost nothing", "x", "A fetch that fails costs nothing. One that succeeds costs one credit, rendered in a real browser or not."],
      ["When we render, and why", "linkedin", "We start with a plain fetch and switch to a real browser only when a page is blocked, shows a challenge or is an empty JavaScript shell.\n\nThe response tells you which, and we remember it per site."],
    ].map(([title, network, text]) => {
      const id = title.toLowerCase().replace(/[^a-z0-9]+/g, "-");
      texts[`post/${id}/post.md`] = text;
      return { id, type: "post", title, status: "draft", tags: [network], summary: `For ${network === "x" ? "X" : "LinkedIn"}`, created: { at: ago(0), by: "seemo" }, updated_at: ago(0), files: [file("body", "post.md", text.length)], publications: [] };
    });
    assets = [...made, ...assets];
    return ok({ started: true });
  }
  if (route === "" && init?.method === "POST") {
    const n: NewAsset = JSON.parse(String(init.body));
    const a: Asset = { id: n.title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, ""), type: n.type, title: n.title, status: "draft", tags: n.tags ?? [], summary: n.summary, created: { at: ago(0), by: "user" }, updated_at: ago(0), files: [], source: n.source, publications: [] };
    assets = [a, ...assets];
    return ok(a);
  }
  if (route === "") {
    const type = new URLSearchParams(query).get("type");
    return ok([...assets].filter((a) => !type || a.type === type).sort((a, b) => b.updated_at.localeCompare(a.updated_at)));
  }
  const [, type, id, action] = route.split("/");
  if (init?.method === "DELETE" && !action) {
    assets = assets.filter((a) => !(a.type === type && a.id === id));
    return ok(undefined);
  }
  const found = assets.find((a) => a.type === type && a.id === id);
  if (!found) return Promise.reject(new Error("no asset"));
  // Status follows the facts, as in the daemon.
  const settle = (patch: Partial<Asset>): Asset => {
    const n: Asset = { ...found, ...patch, updated_at: ago(0) };
    n.status = n.archived ? "archived" : n.publications.length ? "published" : n.channel && n.publish_at ? "scheduled" : n.channel ? "ready" : "draft";
    assets = assets.map((a) => (a.type === type && a.id === id ? n : a));
    return n;
  };
  const body = init?.body ? JSON.parse(String(init.body)) : {};
  if (action === "approve") return ok(settle({ channel: body.channel, publish_error: null }));
  if (action === "schedule" && init?.method === "DELETE") return ok(settle({ publish_at: null, publish_error: null }));
  if (action === "schedule") return ok(settle({ channel: body.channel || found.channel, publish_at: body.at, publish_error: null }));
  if (action === "unpublish") return ok(settle({ publications: [] }));
  if (action === "publish") {
    const channel = body.channel || found.channel || "me-by-email";
    settle({ channel, publish_at: null, publish_error: null, publications: [...found.publications, { connector: channel, account: "you@example.com", at: ago(0) }] });
    return ok({ published: true, url: null, detail: "sent", channel_label: "Me by email" });
  }
  if (init?.method === "PATCH") return ok(settle(body));
  if (init?.method === "PUT") return ok(settle(JSON.parse(String(init.body))));
  return ok(found);
}

/** The demo's assets as they stand, for the To do page and the calendar. */
export function demoAssetList() {
  return assets;
}
