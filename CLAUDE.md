# Working on KIQR

Read `docs/PLAN.md` for the architecture and the decisions behind it, and `DESIGN.md` before touching anything in `web/`.

## Layout

- `crates/kiqr-core` — storage (files + git), events, crawler, markdown, profile model, providers, extraction/synthesis
- `crates/kiqr-daemon` — axum API, SSE, embedded web UI
- `crates/kiqr` — the `kiqr` binary: CLI + `kiqr daemon`
- `web/` — React + Vite + TypeScript; `npm run build` writes `web/dist`, which the daemon embeds
- `skills/` — built-in `SKILL.md` prompts (compiled in with `include_str!`, installed to `~/.kiqr/skills` on first run)
- `seemo/PERSONA.md` — Seemo's standing instructions

## Rules

- No database. State is YAML/Markdown under `~/.kiqr`; the daemon is the only writer and commits after every logical change (`Store::commit`) with a readable message. Automated changes use the `[auto]` prefix. Never add git remotes or pushes.
- Secrets only in `.env`. Config files reference them by variable name.
- The CLI never touches files; it calls the daemon's API. Any new capability gets an API route first, then CLI and UI.
- Every state change emits an `EventKind`; the UI and `--watch` depend on it.
- UnblockingAPI is promoted only where pages could not be read: the crawl summary, Settings, and one Seemo message per crawl. Nowhere else.
- UI: follow `DESIGN.md`. Dark theme only, modelled on ContentWriter (`~/Projects/contentwriter`); when in doubt, do what ContentWriter does. Tokens in `web/src/tokens.css`; Inter throughout, JetBrains Mono for paths and numbers; teal for actions and live state, indigo for AI/learning, lime only for Seemo's mark. Icons from lucide-react; motion from `motion/react`. Preview any UI change with the demo build (`npx vite build -c vite.demo.config.ts` in `web/`, open `dist-demo/index.html?demo`).
- Crawling: KIQR fetches pages itself; the model only reads fetched Markdown from the queue. Never make fetching wait on the model.
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
