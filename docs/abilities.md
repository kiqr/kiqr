# Abilities and channels

The two words a user sees, and the one folder format behind them.

- **Ability**: something Seemo can do. KIQR's core ones are code (write, crawl a site into a business profile, read the web, read blocked pages with UnblockingAPI, write social media posts, run tasks on a schedule, create images with the built-in renderer); every other ability is one folder, holding **instructions** (`SKILL.md`: how Seemo does this kind of work, in words the user can edit), a **connection** (`extension.yaml`: a service behind a key, a program on this computer, or an MCP server, providing a contract such as `create_image` or a few tools of its own), or both ("Publish to WordPress": the call and the instructions for it). Abilities are installed once for the machine, from the marketplace under Setup → Abilities, and that only puts the code there; each project then switches it on, adds its own **accounts** of it (keys, see Accounts below) and teaches it (copies of the instructions). The user's own, written on that page in plain words, are abilities too. Core abilities can be switched off, never removed.
- **Channel**: where a project's content goes out: an email address, a blog, an account. A channel is one account of an ability that publishes, plus a destination; a project can have as many as it wants of each kind (two blogs are two channels, each with its own keys). Publishing always asks the user first.

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

Install from Setup → Abilities (the marketplace, or Advanced for a folder or git URL), or with `kiqr abilities install <id>`; then add a project's account with `kiqr accounts add <slug> <id>`. `kiqr abilities ls|show|teach|update|pause|resume|rm`. Only the core folders (the renderer) install themselves; a shipped `extension.yaml` follows each release once installed, and shipped instructions follow too unless you edited them (`.builtin` beside the file remembers what was installed; the old copy is kept as `SKILL.previous.md`). A project's copy of shipped instructions (`projects/<slug>/abilities/<folder>/SKILL.md`) wins for that project; the Instructions tab of the ability's modal edits and drops copies. Homes from before the rename had these folders under `skills/`; the daemon moves them once and commits.

Switches: an ability is switched on and off per project, in its `project.yaml` (`abilities_off`); that is the only switch an installed ability has. A core ability can also be off for every project in `config.yaml` (`core_off`); switching it on in a project clears that. Off means Seemo does not see its tools, its instructions leave the prompt and `read_instructions` refuses; a core ability off takes its tools with it (writing: `create_asset`; reading the web: `read_url`; tasks: `create_task` and the scheduler; the posts job refuses). An ability with keys cannot be switched on before the project has an account of it with all of them.

## Accounts

An installed ability holds no keys. A project adds **accounts** of it (the code calls them connections), each with a name and its own keys:

```
projects/<slug>/connections/<id>/connection.yaml   committed: id, label, ability, service, default, created_by, created_at
                                                   and for a channel: kind, destination, asks_first, publications, last_published
projects/<slug>/connections/<id>/.env              the keys, never committed, mode 0600
```

- An account is an account of a **service**: the manifest's `service`, else the ability's own id. Every installed ability that names the same service uses the same accounts (one sign-in for "Read posts", "Reply to posts" and "Improve the profile"), so abilities sharing a service must agree on its key names. `ability` on the record is the one it was made with; a record without `service` (older homes) belongs to that ability's own id.
- A call goes through one account: the one named, else the service's **default** (the first one added, or the one the user starred). The names an ability declares (`auth`, what `connect` writes, `env`, a command's `env`) are read from that account only, never from the project's or the machine's `.env`; KIQR's own keys (the model providers, UnblockingAPI) stay in `.env` as before.
- With more than one account of a service in a project, its tools get an optional `connection` argument (an enum of the account names), and `send_email` takes `from`. Seemo leaves it out for the default.
- A hosted program (`transport: mcp`) runs once per ability and account, started with that account's keys; changing a key on an account or removing it stops every ability's program for it. It works in the account's own folder, `connections/<id>/data/` (never committed, removed with the account), which every ability of the service shares: `KIQR_ACCOUNT_DIR` in its environment and `${KIQR_ACCOUNT_DIR}` in its `args` (`--user-data-dir ${KIQR_ACCOUNT_DIR}/profile` keeps one browser login for all of them). A manifest's own `cwd` still wins.
- Abilities sharing an account do not get in each other's way:

  ```yaml
  exclusive: true          # one call at a time per account, across the service's abilities that say so (a browser session)
  daily_cap: 20            # actions per account per day, across every ability of the service; the lowest declared wins
  tools:
    - name: reply
      role: reply          # publish | reply | manage | read; empty is read, or publish for a channel kind's publish tool
      target: "{{post_url}}"   # the thing acted on: the same role on the same target through one account is refused the second time
  ```

  Every action (a call whose role is not `read`) is logged on the account, `connections/<id>/activity.jsonl` (never committed): when, which ability, the role, the tool, the target and the link it returned. The cap counts today's (UTC) actions there. `GET /api/projects/{slug}/connections/{id}/activity` lists them, newest first, with today's count and the cap.
