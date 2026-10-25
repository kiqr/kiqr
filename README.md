# KIQR

Your self-hosted CMO.

Give KIQR a website. It reads it, builds a business profile where every fact links back to the page it came from, and keeps that profile current. Seemo, the marketing persona built in, works from that profile: strategy, content, campaigns — on your machine, with your own Claude or Codex subscription or API key. No account required.

## Install

```sh
curl -fsSL https://kiqr.dev/install.sh | sh
```

This builds from source with Cargo (installing Rust if needed), puts `kiqr` on your PATH and starts the daemon at login. Then:

```sh
kiqr project add https://your-site.com --watch
```

or open http://127.0.0.1:7777 and paste the URL there.

### Docker

```sh
docker run -d -p 7777:7777 -v kiqr-data:/data --env-file .env ghcr.io/kiqr/kiqr
```

The container runs on API keys (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`) or a subscription token from `claude setup-token` (`CLAUDE_CODE_OAUTH_TOKEN`).

### From source

```sh
cd web && npm ci && npm run build && cd ..
cargo build --release
./target/release/kiqr daemon
```

## How it works

- `~/.kiqr` holds everything as YAML and Markdown in a local git repository. Every change is a commit. KIQR never pushes; add a remote yourself if you want a backup.
- Secrets live in `~/.kiqr/.env` (mode 0600, ignored by git).
- The crawler reads up to 200 pages per site. Pages that block crawlers or need JavaScript are retried through [UnblockingAPI](https://unblockingapi.com) if you add a key, and reported as unreadable otherwise.
- Prompts are `SKILL.md` files under `~/.kiqr/abilities/`; edit them (or a project's copy, from Seemo → Abilities) to tune extraction and profile building.
- Models per task are set in `~/.kiqr/config.yaml` as `provider/model`: `claude-cli/haiku`, `anthropic/claude-sonnet-5-5`, `openai/gpt-5-mini`, `codex-cli/gpt-5`.

## CLI

```
kiqr daemon                      run the daemon (serves the web UI)
kiqr status                      providers, connections, home directory
kiqr project add <url> [--watch]
kiqr project list
kiqr crawl <slug> [--watch] [--max-pages N]
kiqr profile <slug> [--yaml]
kiqr synthesize <slug>
kiqr history <slug>
kiqr events [--project <slug>]
```

## Development

See [docs/PLAN.md](docs/PLAN.md) for the architecture and roadmap and [DESIGN.md](DESIGN.md) for the UI standard. `npm run dev` in `web/` serves the UI with hot reload and proxies `/api` to a daemon on 7777.

## License

AGPL-3.0. Commercial hosting and connectors are offered at [kiqr.cloud](https://kiqr.cloud).
