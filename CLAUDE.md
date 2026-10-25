# Working on KIQR

Read `docs/PLAN.md` for the architecture and the decisions behind it, and `DESIGN.md` before touching anything in `web/`.

## Layout

- `crates/kiqr-core` — storage (files + git), events, crawler, markdown, profile model, providers, extraction/synthesis
- `crates/kiqr-daemon` — axum API, SSE, embedded web UI
- `crates/kiqr` — the `kiqr` binary: CLI + `kiqr daemon`
- `web/` — React + Vite + TypeScript; `npm run build` writes `web/dist`, which the daemon embeds
- `abilities/` — built-in ability folders (compiled in with `include_str!`, installed to `~/.kiqr/abilities` on first run or from the marketplace): `SKILL.md` instructions (`seemo-agent` is Seemo's tool guide, `make-images` the image process, `social-posts`, `marketing-plan` and the four profile prompts belong to core abilities) and `extension.yaml` connections (`kiqr-render`, `kiqr-smtp`, `fal-image`, `openai-image`, `google-analytics`, `search-console`, `plausible`); format in `docs/abilities.md`
- `assets/fonts/` — Inter, bundled for the renderer
- Seemo: `crates/kiqr-core/src/tools.rs` (the built-in tool catalogue and groups), `abilities.rs` (ability contracts), `extensions.rs` (manifests, instructions frontmatter, the registry over `abilities/` folders, what ships and what is core), `secrets.rs` (`.env` per project with the machine's as fallback), `render.rs` (SVG via resvg, HTML via a browser), `channels.rs`, `agent.rs` (claude arguments, stream-json parsing, system prompt), `chat.rs`, `tasks.rs`, `marketing.rs` (the plan and campaigns), `metrics.rs` (snapshots, KPI bindings, windows); the daemon's `agent.rs` (processes, transcripts, runs), `seemo.rs` (chat and task routes, approvals, the calendar, To do), `ext.rs` (the dynamic catalogue, the executor for http/command/builtin, instructions and secrets routes), `market.rs` (the one ability view: the list per project, the marketplace index, install, update, pause, remove), `marketing.rs` (plan and campaign routes, the cards behind them), `metrics_src.rs` (the readers: Google Analytics, Search Console, Plausible, KIQR's own publish count), `performance.rs` (the Performance view, refresh, the daily pull), `mcp_host.rs` (hosted MCP servers), `images.rs` (brand kit, rendering), `publish.rs` (channels, publishing, approve / schedule / unpublish, the scheduled publishes, email), `builtin.rs`, `scheduler.rs`; the CLI's `mcp.rs` (`kiqr mcp`, the MCP server, which reads the catalogue from the daemon)
- `seemo/PERSONA.md` — Seemo's standing instructions

## Rules

- No database. State is YAML/Markdown under `~/.kiqr`; the daemon is the only writer and commits after every logical change (`Store::commit`) with a readable message. Automated changes use the `[auto]` prefix. Never add git remotes or pushes.
- Secrets only in `.env`. Config files reference them by variable name.
- The CLI never touches files; it calls the daemon's API. Any new capability gets an API route first, then CLI and UI.
- Every state change emits an `EventKind`; the UI and `--watch` depend on it.
- UnblockingAPI is promoted only where pages could not be read: the crawl summary, its ability card, and one Seemo message per crawl. Nowhere else. It is an ability like the others, on when it has a key; there is no separate crawl switch.
- UI: follow `DESIGN.md`. Dark theme only, modelled on ContentWriter (`~/Projects/contentwriter`); when in doubt, do what ContentWriter does. Tokens in `web/src/tokens.css`; Inter throughout, JetBrains Mono for paths and numbers; teal for actions and live state, indigo for AI/learning, lime only for Seemo's mark. Icons from lucide-react; motion from `motion/react`. Preview any UI change with the demo build (`npx vite build -c vite.demo.config.ts` in `web/`, open `dist-demo/index.html?demo`).
- Crawling: KIQR fetches pages itself; the model only reads fetched Markdown from the queue. Never make fetching wait on the model.
- Seemo acts only through KIQR's tools; never the provider's built-in tools (`--tools ""` stays; web reading is `read_url` over our fetcher, not WebFetch). A new capability is a route, then a tool, then the UI; one that a connection could provide is an ability contract in `abilities.rs` plus a manifest, yaml first and Rust only when yaml cannot express it. Anything with side effects a person should see first goes through an approval (`create_task`, `change_config`, `save_ability`, `create_channel`, `publish_asset`, `send_email`); publishing asks unless the user switched a channel to let Seemo publish on its own (`asks_first: false`), and even then the card is put in the chat as the record. What Seemo makes is approved whole: an asset shows in the chat as it will look, image included, and the user approves it there, in the Library or on Seemo → To do; images are never approved on their own. An asset's status is derived, never set: Draft until approved for a channel (Ready), Scheduled with a publish time, Published when it went out, Archived when put away; the user's only direct moves are unpublish, archive and bring back. Everything Seemo writes comes with a featured image (a post's image, an article's cover, an email's header), all on brand; a type says so in its `TYPE.yaml` (`image: always | usually | none`) and the image lands in the role the type names. The system prompt stays small (the brief and the list of abilities with instructions, not the profile: argv is capped at 128 KiB).
- Two words for the user, one folder underneath: the user sees abilities (one page per project, Setup → Abilities: what Seemo can do here with a switch each, "Teach Seemo something", the marketplace, a modal per ability with Overview, Instructions and Connect; installed once for the machine, switched on, connected with the project's own key and taught per project; core abilities switch off but never leave; nothing but writing, the renderer and web reading is on by default) and channels (per project, with the project's own account). There is no "skill" in the UI: instructions written by the user are an ability. Seemo sees tools named after ability contracts and reads instructions with `read_instructions`; the code has extensions and transports (http, command, mcp, builtin). No transport word, and no "connection", outside Advanced. Secrets are set by name through the daemon, per project first (`projects/<slug>/.env` wins over `~/.kiqr/.env`), and never shown or sent to the model.
- Seemo writes in first person, plainly, no emoji or exclamation marks; it never claims to have done something it has not.
- Keep `docs/PLAN.md` current when a decision changes.

## Build and test

```sh
cargo build
cd web && npm ci && npm run build
KIQR_HOME=/tmp/kiqr-dev ./target/debug/kiqr daemon   # throwaway home
./target/debug/kiqr project add http://127.0.0.1:8099/ --watch
```

A quick local site for crawl tests: `python3 -m http.server 8099` in a folder of HTML files.