- A channel is an account with a `kind`. A new one can start with the keys of another account of the same ability (`same_as`: the same mail account to another address); the keys are copied, so changing one later does not change the other.
- `kiqr accounts ls|add|set|default|rm`; routes under `/api/projects/{slug}/connections`, channels under `/channels` as before.
- Homes from before accounts are moved once at start: each channel keeps its id with a copy of its ability's keys, an ability a project had keys for becomes its first account (a channel when its kind needs no destination), the ability's keys leave the project's `.env`, and an ability switched off for the machine becomes switched off in every project. Installing an ability later does the same for keys a project's `.env` still holds.

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
auth:                          # keys by variable name; values live on the project's accounts; `placeholder` shows what a value looks like inside the empty field, `help` sits under it (where to get it), `options: [a, b]` makes it a choice from a list
  - { env: WP_USER, label: User name }
  - { env: WP_APP_PASSWORD, label: Application password, help: https://example.com/wp-admin/profile.php }
service: wordpress             # optional: abilities naming the same service share its accounts (and agree on its key names); default: the id
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

### Networks

A network is a channel kind; KIQR's code names none. An ability that publishes to a social network says what the rest of KIQR needs to know about it, on the channel kind:

```yaml
channels:
  - kind: linkedin
    label: LinkedIn page
    publish: post_update
    name: LinkedIn                   # as people write it
    icon: send                       # a lucide icon name
    profile_hosts: [linkedin.com]    # the brand crawl recognises the business's profile there
    text_limit: 3000                 # characters a post may have
    image: { width: 1200, height: 627 }
    preview: feed                    # previewed as a feed card
    writing: "600 to 1,300 characters, short paragraphs, at most three hashtags."
```

With it installed, the posts job writes for that network (within its limit, following `writing`), its card is rendered at its size (`size: linkedin` in `create_image`), the preview shows its name and limit, and a post tagged `linkedin` matches a LinkedIn channel on approval. Seemo writes posts only for the networks a project can publish to; with none, the posts work anywhere.

List the same fields (`kind`, `name`, `icon`, `profile_hosts`) under `channels` in the marketplace index: the brand crawl then finds the business's profile there before anything is installed, and Seemo suggests the ability ("I found your LinkedIn page; the marketplace has …") after the crawl and under To do, until the user installs it or says not now.

### Signing in

A connection can offer one button instead of typed keys. `connect` names a generic flow, and everything specific to the service stays in the manifest; the values land in the variables `auth` declares, in the `.env` of the one account being signed in (a new one is added as the flow starts), so readiness, `${…}` and hosts work as for typed keys, and typing them stays possible.

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

- **redirect**: the browser opens `url`; the service comes back to KIQR's callback (`/api/connect/callback`, under the address the UI runs on) with the values as query parameters, mapped by `returns`. `callback: https` means the service refuses a plain-http callback. When KIQR is signed in to kiqr.cloud, the callback is a single-use relay there (`https://kiqr.cloud/r/<id>`) that sends the browser straight on to KIQR, and nothing is pasted; otherwise KIQR opens the page without a callback and the user pastes what it shows.
- **oauth2**: authorization code with PKCE. `authorize_url`, `token_url`, `client_id` (and `client_secret` when the provider wants one, both usually `${VAR}`), `scopes`, extra `params`. The access token goes to `token_env` (declared in `auth`), the refresh token to `refresh_env`, the expiry to `<token_env>_EXPIRES_AT`; KIQR refreshes before a call when it is about to run out.
- **service_account**: no browser. `key_file` names the variable holding the path of a JSON key (`client_email`, `private_key`); KIQR signs a JWT for `scopes`, exchanges it at `token_url`, and offers the token to requests as `${<token_env>}` (not declared in `auth`, never stored), minted again when it runs out.

Every URL's host must be in `hosts` (or come from a `${VAR}` host). Variables in `needs` and `returns` must be declared in `auth`.

### Placeholders

- `{{arg}}` and `{{arg|default}}`: the tool's arguments. A string that is exactly one placeholder takes the argument's own JSON value (numbers stay numbers).
- `${NAME}`: a variable from `.env`, filled by the daemon at call time. The model never sees values. `${NAME|default}` uses the default when the variable is not set; `${NAME|urlencode}` encodes the value for a place in a URL path.
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

### Uploading files

A call about an asset can send the asset's files (its image, video or attachments; not its text). Name them in a template and the daemon reads them from disk when it sends:

- `{{files}}`: the list, `[{name, mime, size}]` (with `base64` added when the manifest mentions `base64`, for APIs that take JSON only); `{{files.0}}` the first.
- `{{file}}`: the current file inside a `for_each` (see Steps).

A request's body is one of:

```yaml
json: { title: "{{title}}" }                         # JSON (as before)
form: { status: "{{text}}" }                          # URL-encoded form
multipart:                                            # parts with a value or a file
  - { name: caption, value: "{{text}}" }
  - { name: media, file: "{{files.0}}", filename: "{{files.0.name}}", mime: image/png }
body_file: "{{files.0}}"                              # one file's bytes; its type is the Content-Type unless a header sets one
```

### Steps

When one call is not enough (register an upload, send the bytes, then create the post with the id), a tool or a provided ability lists `steps` instead of `request`:

```yaml
steps:
  - name: register                     # lowercase letters, digits, _
    for_each: files                    # once per file: {{file}}, {{index}}
    request: { method: POST, url: "https://api.example.com/uploads", json: { size: "{{file.size}}" } }
    save: { upload: /uploadUrl, id: /id }        # pointers into the answer; empty: the whole answer
  - name: put
    for_each: register                 # once per entry of an earlier step: {{item}}, and {{file}} at the same position
    request: { method: PUT, url: "{{item.upload}}", body_file: "{{file}}" }
  - name: post
    when: "{{text}}"                   # skipped when it fills to nothing
    request: { method: POST, url: "https://api.example.com/posts", json: { text: "{{text}}", media_ids: "{{register.*.id}}" } }
    save: { url: /url }
result: { text: "{{post.url}}" }
```

- What a step saves is `{{name.var}}` for the steps after it; after a `for_each` it is a list (`{{register.0.id}}`), and `*` takes one field from every entry (`{{register.*.id}}` → `["a", "b"]`). A placeholder that is a whole JSON value keeps its type, so lists and numbers go into JSON bodies as they are; a missing one leaves the field out.
- Every step's URL is checked against `hosts` when it is called, also a URL taken from an earlier answer, so list the upload host too. Redirects are not followed.
- `result.text` may be a template over everything saved; a pointer reads the last answer. A failing step says which: `step 'put' (file 2): … answered 413`.
- Saved values last for the one call. At most 10 steps and 20 runs of one `for_each`. There is no branching beyond `when`; a service that needs more is a `command` ability.

The marketplace's WordPress manifest is the worked example: the asset's image goes into the media library first and becomes the post's featured image.

### Reading numbers

A connection that reads analytics declares the sources it serves (`traffic`, `search`, `conversions`, `ranks`, `mentions`), each with its requests and where in the answers the rows are. The daemon makes the requests once a day (and on Refresh) through the same executor as any call, with `{{from}}`, `{{to}}` (YYYY-MM-DD) and `{{days}}` for the last 30 days, and keeps the rows as a snapshot.

```yaml
sources:
  traffic:
    requests:
      - request: { method: POST, url: "https://…/runReport", headers: { Authorization: "Bearer ${TOKEN}" }, json: { from: "{{from}}", to: "{{to}}" } }
        rows: /rows                        # the array of rows in the answer
        date: /dimensionValues/0/value     # a pointer into a row, or a template ("{{to}}") for a list over the window
        dims: { channel: /dimensionValues/1/value }
        metrics: { sessions: /metricValues/0/value, key_events: /metricValues/1/value }
        totals: [sessions, key_events]     # summed per day as the day's totals (default: every metric)
        breakdown: [sessions]              # kept per dimension (default: every metric, when there are dims)
        skip_zero: true                    # leave out breakdown rows that are 0
        scale: { ctr: 100 }
    derive:
      - { metric: organic_sessions, sum: sessions, where: { channel: Organic Search } }
      - { metric: conversion_rate, ratio: [key_events, sessions], scale: 100 }
```

Numbers may be numbers or numeric strings; days `YYYYMMDD` or `YYYY-MM-DD`. Use KIQR's metric names so the plan and the Performance page understand them: `sessions`, `users`, `pageviews`, `organic_sessions`, `key_events`, `conversion_rate`, `clicks`, `impressions`, `ctr`, `average_position`. The marketplace's Google Analytics, Search Console and Plausible manifests are worked examples.

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
- **smtp**: mail through a server. The `smtp` block names it, each value a template (usually `${VAR}`); it serves `provides.email` and email channels with `to`, `subject` (else the title), `text` (Markdown, sent as plain text and HTML) and the asset's files attached.
  ```yaml
  transport: smtp
  smtp: { host: "${SMTP_HOST}", port: "${SMTP_PORT}", user: "${SMTP_USER}", password: "${SMTP_PASSWORD}", from: "${SMTP_FROM}", security: "${SMTP_SECURITY}" }   # security: starttls (default), tls, none
  provides: { email: { cost: free } }
  ```
- **builtin**: Rust inside KIQR, for the core renderer only (`kiqr-render`).

Every ability's modal shows what it includes: instructions, keys (and where each is set), hosts it may call, the program or MCP server it runs, its tools, where it publishes, what it provides. One that runs a program is marked so and is never installed without you.

## Channels

A channel is an account with a kind (see Accounts): `projects/<slug>/connections/<id>/`, with its own keys.

Several abilities may work on one channel: every ability of the channel's service that declares its kind, each with what it does there (`does` on the kind: `publish`, `read`, `reply`, `manage`; `publish` when left out, which then needs a `publish` tool). One that only reads or replies leaves out `publish`:

```yaml
service: demo
channels:
  - { kind: demo-page, label: Demo page, destination: { page: { label: Page } }, does: [read, reply] }
```

Exactly one ability publishes to a channel: its `publisher` (`connection.yaml`, set with `PUT …/channels/{id} {publisher}` from those that can), else the ability it was made with, else the first that is on here. A call any ability of the service makes through a channel (its tool's `connection` argument names it) gets the channel's destination fields for what the call left out. The channel view lists `publisher`, `publishers` and `used_by` (each ability with what it does there); the kinds a new channel can use come once per service and kind, with the abilities behind it.

Create one on the Channels page or in the ability's Channels tab, with `kiqr channels <slug>` to list, or let Seemo propose one (`create_channel`, confirmed on a card). Publish with the asset page's Publish button, `kiqr publish <slug> <type>/<id> <channel>`, or Seemo's `publish_asset`, which always opens a card first. A task run that reaches a publish gets the card in the task's chat and waits.

Email is a channel kind like any other, declared by whichever ability sends mail: the marketplace's "Send email from your mail account" (`transport: smtp`, keys `SMTP_HOST`, `SMTP_USER`, `SMTP_PASSWORD` (a Gmail app password), optional `SMTP_PORT`, `SMTP_FROM`, `SMTP_SECURITY`), or an HTTP sender with `provides.email`.
