# Abilities, credentials and channels

Three things a user sees, and the one folder format behind abilities.

- **Ability**: something Seemo can do. KIQR's core ones are code (write, crawl a site into a business profile, read the web, read blocked pages with UnblockingAPI, write social media posts, run tasks on a schedule, create images with the built-in renderer); every other ability is one folder, holding **instructions** (`SKILL.md`: how Seemo does this kind of work, in words the user can edit), a **connection** (`extension.yaml`: what it provides, the hosts it works on and the credentials it would like), or both.
- **Credential**: a login, an API key, an app password, a signed-in app or a mail server in the **Vault**, machine-wide, scoped to the domains it is for. It knows nothing about abilities, channels or projects.
- **Channel**: somewhere the business's people are, owned or third-party, with what Seemo may do there and who it posts as (its publisher, a credential).

Seemo only ever sees tools, named after ability contracts, whatever installed ability is behind them, plus the list of abilities that come with instructions (`id — name: when it applies`) in its system prompt, which it reads with `read_instructions` before acting. Transport words (HTTP, MCP, command) stay in the manifest and under Advanced. The marketplace index (`config.marketplace.url`, `{ "abilities": [ { id, name, author, summary, version, repo, path, categories, icon, cost, provides, kinds, service, auth, trust, transport } ] }`) lists repositories to clone, optionally a `path` inside one (the marketplace repository holds its own abilities as folders); a folder with an `extension.yaml` or a `SKILL.md` in a public git repository is all a community ability needs (`transport: instructions` in the index marks one that is instructions only). The vendor shown on the card is `author`, else the GitHub user or organisation in `repo`; KIQR's own are `kiqr` (github.com/kiqr). Installing copies the folder into `~/.kiqr/abilities/<id>` (committed, with `.source` remembering the repository and path); Update fetches the repository again.

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

Install from Setup → Abilities (the marketplace, or Advanced for a folder or git URL), or with `kiqr abilities install <id>`; then allow it a credential from the Vault on its Access tab, or with `kiqr access allow <slug> <ability> <credential>`. `kiqr abilities ls|show|teach|update|pause|resume|rm`. Only the core folders (the renderer) install themselves; a shipped `extension.yaml` follows each release once installed, and shipped instructions follow too unless you edited them (`.builtin` beside the file remembers what was installed; the old copy is kept as `SKILL.previous.md`). A project's copy of shipped instructions (`projects/<slug>/abilities/<folder>/SKILL.md`) wins for that project; the Instructions tab of the ability's modal edits and drops copies. Homes from before the rename had these folders under `skills/`; the daemon moves them once and commits.

Switches: an ability is switched on and off per project, in its `project.yaml` (`abilities_off`); that is the only switch an installed ability has. A core ability can also be off for every project in `config.yaml` (`core_off`); switching it on in a project clears that. Off means Seemo does not see its tools, its instructions leave the prompt and `read_instructions` refuses; a core ability off takes its tools with it (writing: `create_asset`; reading the web: `read_url`; tasks: `create_task` and the scheduler; the posts job refuses). An ability that needs a credential cannot be switched on before the project allows it a complete one.

## Credentials, access and channels

An installed ability holds no keys and owns no credential. It says what it would like (`credentials:` in its manifest); the user keeps credentials in the **Vault** and decides, per project, which ability may use which.

```
~/.kiqr/vault/vault.enc                    every credential, encrypted (XChaCha20-Poly1305); never committed
~/.kiqr/vault/sessions/<credential>/       a login's browser profile and status.json; never committed
~/.kiqr/vault/activity/<credential>.jsonl  every action as it, in every project; never committed
projects/<slug>/access.yaml                committed: ability → [credential ids] it may use here
projects/<slug>/channels/<id>/channel.yaml committed: label, ownership, domain, kind, fields, url, publisher, ability, read, publish, reply, learn
projects/<slug>/channels/<id>/contacts.csv a list's people, and sends/: never committed
```

