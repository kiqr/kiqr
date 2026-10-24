# KIQR — architecture and plan

KIQR is a self-hosted marketing agent. You give it a website; it crawls it, builds a business profile with evidence for every fact, and from then on Seemo, the CMO persona, plans and runs marketing with you. This document is the design the code follows and the roadmap after the MVP.

## Decisions

- **Open source, no account required.** Everything in this repo runs on the user's machine (or in Docker) with their own Claude/Codex subscription or API key. kiqr.cloud adds optional pro features; it never gates the core.
- **Rust.** One binary, `kiqr`: CLI and daemon (`kiqr daemon`). The web UI (React + Vite) is built to `web/dist` and embedded in the binary.
- **Files, not a database.** `~/.kiqr` (or `/data` in Docker) holds YAML and Markdown under a local git repo. The daemon is the single writer and commits after every change with a readable message; automated commits start with `[auto]`. There are no remotes and no pushes; a user who wants a backup adds a remote and pushes with plain git. Raw fetches live in `projects/<slug>/cache/` and are ignored.
- **Secrets in `.kiqr/.env`**, mode 0600, ignored by git. Config files reference secrets by variable name only.
- **Built-in fetcher first; UnblockingAPI on escalation.** Pages that return 401/403/429/503, show a bot challenge or are empty JavaScript shells are retried through UnblockingAPI when a key is present. Without a key they are reported as unreadable, and UnblockingAPI is mentioned exactly there: in the crawl summary, in Settings, and once by Seemo.
- **Providers.** `provider/model` strings per task (`extract`, `synthesize`, `chat`): `anthropic/…`, `openai/…` over HTTP with keys; `claude-cli/…`, `codex-cli/…` through the local CLIs with the user's login. Docker runs on API keys or `CLAUDE_CODE_OAUTH_TOKEN` from `claude setup-token`.
- **`web/dist` is committed** so `cargo install --git` works without Node. Rebuild it (`npm run build`) in the same commit as any UI change; a release workflow should take this over.
- **Crawling is two independent stages.** KIQR fetches pages itself (reqwest, or UnblockingAPI on escalation), `concurrency` at a time (default 5, max 10). Every fetched page is queued for the model, which reads `ai_concurrency` pages at a time (default 3). A slow model never slows the fetcher. Both are per-project settings and can be set per run.
- **Demo mode.** `?demo` (or `window.__KIQR_DEMO__`) replays a scripted crawl into the real screens without a daemon; `web/vite.demo.config.ts` builds one self-contained HTML file for previews and the site.
- **SSE, not WebSockets.** All state changes flow daemon → client as events; actions are POSTs. `kiqr crawl --watch` prints the same stream.
- **Crawls replay.** The daemon logs every event of a project's latest crawl job (crawl plus synthesis) in memory, and on finish keeps it in `cache/crawl-log.jsonl`. `/api/events?project=…&replay=1` sends that log first (frames marked `"replay": true`), then `replay_done` with `running`, then live events, with no gap between them. The crawl screen and `kiqr crawl --watch` use it, so leaving the page or joining mid-crawl shows the whole crawl.
- **Only the business's own pages.** KIQR crawls the site and nothing else. Links with `target="_blank"` or `rel` nofollow, noopener or sponsored are never followed, even internal ones: that is how sites mark outbound and affiliate links. Redirects are followed within the site (http→https, www, moved pages) and never to another website: a link that redirects away (`/till/…`, `/go/…`) is skipped (`page_skipped`) without fetching the other site's page, and after three such redirects under one path the rest of that path is skipped without a request. Two links that land on the same page read it once. If the project's own start URL redirects away, Seemo says so. Pages cached by older crawls that left the site are ignored by the profile, the brief and the brand, and before synthesis any evidence from them is removed from the profile (fields that rested only on it are dropped, locked fields kept). Brand imagery comes only from the business's own hosts: its domain and subdomains, and the hosts its own logo, icon and share image live on (a shop platform's CDN); resized copies of one picture count once. Social profiles count as the business's own only when its JSON-LD Organization lists them (`sameAs`), when the handle is its name or domain, or when they sit in the header or footer of at least half its pages; JSON-LD inside products (a product's brand or seller) and organisations with another website are ignored.
- **Brands are saved, not rebuilt per view.** Building a brand reads every cached page (seconds), so it happens once: at the end of a crawl, on a project's first brand request, and at daemon start for projects without one. Each saved brand records the collection `rules` version that built it; when the rules change (`brand::RULES`), outdated brands are rebuilt from the cache the same way.
- **Live page limit.** A running crawl's page limit can be changed while it fetches (`PUT /api/projects/{slug}/crawl/limit`, the crawl screen's "Raise the limit" dialog, `kiqr crawl <slug> --max-pages N`). The crawler checks the limit before each fetch, and pages over the old limit stay queued, so a raise continues straight away. Once fetching has ended the limit applies from the next crawl; the response says which (`live`). The limit is saved to the project either way, and the change is the `crawl_limit_changed` event.
- **One job per project.** Starting a crawl, synthesis or brief claims the project's slot in one step; a second request gets 409. A crawl ends by rebuilding the profile, so it doesn't overlap a profile job either. `GET /api/projects/{slug}` lists running `jobs`, and `job_finished` says when one ends. The UI shows a running crawl instead of starting another; `kiqr crawl --watch` follows it.
- **Assets, in a Library.** What users make with Seemo (articles, posts, Reddit replies, images, memes, videos, songs, ad campaigns, and kinds nobody has defined yet) are assets. A type is data, not code: `asset-types/<type>/TYPE.yaml` names it, picks a lucide icon, lists the files it holds and how to preview it (article, post, image, video, audio, campaign or files). Built-in types ship in `asset-types/` and are installed like skills; users and Seemo add more (`POST /api/asset-types`), and the sidebar grows an item for each type that has assets (archived ones aside: they show only under Archive). An asset is a folder, `projects/<slug>/assets/<type>/<id>/`: `asset.yaml` (type, title, status draft/ready/scheduled/published/archived, tags, who made it: `user`, `task:<id>` or `chat:<id>`, what it was made from, its files with size and sha256, and where it was published) plus its files.
- **Seemo's first assets.** The library's getting-started guide ends with three optional steps (skippable, remembered in the browser): generate 3 social media posts, connect where you publish, and put Seemo on the calendar. Posts are written by the chat model with the `social-posts` skill from the brief and the brand's own social networks, saved as draft `post` assets made by `seemo` (`kiqr assets generate <slug>`).
- **Generated assets stay out of git.** `projects/*/assets/` is ignored (videos and audio would bloat the history), so asset changes emit `asset_saved`/`asset_deleted` but are not committed. The library is read from the folders. Backups to S3 come later. Asset types are configuration and are committed.
- **Seemo chat is Claude Code with KIQR's tools.** Each chat turn and each task run is one `claude -p` process (stream-json, `--tools ""`, `--strict-mcp-config`, no `--bare` so the user's subscription login works) whose only tools come from `kiqr mcp`: the same binary serving KIQR's API over MCP on stdio, told the project, the chat or run, and the allowed tool groups through its environment. The tool catalogue lives once in `kiqr-core::tools` (groups `read`, `write_profile`, `write_assets`, `jobs`, `tasks`, `config`); each tool is one request to the daemon, so the MCP process never touches files. A chat keeps the provider's session id and resumes it every turn; our own transcript (`chats/<id>/messages.jsonl`, committed per turn) is the record, and when the provider has forgotten the session a new one starts from that transcript. The system prompt is the persona, the `seemo-agent` skill, where we are (project, local time and timezone, running jobs, tasks) and the brief as compact JSON; never the whole profile, which is over Linux's 128 KiB argument limit and is a tool call away. The chat model must be `claude-cli/…` for now; Codex and HTTP providers get their own tool loops later. **No built-in tools, ever:** Claude Code's Bash, Read, Write, WebFetch and WebSearch stay off (`--tools ""`), whatever the provider. Every capability Seemo gets is one of ours, so it behaves the same on every provider, is confined to what KIQR knows how to do, and shows up as a step the user can read. Reading the web is `read_url` (group `web`), which goes through KIQR's own fetcher with the UnblockingAPI fallback, not WebFetch; images, search and the like will be KIQR tools in the same way. The `assets/generate` job stays as the provider-agnostic way to write posts (`kiqr assets generate`).
- **Approvals.** Two tools ask the user first: `create_task` and `change_config`. The tool opens an approval (`POST …/approvals`), the chat gets a card, the tool polls until the person approves or declines in the UI (or 15 minutes pass), and Seemo finishes its turn knowing the outcome. Approving a config change saves `config.yaml`, commits, swaps the daemon's cached config and emits `config_changed`; host and port apply after a restart and the card says so. Secrets can never be set this way.
- **Seemo is the automation.** A task is a plain-language instruction, a schedule (once, or an RRULE with an IANA timezone: "weekdays at 10:00", "every day in December until the 24th") and the tool groups its runs may use (never `tasks` or `config`: those need a person). Seemo proposes tasks from chat; the user confirms once on the card, which shows the schedule in words and the next three occurrences; "do it now" is a one-off task that runs on approval. Each run is a headless Seemo session, recorded in `tasks/<id>/runs/<ts>.yaml` (steps, assets made, summary, cost); runs hold `task:<id>` in the job set, not the crawl slot, so they may overlap a crawl. The scheduler ticks every 30 s; runs due while the daemon was down execute once if under an hour late, otherwise they are recorded as missed. A one-off task made in a chat reports back there: the daemon adds a note and Seemo writes one message about the result. Scheduled runs only update their card and say one line as `seemo_says`.
- **Connectors** (planned, roadmap 5). Two kinds behind one interface: community packs installed via the kiqr.cloud OAuth relay, and custom MCP servers. Each has `connector.yaml` (auth as env variable names, actions with read/write/publish level, asset types it publishes or produces) and a `SKILL.md` Seemo reads before using it. Credentials live in `projects/<slug>/.env` (ignored) or the global `.env`; values never reach the model. Connecting must be easy for people who are not technical: the Connectors page is a grid of services with one Connect button each (sign in on the service's own page, back to KIQR with "Connected as …"), permissions are two plain-word toggles (read and draft; publishing asks first) that become the connector's tool group, a key is asked for only when a service offers nothing else (one field, a link to where it is, never shown again), and an expired token shows a Reconnect badge and one line from Seemo. Seemo can open the same Connect card from chat. The sign-in flow is done by the kiqr.cloud relay, which may itself sit on a hosted broker (Composio, Pipedream or Zapier MCP) rather than registering every app; the official MCP Registry feeds an Advanced list for adding any MCP server by hand. Command lines, JSON and environment variable names never appear outside Advanced.
- **Skills.** Prompts are `SKILL.md` files. Built-in ones ship in `skills/`; copies in `~/.kiqr/skills/` override them so users can tune extraction and synthesis.
- **Seemo** is the persona, defined in `seemo/PERSONA.md` (installed to `~/.kiqr/seemo/PERSONA.md`). Chat is scoped to the active project and always starts from that project's business brief; the profile's evidence comes through tools.
- **License: AGPL-3.0.** Keeps the client open while making competing hosted offerings unattractive.

## Layout of `~/.kiqr`

```
.git/                      local only
.gitignore
.env                       secrets, 0600, ignored
config.yaml                daemon host/port, crawl defaults, models per task
skills/<name>/SKILL.md     user-editable prompts
asset-types/<type>/TYPE.yaml  asset types: built-in and user-defined
seemo/PERSONA.md
projects/<slug>/
  project.yaml             url, crawl settings, last crawl summary
  PROFILE.md               rendered profile; what agents read
  profile/<section>.yaml   editable source; fields with evidence, confidence, locks
  profile/brief.yaml       business brief: summary, offerings, audiences, voice, topic clusters, content gaps
  profile/brand.yaml       brand assets: name, tagline, logo, icon, cover, colours, fonts, images, social links
  facts/<page>.yaml        per-page extracted facts
  assets/<type>/<id>/      asset.yaml + files — ignored, not committed
  .env                     project secrets — ignored
  chats/<id>/              chat.yaml + messages.jsonl — committed after every turn
  tasks/<id>/              task.yaml + runs/<ts>.yaml — committed
tmp/agent/                 working directory of Seemo's `claude` processes — ignored
  cache/pages/<hash>.*     raw html, markdown, metadata — ignored
```

## Pipeline

1. **Crawl** (`kiqr-core::crawl`): sitemap + robots, same-origin BFS, priority for pricing/about/product pages, 200-page cap, tracking params stripped, raw cache written.
2. **Extract** (`skills/extract-facts`): each page's Markdown → JSON facts with a verbatim quote and confidence, routed to one of twelve sections. Runs during the crawl so facts stream into the UI.
3. **Synthesize** (`skills/build-profile`): per section, merge facts into fields, resolve conflicts, keep evidence, never touch locked fields. Writes `profile/*.yaml` and `PROFILE.md`, commits.
4. **Brief** (`skills/business-brief`): one call on top of the sections, the same business profile ContentWriter builds and more. Input: the site URL, every merged section (field name + value) and a numbered page list (path, title, up to three facts). Output: business name, summary, language, geographies, offerings (with source URLs), audiences, tone of voice, brand terms, differentiators, competitors, pricing summary, 3–8 topic clusters and 8–15 content gaps. Clusters come back as page numbers; Rust maps them to URLs (each page in one cluster) and sets `page_count`. Written to `profile/brief.yaml` with a `version` that increments on every write and `source: crawl | edited`, rendered at the top of `PROFILE.md`, committed as `[auto] profile(<slug>): business brief v<N>`. Runs at the end of every synthesis; `POST …/brief` reruns just this step.
5. **Brand** (`kiqr-core::brand`, no model): when fetching ends, before waiting on the model queue, the raw HTML in the cache plus up to three same-origin stylesheets of the home page (5 s, 2 MB each) give the brand: site name and tagline (og/JSON-LD/title), logo (JSON-LD Organization logo, else a logo image in the header), icon (apple-touch-icon, else the largest icon link, else `/favicon.ico`), cover (home og:image), up to 6 colours (theme-color first, then the most used saturated CSS colours, near-duplicates merged), up to 3 fonts ordered by weighted `font-family` use (body/sans stacks count most, monospace and cursive accents least; Google Fonts link order breaks ties), up to 12 images (each page's og:image, then large content images) and one social profile per network. Saved to `profile/brand.yaml`, rendered as a short Brand block in `PROFILE.md`, committed as `[auto] brand(<slug>): …`, emits `brand_updated {logo, colors, images}`. `GET …/brand` builds it from the cache on the fly (without writing) when the file is missing, so projects crawled earlier get one without a rescan; `POST …/brand` rebuilds and saves.
6. **Edit**: the UI and API write sections and the brief; a user edit locks a field, and an edited brief is `source: edited` until the next crawl rewrites it.

Synthesis emits `synthesis_started {total}` (sections to build + 1 for the brief), one `profile_updated {section, by, title, items}` per section and one for `brief`, then `synthesis_finished {sections}` whether it succeeded or not; on failure Seemo says what failed. `page_fetched` carries the page `title` and `words` so the crawl screen can show them.

## API

```
GET  /api/health                         version, home, address, provider availability, which secrets are set (names only), model CLIs found
GET  /api/config  PUT                    config.yaml
GET  /api/projects  POST {url, crawl}    list / create (+ start crawl)
GET  /api/projects/:slug  DELETE
POST /api/projects/:slug/crawl?max_pages=
POST /api/projects/:slug/synthesize
GET  /api/projects/:slug/profile         sections as JSON
PUT  /api/projects/:slug/profile/:id     save one section
GET  /api/projects/:slug/profile.md
GET  /api/projects/:slug/brief           business brief, or null
PUT  /api/projects/:slug/brief           save an edited brief (source: edited, version + 1)
POST /api/projects/:slug/brief           rewrite the brief in the background
GET  /api/projects/:slug/brand           brand assets (built from the cache if not saved yet); 404 before a crawl
POST /api/projects/:slug/brand           rebuild brand assets from the crawl cache and save
GET  /api/projects/:slug/history         git log for the project
PUT  /api/projects/{slug}/crawl/limit    {max_pages} → {max_pages, live}
GET  /api/asset-types  POST             types (built-ins first); add or change a type
GET  /api/projects/:slug/assets?type=&status=&tag=   POST {type, title, files: [{role, name, text}]}
GET  /api/projects/:slug/assets/summary  asset counts per type
POST /api/projects/:slug/assets/generate {type: post, count}  Seemo writes drafts from the brief (job `posts`)
GET  /api/projects/:slug/assets/:type/:id  PUT  DELETE
GET  /api/projects/:slug/assets/:type/:id/files/:name  PUT ?role= (raw body, up to 1 GB)
GET  /api/events?project=[&replay=1]     SSE (replay: latest crawl job first)
GET  /api/tools                          the tool catalogue and groups
GET  /api/skills  GET /api/skills/:name  skill names; a skill's text
GET  /api/projects/:slug/profile/:section            PUT …/fields/:field {value}  (sets and locks one field)
PATCH /api/projects/:slug/brief {changes}            PATCH /api/projects/:slug/assets/:type/:id
GET  /api/projects/:slug/chats  POST {title?}        GET PUT DELETE …/chats/:id
POST /api/projects/:slug/chats/:id/messages {text}   202; the reply streams as chat_delta/chat_tool, lands as chat_message
POST /api/projects/:slug/chats/:id/stop
POST /api/projects/:slug/approvals {kind, chat, payload}   from kiqr mcp; GET …/approvals/:id?wait=  POST {decision}
GET  /api/projects/:slug/tasks  POST                 GET PATCH DELETE …/tasks/:id; POST …/run | pause | resume
GET  /api/projects/:slug/tasks/:id/runs  …/runs/:ts
GET  /api/projects/:slug/calendar?from&to            upcoming runs
```

Chat events are project-scoped and carry the chat id: `chat_created/renamed/deleted`, `chat_turn_started`, `chat_delta` (display only, coalesced), `chat_tool` (started/done/failed with a human label), `chat_message` (a message added or, same id, changed), `chat_turn_finished`, `approval_requested/resolved`; tasks: `task_saved/deleted`, `task_run_started/step/finished`; `config_changed {by, restart}`. None of them go into the crawl log. The web client shares one `EventSource` per URL across listeners.

## Roadmap

1. ~~Workspace, store + git, events, crawler, extraction, synthesis, CLI, web UI~~
1b. ~~Parallel fetch with a separate model queue; project selector and project-scoped navigation; the animated crawl map; design v2 (warm paper, Bricolage + Inter)~~
1c. ~~Business brief: ContentWriter's site profile (summary, offerings, audiences, voice, clusters, gaps) on top of the evidence-backed sections; `kiqr brief`~~
1d. Content: KIQR does everything ContentWriter does, and more. An overview, a calendar, articles, social posts and other formats, a content map and content gaps. The brief's topic clusters (with the URLs of their pages) are the content map; its content gaps seed the article plan; articles and posts are generated from the business profile and the brief's tone of voice.
1e. ~~Library: asset types as data, assets per type, the sidebar by type, content gaps → draft articles; `kiqr assets`, `kiqr types`~~
2. ~~Seemo chat: Claude Code sessions with KIQR's tools over `kiqr mcp`, transcripts in `chats/`, approval cards, the Chats page and the widget; `kiqr chat`~~
2b. ~~Tasks: task files, the confirmation card, the scheduler, run records, the Tasks page and the calendar agenda~~; month/week/day views of the calendar (ContentWriter's board)
2c. Tool loops for Codex CLI and the HTTP providers, so chat is not tied to Claude Code
3. Re-crawl diffs in the UI from git history; scheduled re-crawls
4. Competitor sub-profiles
5. MCP broker: a local stdio MCP server exposed to the model that forwards to connectors with allowlists, scopes, an approval gate and an audit log; tokens encrypted at rest, never visible to the model
6. kiqr.cloud: sign-in, connect relay (Gmail, Stripe, LinkedIn…), hybrid mode (cloud UI, local daemon), hosted daemons
7. Prebuilt binaries so `install.sh` no longer needs Cargo
8. Marketing tasks on top of the profile: content, social, email, ads

## Not in scope for the MVP

GitHub integration, multi-user, a TUI. Posting on the user's behalf arrives with connectors (roadmap 5), only through actions the user confirmed.
