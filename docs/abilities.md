# Abilities and channels

The two words a user sees, and the one folder format behind them.

- **Ability**: something Seemo can do. KIQR's core ones are code (write, crawl a site into a business profile, read the web, read blocked pages with UnblockingAPI, write social media posts, run tasks on a schedule, create images with the built-in renderer); every other ability is one folder, holding **instructions** (`SKILL.md`: how Seemo does this kind of work, in words the user can edit), a **connection** (`extension.yaml`: a service behind a key, a program on this computer, or an MCP server, providing a contract such as `create_image` or a few tools of its own), or both ("Publish to WordPress": the call and the instructions for it). Abilities are installed once for the machine, from the marketplace under Setup → Abilities, and then switched on, connected (keys) and taught (copies of the instructions) per project. The user's own, written on that page in plain words, are abilities too. Core abilities can be switched off, never removed.
- **Channel**: where a project's content goes out: an email address, a blog, an account. A channel names an installed ability that publishes plus a destination, and the account behind it is set up inside the project. Publishing always asks the user first.

Seemo only ever sees tools, named after ability contracts, whatever installed ability is behind them, plus the list of abilities that come with instructions (`id — name: when it applies`) in its system prompt, which it reads with `read_instructions` before acting. Transport words (HTTP, MCP, command) stay in the manifest and under Advanced. The marketplace index (`config.marketplace.url`, `{ "abilities": [ { id, name, author, summary, version, repo, path, categories, icon, cost, provides, channels, auth, trust, transport } ] }`) lists repositories to clone, optionally a `path` inside one (the marketplace repository holds its own abilities as folders); a folder with an `extension.yaml` or a `SKILL.md` in a public git repository is all a community ability needs (`transport: instructions` in the index marks one that is instructions only). The vendor shown on the card is `author`, else the GitHub user or organisation in `repo`; KIQR's own are `kiqr` (github.com/kiqr). Installing copies the folder into `~/.kiqr/abilities/<id>` (committed, with `.source` remembering the repository and path); Update fetches the repository again.

## The folder

```
~/.kiqr/abilities/<id>/                       everywhere
~/.kiqr/projects/<slug>/abilities/<id>/       this project only (wins on a name clash)
  SKILL.md          instructions: frontmatter + the text Seemo follows   (optional)
  extension.yaml    a connection: what it provides and needs            (optional)
  templates/…       files it ships                                       (optional)
  avatar.svg        a 64×64 mark shown on its card and in the list; `avatar: avatar.svg` in the manifest  (optional)
```

The ability's id is the manifest's `id`, else the folder name. A folder with only `SKILL.md` is instructions (what the user writes with "Teach Seemo something", or a shared recipe from the marketplace). With only `extension.yaml` it is a connection. With both it is one ability with instructions and a connection, such as "Publish to WordPress".