- **A credential** is a template's fields (`login`: username, password, two-factor key; `api_key`; `app_password`; `oauth`: token, refresh, expiry, scopes; `smtp`; or `custom` fields) scoped to one or more domains, the first its icon. Values go in and never come back out of the API. A login's password and two-factor key are **runner-only**: only the sign-in runner types them; no call, no template and no manifest ever gets them.
- **A need** (an entry of `credentials:`) names the templates that fit, the `hosts` a fitting credential is for (the ability's `hosts` when left out; a `${SITE}` filled from the project's settings), a kiqr.cloud `provider` and `scopes` for a signed-in app, what it is `for`, whether it is `optional`, and `env`: which field a call gets under which name (`FAL_KEY: key`; `token|key` takes the first that is set). A credential fits when its template is one of them, one of its domains is on the same site as one of the hosts (a mail server fits on its template alone), and for a signed-in app it has every scope.
- **Access.** A project grants a credential to an ability (`POST /api/projects/{slug}/access/grant {ability, credential}`; the ability's Access tab; `kiqr access allow`). Picking a channel's publisher grants it to the ability that publishes there; that is the only grant the user does not make by hand, and the channel says so. Deleting a credential takes it out of every project's grants.
- **A call** goes through the credentials the project granted the ability, one per need: the one the call names (`credential`, only one that was granted; the tool gets an enum of their names when there are several), else the one its settings name, else the first that fits. A required need with nothing granted refuses the call with what to do. The call sees only the names its needs map (`conn::env_for`), the project's settings (`${PROPERTY}`) and KIQR's own keys (model providers, UnblockingAPI); never anything else in `.env`.
- **A credential's value goes only to its own domains.** A request that names one of its values (`${FAL_KEY}` anywhere in it) to a host outside that credential's domains is refused, whatever the manifest's `hosts` allow. A manifest cannot widen a credential's domains; only the user edits them.
- **One rule for whether a credential works** (`conn::State`): its required fields, then the program its sign-in runs on, then a run-out token, then the sign-in: `ready`, `missing_fields`, `needs_program`, `expired`, `signed_out`, `needs_hand`, `checking`.
- **Sessions, turns, the log and the cap are the credential's, machine-wide.** A hosted program runs once per credential and program, in `vault/sessions/<id>/` (`${KIQR_ACCOUNT_DIR}`), so two projects never drive one login's browser at once. `exclusive: true` gives one call at a time per credential. Every action (a call whose role is not `read`) is logged in `vault/activity/<id>.jsonl` with the project, the ability, the role, the tool, the target, the channel and the link; the same role on the same target is refused the second time, and `daily_cap` counts today's actions in every project. `GET /api/vault/{id}/activity`; Seemo reads it with `credential_activity`.
- **Settings.** What the project picks once for an ability (a site's address, an analytics property, a repository) is in `settings:` (name → {label, help, placeholder, optional, enum, choices}), kept in `project.yaml` (`ability_settings`) with the credential its calls go through; the values fill its calls (`{{property}}`, or `${PROPERTY}` in templates and `hosts`). A setting with `choices` is picked from a list read through that credential.
- **Recommendations.** `GET /api/projects/{slug}/recommendations?credential=` lists, per credential, the abilities whose needs fit it: allowed here, installed, or in the marketplace (the index carries each item's `hosts` and `credentials`). The Abilities page shows "Recommended for your credentials"; a credential shows the abilities that can use it.

**Channels** are everywhere the business's people are, owned (a profile, a page, a blog, a mailing list) or third-party (a subreddit, a forum). A channel is on a site (`domain`) and may name a `kind` an ability declares (or one is found from its site and link once an ability that knows it is on); a kind's `target` fields name a spot (`subreddit: selfhosted`), read from a link through `target_from`, or the profile itself is the spot (`account_is_target`). It holds what Seemo may do there (`read` on/off, `publish` and `reply` off / ask / auto, `learn` for owned ones) and its `publisher`, a credential. The executor resolves the channel a call means (`channels::spot_of`): `off` refuses, `ask` waits for the card's yes, `auto` goes at once with the card as the record; a spot that is no channel always asks and the yes adds it. A channel shows its one missing thing with the fix (`readiness`: no ability for its site, with the marketplace item to install; no publisher; the publisher not working). The crawl adds the business's own profiles as owned channels; Seemo suggests others with `suggest_channel` and proposes new ones with `create_channel`.

## SKILL.md

```markdown
---
name: Write product update posts          # the action, as the user says it
description: When asked for a post about something we shipped.   # the trigger: what Seemo sees in the prompt
categories: [writing]        # optional, for the marketplace shelves
icon: book-open              # optional, a lucide icon name
author: acme-digital         # optional, the vendor when shared; empty for your own
abilities: [image]           # optional: contracts it needs; shown as "needs setup" when none is ready
kinds: [wordpress]           # optional
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
credentials:                   # what it would like from the Vault; the project grants one per need (see "Credentials, access and channels")
  - template: app_password     # login | api_key | app_password | oauth | smtp | custom; or a list: [oauth, api_key]
    hosts: [myblog.com]        # the domains a fitting credential is for; the ability's `hosts` when left out
    for: Publish posts on your site
    env: { WP_USER: username, WP_APP_PASSWORD: password }   # name a call gets → the credential's field; never a login's password or two-factor key
    # provider: google         # a signed-in app through kiqr.cloud ("Sign in with Google"), with the `scopes: [...]` it must have
    # optional: true
    # connect: { … }           # its own one-click sign-in (see "Signing in")
settings:                      # optional: what the project picks once for this ability; the values fill the calls as {{name}} and ${NAME}
  property: { label: GA4 property, choices: { request: { method: GET, url: "https://…" }, list: /items/*, value: /id, label: /name } }
hosts: [myblog.com]            # the only hosts an http ability may call (subdomains included); "${WP_SITE}" makes it the project's own site (the setting wp_site)
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
kinds:                         # where content goes out through this ability (see "Publishing: kinds and channels")
  - kind: wordpress
    label: Blog
    section: page              # page and list (owned channels), community (third-party)
    account_is_target: true    # the profile itself is the spot (a blog, an X profile): no target fields
    publish: publish_article   # the tool (or ability) that performs a publish
```

### Networks

A network is a kind; KIQR's code names none. An ability that publishes to a social network says what the rest of KIQR needs to know about it, on the kind, with the hosts its profiles live on in its `service` block (`profile_hosts: [linkedin.com]`: the brand crawl recognises the business's profile there):

```yaml
kinds:
  - kind: linkedin
    label: Page
    section: page
    publish: post_update
    target: { page_id: { label: Page } }   # a person's account reaches the company pages it manages
    name: LinkedIn                   # as people write it
    icon: send                       # a lucide icon name
    text_limit: 3000                 # characters a post may have
    image: { width: 1200, height: 627 }
    preview: feed                    # previewed as a feed card
    writing: "600 to 1,300 characters, short paragraphs, at most three hashtags."
```

With it installed, the posts job writes for that network (within its limit, following `writing`), its card is rendered at its size (`size: linkedin` in `create_image`), the preview shows its name and limit, and a post tagged `linkedin` matches a LinkedIn channel on approval. Seemo writes posts only for the networks its channels are on (every network an ability on here publishes to while there are no channels); with none, the posts work anywhere. The first three posts for a network are written when its first channel is added.

List the same fields (`kind`, `name`, `icon`, `profile_hosts`) under `kinds` in the marketplace index: the brand crawl then finds the business's profile there before anything is installed, and Seemo suggests the ability ("I found your LinkedIn page; the marketplace has …") after the crawl and under To do, until the user installs it or says not now.

### When to suggest it

`fits` tells KIQR when to suggest the ability to a new project, at the end of Seemo's first conversation and in the getting-started steps:

```yaml
fits:
  goals: [customers]                 # the welcome question's goals it serves: customers, repeat, known, launch
  html: ["/wp-content/"]             # text in the site's HTML that shows the business already uses the service; when given, it must match (a goal alone is not enough)
  every_site: false                  # true: worth having for any website (a search console); suggested to every project as "Suggested for all websites"
```

A network the owner names in that conversation matches the ability's `kinds` (kind or name) without any `fits`. Put the same `fits` in the marketplace index entry, so it works before the ability is installed. HTML is matched case-insensitively against the pages the crawl kept; pick text only that service puts there (a script URL, a generator tag), never a common word.

### Signing in

Credentials are added in the Vault: typed into a template's fields, or with **"Sign in with …"**, a kiqr.cloud provider (Google, GitHub) whose app kiqr.cloud holds; the tokens become a signed-in app (`oauth`) for the provider's domains and are renewed through kiqr.cloud at call time. A need with `provider` and `scopes` asks for such a sign-in, and its Access tab offers it in one click; signing in again adds scopes.

A need may also carry its own one-click way to make the credential, for a service whose sign-in no kiqr.cloud provider covers:

```yaml
credentials:
  - template: app_password
    env: { WP_USER: username, WP_APP_PASSWORD: password }
    connect:
      label: Connect with WordPress
      flow: redirect                 # redirect | oauth2
      needs: [WP_SITE]               # settings asked first (here the setting wp_site); the page lives on it
      url: "${WP_SITE}/wp-admin/authorize-application.php?app_name=KIQR&app_id={{app_id}}"
      callback_params: [success_url, reject_url]   # where KIQR's callback URL goes
      callback: https                # the service only calls back over https
      returns: { WP_SITE: site_url, WP_USER: user_login, WP_APP_PASSWORD: password }
      denied: { param: success, equals: "false" }
```

- **redirect**: the browser opens `url`; the service comes back to KIQR's callback (`/api/connect/callback`) with the values as query parameters, mapped by `returns`. `callback: https` means the service refuses a plain-http callback: when KIQR is signed in to kiqr.cloud, a single-use relay there carries it; otherwise the service's page shows the values once and the user types them into the Vault's form.
- **oauth2**: authorization code with PKCE: `authorize_url`, `token_url`, `client_id` (and `client_secret`, usually `${VAR}` settings), `scopes`, extra `params`; the token is mapped by `returns` like the rest.

Names in `needs` and `returns` are the need's `env` names (they become the credential's fields) or settings in capitals (they go to the project's settings: the site). The credential is made for the site the flow names (a returned address), else the need's hosts, and granted to the ability in the project it was started from. `POST /api/projects/{slug}/abilities/{ability}/connect {origin, values}` starts it.

A setting may read its values through the ability's credential (`choices`), so the user picks instead of typing (the analytics property): a request, a pointer to the items (`*` steps into every element of an array), and in each item the value, its label and a prefix to strip.

```yaml
settings:
  property:
    label: GA4 property
    choices:
      request: { method: GET, url: "https://analyticsadmin.googleapis.com/v1beta/accountSummaries?pageSize=200", headers: { Authorization: "Bearer ${GOOGLE_TOKEN}" } }
      list: /accountSummaries/*/propertySummaries/*
      value: /property
      label: /displayName
      strip: "properties/"
```

Every URL's host must be in `hosts` (or come from a `${VAR}` host).

### Placeholders

- `{{arg|json}}`: the argument as JSON (also valid YAML), `null` when it is missing or empty: `title: {{title|json}}` and `tags: {{tags|json}}` in frontmatter, whatever the title holds.
- `{{arg}}` and `{{arg|default}}`: the tool's arguments. A string that is exactly one placeholder takes the argument's own JSON value (numbers stay numbers). A default may hold placeholders of its own: `{{repo|${GITHUB_REPO}}}` (the argument, else the setting), `{{title|{{tag}}}}`.
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

`create_image` has two shapes, and a generator only ever sees one of them: a call with `source` (SVG or HTML) is a design Seemo wrote and renders through the built-in renderer, free, whichever generator the project picked; a call with `prompt` goes to the generator. Real pictures of the business come from the Library's Media (below), never from a generator.

When a publish goes out as a channel's publisher, the publish tool receives `title`, `text` (Markdown), `html`, `summary`, `tags`, `asset` (`type`, `id`; the id is a slug of the title), `date` (YYYY-MM-DD) and `datetime` (RFC 3339) of the publish, `image` (the asset's first image as named in `{{files}}`, when it has one), and the channel's target fields (`subreddit`). Its `result.text` or a `url` in the answer becomes the publication's link.

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
    when: "{{text}}"                   # skipped when it fills to nothing; `unless:` is the opposite
    request: { method: POST, url: "https://api.example.com/posts", json: { text: "{{text}}", media_ids: "{{register.*.id}}" } }
    save: { url: /url }
result: { text: "{{post.url}}" }
```

- What a step saves is `{{name.var}}` for the steps after it; after a `for_each` it is a list (`{{register.0.id}}`), and `*` takes one field from every entry (`{{register.*.id}}` → `["a", "b"]`). A placeholder that is a whole JSON value keeps its type, so lists and numbers go into JSON bodies as they are; a missing one leaves the field out.
- Every step's URL is checked against `hosts` when it is called, also a URL taken from an earlier answer, so list the upload host too. Redirects are not followed.
- `result.text` may be a template over everything saved; a pointer reads the last answer. A failing step says which: `step 'put' (file 2): … answered 413`.
- Saved values last for the one call. At most 10 steps and 20 runs of one `for_each`. There is no branching beyond `when` and `unless` (a photo with a caption `when: "{{files}}"`, a plain message `unless: "{{files}}"`); a service that needs more is a `command` ability.

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

### Media

An ability that reaches the business's own files (a folder, a drive, a bucket) declares `media:`: how to list what is there and how to fetch one item. Its items show under Library → Media, where the user (or Seemo, with `list_media` and `add_media`) brings one into the Library as a `media` asset; `brand_kit` then hands Seemo every picture there as a local file to design with. The website is the one source built in: the pictures the crawl found, fetched only from the business's own hosts. KIQR knows an item's shape (an id, a name, a type, a size, when it changed, a URL that shows it) and nothing about any service.

```yaml
media:
  list:
    request: { method: POST, url: "https://api.example.com/files/list", json: { path: "{{folder}}" } }
    items: /entries                 # the array in the answer; empty: the answer is the array
    id: /id                         # pointers into an entry; id and name are required
    name: /name
    mime: /mime_type                # optional; sniffed from the bytes when missing
    bytes: /size                    # optional
    modified: /server_modified      # optional
    url: /thumbnail_url             # optional: a picture of the item for the picker
  get:
    request: { method: POST, url: "https://content.example.com/files/download", headers: { "X-Path": "{{id}}" } }
    # The answer is the file itself, or a `result` with `file_url` / `file_b64` as for a provided ability.
```

`get` is filled with the item's `id`, `name`, `url` and `mime`. Both calls go through the same executor as any other: the ability's hosts, the credential the project granted it, its settings (`{{folder}}` above). A file lands in the Library once: an item imported twice, or two sources offering the same bytes, is one asset. Pictures under 400 px a side, SVG and GIF are refused.

### Learning

A connection whose channels hold what the product is (a repository's README and docs) declares `learn:`. With learning on for an owned channel of it, KIQR reads those documents through the ability's own tools, extracts each into facts like a page of the site, and writes the profile and the brief again from everything; each fact cites the document's link. It reads them when the switch goes on (or once its credential works), with every crawl, on "Learn now" (`POST …/channels/{id}/learn`), and every week.

```yaml
learn:
  label: Learn the product from its README, docs and releases   # what the switch says
  documents:
    - { tool: gh_readme, title: README, url: "https://github.com/${GITHUB_REPO}#readme" }
    - tool: gh_tree                       # one call, then one per entry of its answer
      args: { path: docs }
      each: { list: "", where: { type: file }, only: [.md, .mdx], value: /path, tool: gh_file, arg: path, max: 20 }
      title: "{{value}}"
      url: "https://github.com/${GITHUB_REPO}/blob/HEAD/{{value}}"
```

- `tool` is one of the ability's tools that only reads; `args` are its arguments. A document that is not there (a 404) is skipped.
- `each` calls `tool` once per entry of the first answer (`list` points at the array; empty: the answer is one), keeping entries whose fields equal `where` and whose `value` ends in one of `only`, at most `max` (20, up to 50), with the value as the argument `arg`.
- `title` and `url` may name settings (`${GITHUB_REPO}`), never a secret, and under `each` `{{value}}`. The text is what the tool said: a `result.text`, a text file (a raw-text answer such as GitHub's `application/vnd.github.raw` is text), else the JSON.

The GitHub manifest in the marketplace is the worked example.

### A hosted program, a sign-in, a live view

An MCP ability may run on another installed ability's program instead of a command of its own: `program: browser` takes that ability's `command`, `args`, `env_set` and `requires`, and one server runs per credential and program, shared by every ability and project on it (one browser profile, never opened twice). `env_set: { NAME: template }` sets the program's environment from the call's names, `${KIQR_ACCOUNT_DIR}` (the credential's session folder, `vault/sessions/<id>/`) and `${KIQR_PROJECT_DIR}`. `requires:` lists programs KIQR installs itself, only when the user says so (the ability's window names them and asks before anything downloads; installing an ability never fetches them on its own) and with the installer's output shown line by line (`program_log` events, `GET /api/programs/log`), pinned: a `version`, a `check` command, and per platform a download (`os`, `arch`, an https `url`, its `sha256`, `archive` zip or tar.gz, `bin`; `unpack: true` keeps the whole archive, a program with its libraries and data, and `bin` is the program's path inside it), or `uv_tool` (a pinned spec installed through the `uv` KIQR installed the same way), or `system` (a package per package manager, shown to the user as the one command for their machine, never run by the daemon; on a Linux desktop the web UI can run it through pkexec, the system's own password dialog). They are tried in that order: a download first, then uv, the system's package as the last word. Downloads go into `~/.kiqr/bin`, which comes first on the daemon's PATH; a checksum that does not match installs nothing. A credential whose sign-in runs on a program that lacks a requirement is not ready (`needs_program`), and the ability says what is missing. Xvfb, the browser's virtual display on Linux, is a system requirement on purpose: the system's package is the right build for the OS; `install.sh` installs it (asks once; `-y` says yes), and the app shows the one command or installs it through pkexec. `requires` is honoured only for abilities from KIQR's own marketplace or shipped in the binary: a manifest the user or Seemo wrote never gets to download and run anything. A tool or a provided ability on a hosted program may be `steps` too: each step `call`s one of the program's tools with `args` (a template over the arguments, `{{file}}`, and what earlier steps saved), with `save`, `when` and `for_each` as for HTTP steps; `url_args` names arguments that are URLs, each checked against `hosts`. `busy_when` on the program lists what it answers while it is still busy with an earlier action (the browser types a long text at a person's pace and refuses other actions meanwhile): a step refused with one of these is made again every few seconds, up to 15 minutes, so a step never fails because the one before it is still going. An acting tool on a browser may instead be a `hand: { goal, start, hints, image }`: the goal and the start page are templates over the call's arguments, `hints` is what a person would tell someone new to the site, and `image` is `required`, `none` or (empty) attached when there is one. A model reaches the goal page by page, choosing `click` (a listed control, by selector or index), `type_approved` (the approved `text`, or `title` when the goal names a title, typed by KIQR), `type_short` (a few words for a picker, never into the content's field), `attach_approved` (the button that opens the file picker, or the file input), `press`, `navigate` (on `hosts`), `wait`, `submit` (KIQR reads the fields back and clicks only when they hold exactly the approved content), `done` (with a link the page shows), `needs_hand` or `fail`. Selectors to go stale stay out of the manifest; `KIQR_HAND_DRY=1` on the daemon stops every hand at its last click.

`session:` says how a login credential signs in on the program and stays so: `check` steps (signed in when the last answer holds `signed_in_when`, or when none of `signed_out_when` shows; with neither, the model reads the page and says), `login_url` where signing in starts (the model reads each page's visible controls from the program's snapshot, in whatever language the site serves, and names the user, password and one-time code fields and the button; the runner types only into controls the page listed, the password only into a `type=password` field, only on the ability's `hosts`, and follows up to six pages; the model never gets a value, and a captcha, a check or a wrong-password notice it sees makes the credential need a hand) or `login` steps that type the credentials into fixed fields (`{{credential.username}}`, `{{credential.password}}`, `{{credential.otp}}`: available here and nowhere else; for a site whose form never changes), `blocked_when` texts that mean a person is needed, `every` (how often the check runs). The login is the credential: its username, password and two-factor key (the one-time code is computed from it) are read for the runner alone (`Caller::Runner`). The runner (`POST /api/projects/{slug}/credentials/{id}/signin`, `kiqr vault signin`, and the scheduler every `every`) holds the credential's turn, checks, types the login when signed out, checks again; what it typed is struck from every program answer, error and event while in use. The session belongs to the credential, machine-wide: the installed ability with a `session` whose need fits it (one for its own site first) signs it in. Blocked or failing, the credential is `needs_hand` (`GET …/credentials/{id}/session`, `CredentialStateChanged`): a badge, a To do item, one Seemo line, and tools that act refuse until a person finished the sign-in. `POST …/signout` stops the program and wipes what it kept (the profile); the credential stays.

`live_view:` names the program's tools a person's live view is built from, each a step: `frame` answers with an image, `metrics` (optional) answers the window's geometry as JSON (`ow`, `oh`, `iw`, `ih`: the window's and the page's width and height in CSS pixels, and `ox`, `oy`: where the page's top-left corner sits in the window, else taken as centred below the bars) so that a click on a frame of the whole window lands on the page, `click` gets `{{px}}`/`{{py}}` (page pixels when `metrics` is there, else pixels of the frame) or `{{x}}`/`{{y}}` (fractions), `key` gets `{{key}}`, `type` gets `{{text}}`, or `{{char}}` to be called once per character (a real keystroke each; a newline is Enter), `navigate` gets `{{url}}` (checked against `hosts`). The input route answers `{ ok, message }`; a refusal is shown to the person. `POST …/credentials/{id}/hands` (a person in the web UI only, like a factory reset) opens a session: the credential's turn is held, frames go out as `HandsFrame` events a few times a second and are never kept, `POST …/hands/{id}/input {token, kind, …}` sends a click, a key, text or a URL, and `POST …/hands/{id}/end {token, signed_in}` closes it and, when the person says they signed in, runs the check. Ten idle minutes end it.

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
- **smtp**: mail through a server. The `smtp` block names it, each value a template (usually `${VAR}`); it serves `provides.email` (one address or several in `to`, or one message per person of a list) with `to`, `subject` (else the title), `text` (Markdown, sent as plain text and HTML) and the asset's files attached.
  ```yaml
  transport: smtp
  smtp: { host: "${SMTP_HOST}", port: "${SMTP_PORT}", user: "${SMTP_USER}", password: "${SMTP_PASSWORD}", from: "${SMTP_FROM}", security: "${SMTP_SECURITY}" }   # security: starttls (default), tls, none
  provides: { email: { cost: free } }
  ```
- **builtin**: Rust inside KIQR, for the core renderer only (`kiqr-render`).

Every ability's modal shows what it includes: instructions, keys (and where each is set), hosts it may call, the program or MCP server it runs, its tools, where it publishes, what it provides. One that runs a program is marked so and is never installed without you.

## Publishing: kinds and channels

A kind says where content goes out through an ability: a `section` (`page` or `list`, owned channels; `community`, third-party), the `target` fields that name a spot (`subreddit`, with `label`, `help`, `placeholder`, `optional`, `enum`, `pattern`), `target_from` (how a URL argument names them: `post_url: "reddit.com/r/{subreddit}/"`), or `account_is_target` (the profile itself is the spot: an X profile, a blog), a `url` template for a spot's link (its host is the kind's site, which channels are matched on), and the `publish` tool. A kind that publishes, is no network (no `text_limit`, no feed preview), is a `page` and has no `url` publishes **a website**, wherever it is: a website channel (core's kind `website`) picks the ability as how it is published, and holds the kind's `target` fields (`repo` for a repository that builds the site). A target field may have `choices` like a setting's, read through the channel's publisher. In the index, mark such a kind `site: true` so the marketplace can offer it before it is installed. Such an ability keeps nothing about the site: a call made in a website channel gets the channel's address as `${SITE_URL}` (`hosts: ["${SITE_URL}"]`, `url: "${SITE_URL}/wp-json/…"`), the credentials that fit are those for the channel's domain, and a `connect` whose `needs` is `[SITE_URL]` is started from the channel, its credential becoming the channel's publisher.

A kind without `publish` is one an ability only reads or replies in:

```yaml
kinds:
  - kind: reddit
    label: Subreddit
    section: community
    publish: post
    url: "https://www.reddit.com/r/{{subreddit}}"
    target: { subreddit: { label: Subreddit, pattern: "^[A-Za-z0-9_]{2,21}$" } }
    target_from: { post_url: "reddit.com/r/{subreddit}/" }
```

The ability that publishes to a channel is the one on here whose kind it is (by its `kind`, else its site and link), or the one the channel pins (`ability`). It publishes as the channel's `publisher`, a credential that fits the ability's need. Publish with the asset's buttons (pick a channel, or "Somewhere else…" for a new spot, which becomes a channel with the click), `kiqr publish <slug> <type>/<id> <channel>`, or Seemo's `publish_asset` (`target`: a channel; or `spot`: `{kind, fields, credential}`), which opens a card unless the channel's `publish` is `auto`; `off` is refused. The card carries the target and the publish runs what it showed. A task run that reaches a publish gets the card in the task's chat and waits. `asset.target` remembers the channel it was approved for; a `Publication` keeps the `channel`, the `credential` and the `ability`.

**Email.** A mail server (an `smtp` credential) is the sender, used by whichever ability sends mail: the marketplace's "Send email from your mail account" (`transport: smtp`, its need maps `SMTP_HOST`, `SMTP_PORT`, `SMTP_USER`, `SMTP_PASSWORD`, `SMTP_FROM`, `SMTP_SECURITY` to the credential's fields), or an HTTP sender with `provides.email`. `send_email` goes to `to` (addresses, always asking) or to a `list` channel, from its publisher. A send to a list is one message per person, in the background: the list's send log (`channels/<id>/sends/<key>.jsonl`, never committed) remembers who got it, so a stopped run picks up where it left and the same subject never goes to one person twice; every message ends with why it came and how to leave; the credential's activity gets one `publish` entry for the whole send; `ListSending` events say how far it got.

**Lists** are channels of kind `list` with `contacts.csv` (email, name, consent, added, unsubscribed; never committed, mode 0600, so personal data stays on the machine and out of history). `POST /api/projects/{slug}/channels {kind: list, label}`, `…/channels/{id}/contacts` (`GET`, `POST {contacts, consent}`, `DELETE {email}`), `…/import {csv, consent}`, `…/unsubscribe {email}`, `…/sends`; `kiqr lists ls|add|import|unsubscribe|rm`. Seemo sees counts, never addresses (`list_channels`), and proposes a new, empty list with `create_channel` (a card); the user adds people on the list's page.
