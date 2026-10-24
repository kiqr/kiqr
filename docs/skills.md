# Skills, connections and channels

The three words a user sees, and the one folder format behind them.

- **Ability**: what Seemo can do beyond writing: make images, make video, make music, send email, publish. Fixed by KIQR, each with one tool contract (`create_image`, `send_email`…).
- **Connection**: what provides an ability or a few tools of its own: a service behind a key, a program on this computer, or KIQR itself. Picked per ability under Settings → Models when several are set up.
- **Channel**: where a project's content goes out: an email address, a blog, an account. A channel names a connection that publishes plus a destination. Publishing always asks the user first.
- **Skill**: how Seemo does something, in plain words. Listed in the system prompt by name and description; Seemo reads the one that fits before it acts.

Seemo only ever sees tools, named after abilities, whatever connection is behind them. Transport words (HTTP, MCP, command) stay in the manifest and under Advanced.

## The folder

```
~/.kiqr/skills/<name>/              everywhere
~/.kiqr/projects/<slug>/skills/<name>/   this project only (wins on a name clash)
  SKILL.md          a skill: frontmatter + instructions         (optional)
  extension.yaml    a connection: what it provides and needs    (optional)
  templates/…       files the pack ships                        (optional)
```

A folder with only `SKILL.md` is a skill. With only `extension.yaml` it is a connection. With both it is a pack, such as "Publish to WordPress": the instructions and the one call that does the posting.

Install from the Skills page (Advanced), or with `kiqr skills add <folder-or-git-url> [--project <slug>]`. Remove with `kiqr skills rm <name>`. KIQR's built-in folders are reinstalled on every start if missing; a built-in `extension.yaml` follows each release, a built-in `SKILL.md` is written once and then yours.

## SKILL.md

```markdown
---
name: product-updates
description: When asked for a post about something we shipped.
abilities: [images]          # optional: what it needs; the Skills page says "needs a connection" when none is ready
channels: [wordpress]        # optional
---

1. Lead with what changed for the customer, in one sentence.
2. One concrete example or number.
3. End with where to try it. No hype words.
```

`description` is the trigger: it is what Seemo sees in the system prompt. Write it as "When …".

## extension.yaml

```yaml
id: wordpress                  # lowercase letters, digits, dashes
name: WordPress                # the connection's name in the UI
version: 0.1.0
summary: Publish articles to our WordPress site.
homepage: https://example.com/wp-admin
transport: http                # http | command | mcp | builtin
auth:                          # keys by variable name; values live in .env, set under Settings → Connections
  - { env: WP_USER, label: User name }
  - { env: WP_APP_PASSWORD, label: Application password, help: https://example.com/wp-admin/profile.php }
hosts: [myblog.com]            # the only hosts an http connection may call (subdomains included)
tools:                         # the connection's own tools, offered under its own group ("Use WordPress")
  - name: publish_article
    label: Publishing to WordPress
    description: Create a post from a title and HTML body.
    input: { type: object, properties: { title: { type: string }, html: { type: string }, status: { type: string } }, required: [title, html] }
    request:
      method: POST
      url: https://myblog.com/wp-json/wp/v2/posts
      headers: { Authorization: "Basic ${WP_BASIC}", Content-Type: application/json }
      json: { title: "{{title}}", content: "{{html}}", status: "{{status|draft}}" }
      timeout_s: 60
    result: { text: /link }    # JSON pointer into the response: what Seemo is told (a URL becomes the publication's link)
channels:
  - kind: wordpress
    label: WordPress blog
    publish: publish_article   # the tool (or ability) that performs a publish
    destination:
      site: { type: string, label: Site, help: "https://myblog.com" }
```

### Placeholders

- `{{arg}}` and `{{arg|default}}`: the tool's arguments. A string that is exactly one placeholder takes the argument's own JSON value (numbers stay numbers).
- `${NAME}`: a variable from `.env`, filled by the daemon at call time. The model never sees values.
- `map`: replace an argument before templating: `map: { orientation: { landscape: 1536x1024, portrait: 1024x1536, "*": 1024x1024 } }`.

### Providing an ability

```yaml
provides:
  image:                       # image | video | music | email
    cost: about $0.03 per image
    request: { method: POST, url: https://fal.run/fal-ai/flux/schnell, headers: { Authorization: "Key ${FAL_KEY}" }, json: { prompt: "{{prompt}}", image_size: { width: "{{width|1600}}", height: "{{height|900}}" } } }
    result: { file_url: /images/0/url, mime: image/jpeg }
```

The ability's arguments are KIQR's (`prompt`, `size`, `width`, `height`, `orientation`, `aspect`, `asset`, `name`, `title`); `size` presets become `width`/`height`. A `result` with `file_url` or `file_b64` makes the daemon fetch or decode the file and put it on the asset. A non-JSON response body is taken as the file itself.

When a publish goes through a channel, the publish tool receives `title`, `text` (Markdown), `html`, `summary`, `tags`, `asset` and the channel's destination fields. Its `result.text` or a `url` in the answer becomes the publication's link.

### Transports

- **http**: declarative, as above. Hosts are enforced, redirects are not followed.
- **command**: a program on this machine. The daemon runs it with the call's arguments.
  ```yaml
  transport: command
  tools:
    - name: word_count
      command: { command: /usr/local/bin/count-words, args: ["--json"], stdin: json, env: [], timeout_s: 30 }
      result: { text: /words }
  ```
  stdout is read as JSON when it parses. Only the variables named in `env` reach the program.
- **mcp**: an MCP server the daemon hosts and proxies. Started on first use, kept alive, restarted once if it dies, stopped with the daemon.
  ```yaml
  transport: mcp
  command: npx
  args: ["@playwright/mcp@latest", "--headless"]
  env: []
  expose: [browser_navigate, browser_snapshot, browser_click, browser_type]   # or [all]
  provides:
    image: { tool: browser_take_screenshot, args: { filename: "{{name}}.png" } }
  ```
  `expose` offers the server's own tools raw under the connection's group. `provides` maps an ability onto one of them. Image or audio content in a result becomes a file.
- **builtin**: Rust inside KIQR (`kiqr-render`, `kiqr-smtp`).

Every connection shows what it needs (keys), what it can call (hosts), what it costs, and whether it runs a program on this computer. A connection that runs a program is marked so and is never installed without you.

## Channels

```
projects/<slug>/channels/<id>/channel.yaml
  id, label, extension, kind, destination {…}, created_by, created_at, asks_first, publications
```

Create one on the Channels page, with `kiqr channels <slug>` to list, or let Seemo propose one (`create_channel`, confirmed on a card). Publish with the asset page's Publish button, `kiqr publish <slug> <type>/<id> <channel>`, or Seemo's `publish_asset`, which always opens a card first. A task run that reaches a publish gets the card in the task's chat and waits.

Email is the built-in channel kind (`kiqr-smtp`): `SMTP_HOST`, `SMTP_USER`, `SMTP_PASSWORD` (a Gmail app password), optional `SMTP_PORT`, `SMTP_FROM`, `SMTP_SECURITY` (`starttls`, `tls`, `none`).