Install from Setup → Abilities (the marketplace, or Advanced for a folder or git URL), or with `kiqr abilities install <id>` (`--project <slug>` puts the keys you type into that project's `.env`). `kiqr abilities ls|show|teach|update|pause|resume|rm`. Only the core folders (the renderer) install themselves; a shipped `extension.yaml` follows each release once installed, and shipped instructions follow too unless you edited them (`.builtin` beside the file remembers what was installed; the old copy is kept as `SKILL.previous.md`). A project's copy of shipped instructions (`projects/<slug>/abilities/<folder>/SKILL.md`) wins for that project; the Instructions tab of the ability's modal edits and drops copies. Homes from before the rename had these folders under `skills/`; the daemon moves them once and commits.

Switches: a core ability is switched off for the machine in `config.yaml` (`core_off`), an installed folder with an `.off` marker in it; a project switches any ability off for itself in its `project.yaml` (`abilities_off`). Off means Seemo does not see its tools, its instructions leave the prompt and `read_instructions` refuses; a core ability off takes its tools with it (writing: `create_asset`; reading the web: `read_url`; tasks: `create_task` and the scheduler; the posts job refuses). An ability with keys cannot be switched on without them.

## SKILL.md

```markdown
---
name: Write product update posts          # the action, as the user says it
description: When asked for a post about something we shipped.   # the trigger: what Seemo sees in the prompt
categories: [writing]        # optional, for the marketplace shelves
icon: book-open              # optional, a lucide icon name
author: acme-digital         # optional, the vendor when shared; empty for your own
abilities: [image]           # optional: contracts it needs; shown as "needs setup" when none is ready
channels: [wordpress]        # optional
---

1. Lead with what changed for the customer, in one sentence.
2. One concrete example or number.
3. End with where to try it. No hype words.
```

A core ability may own several instruction files (crawl a website into a business profile: reading the site, extracting facts, building the profile, writing the brief); the modal shows them as tabs and `read_instructions` takes `file` to pick one.

## extension.yaml

```yaml
id: wordpress                  # lowercase letters, digits, dashes
name: Publish articles to WordPress   # the ability's name in the UI: an action
version: 0.1.0
summary: Publish articles to our WordPress site.
homepage: https://example.com/wp-admin
transport: http                # http | command | mcp | builtin
auth:                          # keys by variable name; values live in .env, set from the ability's Connect tab
  - { env: WP_USER, label: User name }
  - { env: WP_APP_PASSWORD, label: Application password, help: https://example.com/wp-admin/profile.php }
hosts: [myblog.com]            # the only hosts an http ability may call (subdomains included); "${WP_SITE}" makes it the project's own site
tools:                         # the ability's own tools, offered under its own group ("Use Publish articles to WordPress")
  - name: publish_article
    label: Publishing to WordPress
    description: Create a post from a title and HTML body.
    input: { type: object, properties: { title: { type: string }, html: { type: string }, status: { type: string } }, required: [title, html] }
    request:
      method: POST
      url: https://myblog.com/wp-json/wp/v2/posts
      basic_auth: ["${WP_USER}", "${WP_APP_PASSWORD}"]   # or headers: { Authorization: "Bearer ${TOKEN}" }
      headers: { Content-Type: application/json }
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

### Signing in

A connection can offer one button instead of typed keys. `connect` names a generic flow, and everything specific to the service stays in the manifest; the values land in the same `.env` variables `auth` declares, so readiness, `${…}` and hosts work as for typed keys, and typing them stays possible.

```yaml
connect:
  label: Connect with WordPress
  flow: redirect                 # redirect | oauth2 | service_account
  needs: [WP_SITE]               # asked first; the page lives on it
  url: "${WP_SITE}/wp-admin/authorize-application.php?app_name=KIQR&app_id={{app_id}}"
  callback_params: [success_url, reject_url]   # where KIQR's callback URL goes
  callback: https                # the service only calls back over https
  returns: { WP_SITE: site_url, WP_USER: user_login, WP_APP_PASSWORD: password }
  denied: { param: success, equals: "false" }
```

- **redirect**: the browser opens `url`; the service comes back to KIQR's callback (`/api/connect/callback`, under the address the UI runs on) with the values as query parameters, mapped by `returns`. `callback: https` means the service refuses a plain-http callback; KIQR then opens the page without one and the user pastes what it shows.
- **oauth2**: authorization code with PKCE. `authorize_url`, `token_url`, `client_id` (and `client_secret` when the provider wants one, both usually `${VAR}`), `scopes`, extra `params`. The access token goes to `token_env` (declared in `auth`), the refresh token to `refresh_env`, the expiry to `<token_env>_EXPIRES_AT`; KIQR refreshes before a call when it is about to run out.
- **service_account**: no browser. `key_file` names the variable holding the path of a JSON key (`client_email`, `private_key`); KIQR signs a JWT for `scopes`, exchanges it at `token_url`, and offers the token to requests as `${<token_env>}` (not declared in `auth`, never stored), minted again when it runs out.

Every URL's host must be in `hosts` (or come from a `${VAR}` host). Variables in `needs` and `returns` must be declared in `auth`.

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
  `expose` offers the server's own tools raw under the ability's group. `provides` maps an ability onto one of them. Image or audio content in a result becomes a file.
- **builtin**: Rust inside KIQR (`kiqr-render`, `kiqr-smtp`).

Every ability's modal shows what it includes: instructions, keys (and where each is set), hosts it may call, the program or MCP server it runs, its tools, where it publishes, what it provides. One that runs a program is marked so and is never installed without you.

## Channels

```
projects/<slug>/channels/<id>/channel.yaml
  id, label, extension, kind, destination {…}, created_by, created_at, asks_first, publications
```

Create one on the Channels page, with `kiqr channels <slug>` to list, or let Seemo propose one (`create_channel`, confirmed on a card). Publish with the asset page's Publish button, `kiqr publish <slug> <type>/<id> <channel>`, or Seemo's `publish_asset`, which always opens a card first. A task run that reaches a publish gets the card in the task's chat and waits.

Email is the built-in channel kind (`kiqr-smtp`): `SMTP_HOST`, `SMTP_USER`, `SMTP_PASSWORD` (a Gmail app password), optional `SMTP_PORT`, `SMTP_FROM`, `SMTP_SECURITY` (`starttls`, `tls`, `none`).
