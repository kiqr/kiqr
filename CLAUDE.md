# Working on KIQR

Read `docs/PLAN.md` for the architecture and the decisions behind it, and `DESIGN.md` before touching anything in `web/`.

## Layout

- `crates/kiqr-core` — storage (files + git), events, crawler, markdown, profile model, providers, extraction/synthesis
- `crates/kiqr-daemon` — axum API, SSE, embedded web UI
- `crates/kiqr` — the `kiqr` binary: CLI + `kiqr daemon`
- `web/` — React + Vite + TypeScript; `npm run build` writes `web/dist`, which the daemon embeds
- `skills/` — built-in skill folders (compiled in with `include_str!`, installed to `~/.kiqr/skills` on first run): `SKILL.md` prompts (`seemo-agent` is Seemo's tool guide, `make-images` the image process) and `extension.yaml` connections (`kiqr-render`, `kiqr-smtp`, `fal-image`, `openai-image`); format in `docs/skills.md`
- `assets/fonts/` — Inter, bundled for the renderer
- Seemo: `crates/kiqr-core/src/tools.rs` (the built-in tool catalogue and groups), `abilities.rs` (ability contracts), `extensions.rs` (manifests, the registry over `skills/` folders), `render.rs` (SVG via resvg, HTML via a browser), `channels.rs`, `agent.rs` (claude arguments, stream-json parsing, system prompt), `chat.rs`, `tasks.rs`; the daemon's `agent.rs` (processes, transcripts, runs), `seemo.rs` (chat and task routes, approvals), `ext.rs` (the dynamic catalogue, the executor for http/command/builtin, skills and secrets routes), `mcp_host.rs` (hosted MCP servers), `images.rs` (brand kit, rendering), `publish.rs` (channels, publishing, email), `builtin.rs`, `scheduler.rs`; the CLI's `mcp.rs` (`kiqr mcp`, the MCP server, which reads the catalogue from the daemon)
- `seemo/PERSONA.md` — Seemo's standing instructions

## Rules

- No database. State is YAML/Markdown under `~/.kiqr`; the daemon is the only writer and commits after every logical change (`Store::commit`) with a readable message. Automated changes use the `[auto]` prefix. Never add git remotes or pushes.
- Secrets only in `.env`. Config files reference them by variable name.
- The CLI never touches files; it calls the daemon's API. Any new capability gets an API route first, then CLI and UI.
- Every state change emits an `EventKind`; the UI and `--watch` depend on it.
- UnblockingAPI is promoted only where pages could not be read: the crawl summary, Settings, and one Seemo message per crawl. Nowhere else.
- UI: follow `DESIGN.md`. Dark theme only, modelled on ContentWriter (`~/Projects/contentwriter`); when in doubt, do what ContentWriter does. Tokens in `web/src/tokens.css`; Inter throughout, JetBrains Mono for paths and numbers; teal for actions and live state, indigo for AI/learning, lime only for Seemo's mark. Icons from lucide-react; motion from `motion/react`. Preview any UI change with the demo build (`npx vite build -c vite.demo.config.ts` in `web/`, open `dist-demo/index.html?demo`).
- Crawling: KIQR fetches pages itself; the model only reads fetched Markdown from the queue. Never make fetching wait on the model.
- Seemo acts only through KIQR's tools; never the provider's built-in tools (`--tools ""` stays; web reading is `read_url` over our fetcher, not WebFetch). A new capability is a route, then a tool, then the UI; one that a connection could provide is an ability contract in `abilities.rs` plus a manifest, yaml first and Rust only when yaml cannot express it. Anything with side effects a person should see first goes through an approval (`create_task`, `change_config`, `save_skill`, `create_channel`, `publish_asset`, `send_email`); publishing always asks. The system prompt stays small (the brief and the skill list, not the profile: argv is capped at 128 KiB).
- Three layers of words, each blind to the one below: the user sees abilities, connections, channels and skills; Seemo sees tools named after abilities; the code has extensions and transports (http, command, mcp, builtin). No transport word outside Advanced. Secrets are set by name through the daemon and never shown or sent to the model.
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
