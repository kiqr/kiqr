// The business brief the demo shows for unblockingapi.com, written from a real crawl of the site.
import type { Brand, Brief } from "./api";

const u = (p: string) => `https://unblockingapi.com${p}`;

export const demoBrief: Brief = {
  version: 2,
  source: "crawl",
  updated_at: new Date(Date.now() - 4 * 864e5).toISOString(),
  business_name: "UnblockingAPI",
  summary:
    "UnblockingAPI is a web unblocking API for developers: send a public URL and get clean HTML back, with real-browser rendering, rotating residential proxies, routing and retries handled. Every successful request costs one credit, whatever it took to load, and failed requests are free. Around the API sit a visual editor and a public library of scraper templates that return JSON, and a hosted MCP server that gives AI assistants like Claude and Cursor live web access. It is operated by WK Operations Ltd and serves customers worldwide.",
  language: "en",
  geographies: [],
  offerings: [
    { name: "Unblock API", kind: "product", description: "REST endpoint, POST /unblock, that fetches a public URL through real browsers and residential proxies and returns rendered HTML.", url: u("/docs"), sources: [u("/"), u("/docs")] },
    { name: "Scraper templates", kind: "product", description: "Ready-made scrapers that turn a known page type into JSON; versioned, forkable and callable as API endpoints.", url: u("/templates"), sources: [u("/templates")] },
    { name: "Visual editor", kind: "product", description: "Load a URL, tap the fields you want and publish a template without writing code.", url: u("/editor"), sources: [u("/editor")] },
    { name: "Hosted MCP server", kind: "product", description: "Gives Claude, Cursor, VS Code and other AI apps a real browser through three read-only tools, with OAuth sign-in.", url: u("/mcp"), sources: [u("/mcp")] },
    { name: "Developer setup services", kind: "service", description: "Hands-on help integrating the API, migrating scrapers and tuning difficult targets, quoted on request.", url: u("/contact"), sources: [u("/pricing"), u("/contact")] },
  ],
  audiences: [
    { name: "Developers building scrapers", needs: ["Pages that render client-side", "No proxy pools to maintain", "Predictable cost per request"] },
    { name: "AI agent and LLM app builders", needs: ["Live web access for agents", "Answers grounded in current pages", "MCP that works with existing apps"] },
    { name: "SEO and rank-tracking teams", needs: ["SERP data by country", "Daily checks at a fixed price"] },
    { name: "E-commerce and real estate analysts", needs: ["Price and listing monitoring", "Region-specific content", "Structured JSON instead of HTML"] },
  ],
  tone_of_voice: {
    description: "Plain, technical and direct. Short sentences that state what the product does and what it costs, with numbers instead of superlatives. Comparisons with competitors are specific and sourced.",
    traits: ["Plain", "Technical", "Direct", "Transparent", "Developer-first"],
    examples: ["One API call. Clean HTML back.", "One credit per successful request — rendering, premium proxies and retries included.", "Failed requests cost nothing."],
    avoid: ["Superlatives the product can't prove", "Promising access to logged-in or private pages", "Vague pricing language"],
  },
  brand_terms: ["UnblockingAPI", "Unblock API", "unblock_fetch", "@unblockingapi/mcp", "Scout", "Builder", "Growth", "Pay as you Go", "WK Operations Ltd"],
  differentiators: [
    "Credits never multiply: rendering, premium proxies and routing are in the one credit",
    "Failed requests are free",
    "Templates turn pages into JSON and can be published by the community",
    "Hosted MCP server with OAuth, so AI apps never see the API key",
  ],
  competitors: [
    { name: "ScrapingBee", note: "Compared on rendered-request credits: 25 there, 1 here." },
    { name: "ScraperAPI", note: "Compared on per-domain credit multipliers." },
    { name: "Bright Data", note: "Compared on price per 1,000 rendered premium-proxy requests." },
    { name: "Built-in web fetch in AI apps", note: "Positioned as the fix for pages those tools cannot read." },
  ],
  pricing_summary: "Five tiers from Pay as you Go (€0 to start, top-ups from €10) to Growth; €0.71–€1.00 per 1,000 credits. 500 free credits on sign-up, no card.",
  topic_clusters: [
    { label: "API & docs", description: "Quickstart, parameters, rendering and wait options for the core API.", page_count: 14, urls: [u("/docs"), u("/docs/quickstart"), u("/docs/parameters"), u("/docs/rendering"), u("/docs/wait-options")] },
    { label: "Templates", description: "The template library, categories and individual scrapers for known sites.", page_count: 22, urls: [u("/templates"), u("/templates/real-estate"), u("/templates/ecommerce"), u("/templates/serp")] },
    { label: "AI & MCP", description: "Setup guides for Claude, Cursor, VS Code and agent frameworks.", page_count: 13, urls: [u("/mcp"), u("/mcp/claude"), u("/mcp/cursor"), u("/mcp/vscode")] },
    { label: "Use cases", description: "Price monitoring, rank tracking, real estate listings and lead generation.", page_count: 9, urls: [u("/use-cases/price-monitoring"), u("/use-cases/rank-tracking"), u("/use-cases/real-estate")] },
    { label: "Comparisons", description: "Head-to-head pages against named scraping APIs.", page_count: 6, urls: [u("/compare/scrapingbee"), u("/compare/scraperapi"), u("/compare/brightdata")] },
    { label: "Guides", description: "Long-form developer guides on scraping public data responsibly.", page_count: 11, urls: [u("/guides/property-listings"), u("/guides/serp-scraping")] },
    { label: "Pricing & company", description: "Pricing, security, terms, privacy and contact.", page_count: 8, urls: [u("/pricing"), u("/security"), u("/terms"), u("/privacy"), u("/contact")] },
  ],
  content_gaps: [
    { topic: "How much does web scraping cost per 1,000 pages? A 2026 price comparison", rationale: "Buyers compare credit multipliers before choosing; supports the flat-credit pricing message.", intent: "commercial" },
    { topic: "Scraping JavaScript-heavy sites without running your own headless browser", rationale: "The core pain of the developer audience; leads straight to the Unblock API.", intent: "informational" },
    { topic: "Give Claude live web access with MCP: a step-by-step setup", rationale: "Captures AI-agent builders searching for setup help; supports the hosted MCP server.", intent: "informational" },
    { topic: "Residential vs datacenter proxies: when the difference matters", rationale: "High-volume search topic for scraper developers; explains why proxies are included.", intent: "informational" },
    { topic: "Build a daily Google rank tracker for 100 keywords", rationale: "Concrete use case with a known credit cost; supports SEO teams.", intent: "transactional" },
    { topic: "ScrapingBee alternatives for rendered requests", rationale: "Comparison-intent searchers are close to buying; extends the existing comparison pages.", intent: "commercial" },
    { topic: "Monitoring competitor prices on Shopify stores", rationale: "Price monitoring is a named use case without a dedicated guide.", intent: "informational" },
    { topic: "Is web scraping legal? Public data, terms and responsible use", rationale: "Answers the objection every new customer has; ties to the Acceptable Use Policy.", intent: "informational" },
  ],
};

