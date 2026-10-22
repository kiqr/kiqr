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
- **Skills.** Prompts are `SKILL.md` files. Built-in ones ship in `skills/`; copies in `~/.kiqr/skills/` override them so users can tune extraction and synthesis.
- **Seemo** is the persona, defined in `seemo/PERSONA.md` (installed to `~/.kiqr/seemo/PERSONA.md`). Chat is scoped to the active project and always starts from that project's `PROFILE.md`.
- **License: AGPL-3.0.** Keeps the client open while making competing hosted offerings unattractive.

## Layout of `~/.kiqr`

```
.git/                      local only
.gitignore
.env                       secrets, 0600, ignored
config.yaml                daemon host/port, crawl defaults, models per task
skills/<name>/SKILL.md     user-editable prompts
seemo/PERSONA.md
projects/<slug>/
  project.yaml             url, crawl settings, last crawl summary
  PROFILE.md               rendered profile; what agents read
  profile/<section>.yaml   editable source; fields with evidence, confidence, locks
  facts/<page>.yaml        per-page extracted facts
  chats/                   Seemo conversations (later)
  cache/pages/<hash>.*     raw html, markdown, metadata — ignored
```

## Pipeline

1. **Crawl** (`kiqr-core::crawl`): sitemap + robots, same-origin BFS, priority for pricing/about/product pages, 200-page cap, tracking params stripped, raw cache written.
2. **Extract** (`skills/extract-facts`): each page's Markdown → JSON facts with a verbatim quote and confidence, routed to one of twelve sections. Runs during the crawl so facts stream into the UI.
3. **Synthesize** (`skills/build-profile`): per section, merge facts into fields, resolve conflicts, keep evidence, never touch locked fields. Writes `profile/*.yaml` and `PROFILE.md`, commits.
4. **Edit**: the UI and API write sections; a user edit locks the field.

## API

```
GET  /api/health                         version, home, provider availability
GET  /api/config  PUT                    config.yaml
GET  /api/projects  POST {url, crawl}    list / create (+ start crawl)
GET  /api/projects/:slug  DELETE
POST /api/projects/:slug/crawl?max_pages=
POST /api/projects/:slug/synthesize
GET  /api/projects/:slug/profile         sections as JSON
PUT  /api/projects/:slug/profile/:id     save one section
GET  /api/projects/:slug/profile.md
GET  /api/projects/:slug/history         git log for the project
GET  /api/events?project=                SSE
```

## Roadmap

1. ~~Workspace, store + git, events, crawler, extraction, synthesis, CLI, web UI~~
1b. ~~Parallel fetch with a separate model queue; project selector and project-scoped navigation; the animated crawl map; design v2 (warm paper, Bricolage + Inter)~~
2. Seemo chat: `POST /api/projects/:slug/chat`, transcript in `chats/`, tools to read/edit the profile and start re-crawls, approval gate for side effects
3. Re-crawl diffs in the UI from git history; scheduled re-crawls
4. Competitor sub-profiles
5. MCP broker: a local stdio MCP server exposed to the model that forwards to connectors with allowlists, scopes, an approval gate and an audit log; tokens encrypted at rest, never visible to the model
6. kiqr.cloud: sign-in, connect relay (Gmail, Stripe, LinkedIn…), hybrid mode (cloud UI, local daemon), hosted daemons
7. Prebuilt binaries so `install.sh` no longer needs Cargo
8. Marketing tasks on top of the profile: content, social, email, ads

## Not in scope for the MVP

GitHub integration, multi-user, a TUI, anything that posts or sends on the user's behalf.
