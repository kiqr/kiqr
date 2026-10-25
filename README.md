# KIQR

**A marketing team that lives on your machine.** Give KIQR your website. It reads it, builds a business profile where every fact links back to the page it came from, and from then on Seemo, the marketer built in, writes, designs and schedules with you: posts with their images, articles, emails, a weekly report, whatever you teach it. Runs locally, on your own Claude or Codex subscription or API key. No account, no database, no data leaving your machine that you did not send.

## Why developers pick it

- **It knows your business, with receipts.** The profile is not a prompt you wrote; it is extracted from your site, fact by fact, each one linked to its source page and kept current when the site changes. Everything Seemo makes starts from that.
- **Drafts you approve, not a feed you babysit.** Every post lands in the chat as it will look, image included. One click approves it for a channel; publish now or pick a time, and the daemon sends it on schedule. Seemo asks before publishing until you flip a switch on a channel; then it publishes there on its own and the chat records what went out.
- **Tasks on a schedule.** "Every weekday at 10:00, write one LinkedIn post about a different offering." Seemo proposes the task on a card, you say yes once, and every run is kept with its steps and cost. A To do page collects what waits for you.
- **Images from day one.** No image API needed: Seemo designs on-brand cards and renders them locally (SVG, or HTML through a browser on the machine), from your site's own colours, fonts and logo. Plug in fal.ai or OpenAI when you want photos.
- **Teach it in plain words.** How you write a product update, how to publish to your blog, what the Monday report contains: write it once as an ability and Seemo follows it every time. Abilities are folders of Markdown and YAML, shareable through a marketplace.
- **Files, not a database.** `~/.kiqr` is YAML and Markdown in a local git repo. Every change is a commit with a readable message. Grep it, diff it, back it up with plain git.
- **One binary.** Rust daemon with the React UI embedded, a CLI that talks to it, and an MCP server so Seemo runs as a Claude Code session with KIQR's own tools and nothing else.
- **Open source, AGPL.** Hosted extras live at kiqr.cloud; nothing in the core is gated.

## What a session looks like

```
kiqr project add https://your-site.com --watch     # reads the site, builds the profile
```

Open http://127.0.0.1:7777. The library guide offers to write your first three social posts; approve them for a channel, or tell Seemo what you would rather have:

> Write two Facebook posts for Halloween, with images.

Seemo reads your profile and its instructions for social posts, saves two drafts with a designed image each, and shows them in the chat. You approve, schedule one for Friday at nine, and it goes out through the channel you connected.

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
- Abilities are folders under `~/.kiqr/abilities/` (or a project's own): `SKILL.md` instructions, `extension.yaml` connections, or both. Edit the shipped ones from Setup → Abilities, teach new ones there, or install from the marketplace.
- Seemo runs through Claude Code (`kiqr mcp` is the MCP server) with KIQR's tools only. Anything with a side effect goes through a card you approve: tasks, settings changes, new abilities, channels, publishing, email.
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
kiqr chat <slug>                 talk to Seemo from the terminal
kiqr todo <slug>                 what waits for you: approvals, drafts, publishes coming up
kiqr assets ls|show|approve|publish|schedule|unpublish|archive|rm
kiqr abilities ls|show|teach|install|update|pause|resume|rm
kiqr secrets set <NAME> [--project <slug>]
```

## Development

See [docs/PLAN.md](docs/PLAN.md) for the architecture and roadmap and [DESIGN.md](DESIGN.md) for the UI standard. `npm run dev` in `web/` serves the UI with hot reload and proxies `/api` to a daemon on 7777.

## License

AGPL-3.0. Commercial hosting and connectors are offered at [kiqr.cloud](https://kiqr.cloud).