const tpl = (t: string) => `https://app.unblockingapi.com/templates/kjellberg/${t}/card.png`;

/** What the crawler collects from unblockingapi.com's own pages and stylesheet. */
export const demoBrand: Brand = {
  site_name: "UnblockingAPI",
  tagline: "Access public web pages through a developer-friendly API with JavaScript rendering, country-level routing, managed browser sessions and clean API responses.",
  logo: u("/logo.png"),
  icon: u("/apple-touch-icon.png"),
  cover: u("/og-image.png"),
  colors: ["#ef2f72", "#ff2d78", "#be123c", "#ffbd2e", "#ff5f56"],
  fonts: ["Inter", "JetBrains Mono", "Caveat"],
  images: [
    { url: u("/og-image.png"), alt: "UnblockingAPI", page: u("/") },
    { url: tpl("idealista-search"), alt: "Idealista search template", page: u("/templates/idealista-search") },
    { url: tpl("reddit-post"), alt: "Reddit post template", page: u("/templates/reddit-post") },
    { url: tpl("hetzner-auction"), alt: "Hetzner auction template", page: u("/templates/hetzner-auction") },
    { url: tpl("mempool-space-block-view"), alt: "Mempool block view template", page: u("/templates/mempool-space-block-view") },
    { url: tpl("marbella-living-property"), alt: "Marbella Living property template", page: u("/templates/marbella-living-property") },
    { url: tpl("sigma-world-category"), alt: "Sigma World category template", page: u("/templates/sigma-world-category") },
  ],
  social: [
    { network: "github", url: "https://github.com/unblockingapi" },
    { network: "x", url: "https://x.com/unblockingapi" },
  ],
  updated_at: new Date(Date.now() - 4 * 864e5).toISOString(),
};
