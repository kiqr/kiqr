# Abilities, accounts and lists

The two words a user sees, and the one folder format behind them.

- **Ability**: something Seemo can do. KIQR's core ones are code (write, crawl a site into a business profile, read the web, read blocked pages with UnblockingAPI, write social media posts, run tasks on a schedule, create images with the built-in renderer); every other ability is one folder, holding **instructions** (`SKILL.md`: how Seemo does this kind of work, in words the user can edit), a **connection** (`extension.yaml`: a service behind a key, a program on this computer, or an MCP server, providing a contract such as `create_image` or a few tools of its own), or both ("Publish to WordPress": the call and the instructions for it). Abilities are installed once for the machine, from the marketplace under Setup → Abilities, and that only puts the code there; each project then switches it on, adds its own **accounts** of it (keys, see Accounts below) and teaches it (copies of the instructions). The user's own, written on that page in plain words, are abilities too. Core abilities can be switched off, never removed.
- **Account**: who Seemo signs in as on a service: a Reddit login, a Google account, a mail sender, a blog. A project has as many as it wants (two blogs are two accounts, each with its own keys), each saying what Seemo may do there. An account never says where content goes.
- **Audience**: everywhere the business's people are, each reached through an account: its **pages** (an X profile, a company page, a blog), the **communities** it takes part in (a subreddit) and the **lists** it emails. Publishing goes to an Audience entry; Seemo acts on its own only there, and asks anywhere else.

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

Install from Setup → Abilities (the marketplace, or Advanced for a folder or git URL), or with `kiqr abilities install <id>`; then add a project's account with `kiqr accounts add <slug> <service>`. `kiqr abilities ls|show|teach|update|pause|resume|rm`. Only the core folders (the renderer) install themselves; a shipped `extension.yaml` follows each release once installed, and shipped instructions follow too unless you edited them (`.builtin` beside the file remembers what was installed; the old copy is kept as `SKILL.previous.md`). A project's copy of shipped instructions (`projects/<slug>/abilities/<folder>/SKILL.md`) wins for that project; the Instructions tab of the ability's modal edits and drops copies. Homes from before the rename had these folders under `skills/`; the daemon moves them once and commits.

Switches: an ability is switched on and off per project, in its `project.yaml` (`abilities_off`); that is the only switch an installed ability has. A core ability can also be off for every project in `config.yaml` (`core_off`); switching it on in a project clears that. Off means Seemo does not see its tools, its instructions leave the prompt and `read_instructions` refuses; a core ability off takes its tools with it (writing: `create_asset`; reading the web: `read_url`; tasks: `create_task` and the scheduler; the posts job refuses). An ability with keys cannot be switched on before the project has an account of it with all of them.

## Accounts and the Audience

An installed ability holds no keys. A project adds **accounts** of its service: who Seemo signs in as (a Reddit login, a Google account, a mail sender), each with a name, its keys, where its secrets are kept and what Seemo may do through it. An account never knows where content goes. Where the business's people are is the project's **Audience**: its pages, the communities it takes part in and the lists it emails, each reached through one account.

```
projects/<slug>/accounts/<id>/account.yaml   committed: id, label, service, default, created_by, created_at,
                                             allow (role → auto | ask | off), secrets ({store, item}), last_learned
projects/<slug>/accounts/<id>/.env           keys kept in KIQR and tokens a sign-in got; never committed, mode 0600
projects/<slug>/accounts/<id>/data/          a hosted program's state (a browser profile) and status.json; never committed
projects/<slug>/accounts/<id>/activity.jsonl every action through it; never committed
projects/<slug>/audience/<id>/entry.yaml     committed: id, label, section, kind, account, fields, url, added_by, suggested, reason
projects/<slug>/audience/<id>/contacts.csv   a list's people, and sends/: never committed
```

The user sees three things: **accounts** (Setup → Accounts: who Seemo signs in as), the **Audience** (Marketing → Audience: where the people are) and **abilities** (what Seemo knows how to do). There is no "channel" and no "place" in the UI.

- **One rule for whether an account works** (`accounts::State`): its keys (every ability of its service on here), then the programs they run on, then the sign-in when one signs in on a hosted program: `ready`, `missing_keys`, `needs_program`, `signed_out`, `needs_hand`, `checking`. The account view, the abilities list, the executor, the sign-in runner, the live view, learning, the CLI and the web UI all read it. An account on a hosted program that is not signed in does nothing, reading included (it would read the sign-in page).
- **Permissions.** `allow` says what Seemo may do through the account, by role: `publish`, `reply` and `manage` act, so each is `auto` (on its own; the card is the record), `ask` (the card waits for a yes; the default) or `off`; `read`, `learn` (the business is learned from it, the ability's `learn:`) and `measure` (its numbers are read, the ability's `sources`) never act, so each is on (`auto`) or `off`, on by default. A manifest's `allow:` sets what a new account starts with (only when it differs from those defaults); the user changes it on the account (`PUT …/accounts/{id}/allow {publish: auto}`, `kiqr accounts allow <slug> <account> publish auto`). A role switched off is refused whoever calls. **On its own applies only in the Audience**: an action or a publish whose spot is not an Audience entry always asks, and the user's yes adds the spot.
- **Spots.** A kind declares the `target` fields that name a spot of it (`subreddit`); a call carries them as arguments, or a URL argument names them through `target_from` (a reply's `post_url` is in `r/{subreddit}`), or the account is the spot (`account_is_target`: an X profile, a blog). The executor resolves the spot (`audience::spot_of`), checks its fields (`enum`, `pattern`; values land in URLs), and looks it up in the Audience by account, kind and fields. The card shows it ("In r/homelab via u/acme_dev, not in your Audience yet") and the call runs what the card showed.
- **The Audience.** Entries have a `section` (`page`, `community`, `list`), a `kind` (one a manifest declares, or `list`), the `account` that reaches it and the kind's `fields`; `(account, kind, fields)` is unique. A kind whose account is the spot gets its entry when the account is connected. Seemo suggests entries with `suggest_audience` (`suggested: true` with a `reason`); the user adds or dismisses them on the Audience page. `GET/POST /api/projects/{slug}/audience`, `PUT/DELETE …/audience/{id}` (`{add: true}` accepts a suggestion), `kiqr audience ls|add|accept|rm`. Removing an account leaves its entries, which then say they need an account.
- **Settings.** An ability that works on something the project picks once inside an account (an analytics property, a Search Console site) declares it in `settings:` (name → {label, help, placeholder, optional, enum, choices}). The project keeps the values with the account they were read through (`project.yaml`, `ability_settings`; `PUT …/abilities/{id}/settings {account, values}`); they fill the ability's calls (`{{property}}`, `{{site|urlencode}}`) unless a call names its own, and the account they name is the one its calls and its metrics source use. A field with `choices` is picked from a list read through the account (`GET …/accounts/{id}/choices/{field}`). An ability with a required setting refuses to run until it is picked.
- **Secrets.** `secrets.store` is `kiqr` (the account's `.env`) or the id of a password manager account in the project, with `item` naming the entry there. Logins (`LOGIN_USERNAME`, `LOGIN_PASSWORD`, `LOGIN_TOTP_SECRET`) and typed keys are read from the store at call time through the manager's `credential` contract (`get {ref}` → `username`, `password`, `otp`, `fields`); a token KIQR got from a sign-in is always kept in the `.env`. A login is read by the sign-in runner alone, never by a template.
- An account is an account of a **service**: the manifest's `service`, else the ability's own id. Every installed ability that names the same service uses the same accounts (one sign-in for "Read posts", "Reply to posts" and "Improve the profile"), so abilities sharing a service must agree on its key names.
- A call goes through one account: the one named, else the one the ability's settings name, else the service's **default** (the first one added, or the one the user starred). The names an ability declares (`auth`, what `connect` writes, `env`, a command's `env`) are read from that account only, never from the project's or the machine's `.env`; KIQR's own keys (the model providers, UnblockingAPI) stay in `.env` as before.
- With more than one account of a service in a project, its tools get an optional `connection` argument (an enum of the account names), and `send_email` takes `from`. Seemo leaves it out for the default.
- A hosted program (`transport: mcp`) runs once per ability and account, started with that account's keys; changing a key on an account or removing it stops every ability's program for it. It works in the account's own folder, `accounts/<id>/data/` (never committed, removed with the account), which every ability of the service shares: `KIQR_ACCOUNT_DIR` in its environment and `${KIQR_ACCOUNT_DIR}` in its `args` (`--user-data-dir ${KIQR_ACCOUNT_DIR}/profile` keeps one browser login for all of them). A manifest's own `cwd` still wins.
- Abilities sharing an account do not get in each other's way:

  ```yaml
  exclusive: true          # one call at a time per account, across the service's abilities that say so (a browser session)
  daily_cap: 20            # actions per account per day, across every ability of the service; the lowest declared wins
  tools:
    - name: reply
      role: reply          # publish | reply | manage | read; empty is read, or publish for a kind's publish tool
      target: "{{post_url}}"   # the thing acted on: the same role on the same target through one account is refused the second time
  ```

  Every action (a call whose role is not `read`) is logged on the account, `accounts/<id>/activity.jsonl` (never committed): when, which ability, the role, the tool, the target, the Audience entry it happened in and the link it returned. The cap counts today's (UTC) actions there. `GET /api/projects/{slug}/accounts/{id}/activity` lists them, newest first, with today's count and the cap.

  A tool that acts asks first, unless the account allows its role on its own and its spot is in the Audience. When Seemo calls one, the chat (or the task's chat) gets an `action` card: the tool's label, the ability, the account, where it happens, what it acts on and the arguments; the call runs on the user's yes, through the same turn, cap and log as any call, and a refusal (a repeat, the cap) comes back as the card's outcome. Reading runs at once. The user's own click (`POST …/ext/{ability}/tools/{tool}`) is the yes. A target may name the account's keys (`${GITHUB_REPO}`) but never a secret: tokens, passwords, keys, key files and what a sign-in keeps are left out.
- A sign-in's token may also be typed (`typed: true` on its key): a personal access token instead of signing in, shown as a field.
- A new account from a sign-in is made when the sign-in comes back with its values; one that fails or is abandoned leaves nothing. Unnamed, an account is named after who it is (a site's host, an address), else its service. A new one can start with the keys of another account of the same service (`same_as`); the keys are copied.
- `kiqr accounts ls|add|set|default|rm|activity|allow|login|signin|signout`; routes under `/api/projects/{slug}/accounts`. Accounts in the UI say which abilities use them ("Used by", with what each does) and where they reach in the Audience. Seemo reads the log with `account_activity` before it replies or posts. Every logged action emits `AccountActed`; a refused one emits `ActionRefused` (`why`: `duplicate` or `cap`).

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
auth:                          # keys by variable name; values live on the project's accounts; `placeholder` shows what a value looks like inside the empty field, `help` sits under it (where to get it), `options: [a, b]` makes it a choice from a list
  - { env: WP_USER, label: User name }
  - { env: WP_APP_PASSWORD, label: Application password, help: https://example.com/wp-admin/profile.php }
service: wordpress             # optional: abilities naming the same service share its accounts (and agree on its key names); default: the id.
                               # Or a block, which is how the service shows in the grid of things to connect:
                               # service: { id: wordpress, name: WordPress, icon: newspaper, group: website, profile_hosts: [] }
                               # groups: social, website, email, numbers, passwords, sites, tools
allow: { publish: auto }       # optional: what a new account allows Seemo, by role, when it differs from the defaults (publish, reply, manage ask; read, learn, measure on)
settings:                      # optional: what the project picks once for this ability, through an account of its service; the values fill the calls as {{name}}
  property: { label: GA4 property, choices: { request: { method: GET, url: "https://…" }, list: /items/*, value: /id, label: /name } }
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
kinds:                         # where content goes out through this ability (see "Publishing: kinds and the Audience")
  - kind: wordpress
    label: Blog
    section: page              # page (the business's own), community (it takes part), list (people it emails)
    account_is_target: true    # the account is the spot: connecting it puts the blog in the Audience
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

With it installed, the posts job writes for that network (within its limit, following `writing`), its card is rendered at its size (`size: linkedin` in `create_image`), the preview shows its name and limit, and a post tagged `linkedin` matches a LinkedIn page in the Audience on approval. Seemo writes posts only for the networks the Audience has (every network an ability on here publishes to while the Audience is empty); with none, the posts work anywhere. The first three posts for a network are written when it first joins the Audience.

List the same fields (`kind`, `name`, `icon`, `profile_hosts`) under `kinds` in the marketplace index: the brand crawl then finds the business's profile there before anything is installed, and Seemo suggests the ability ("I found your LinkedIn page; the marketplace has …") after the crawl and under To do, until the user installs it or says not now.

### When to suggest it

`fits` tells KIQR when to suggest the ability to a new project, at the end of Seemo's first conversation and in the getting-started steps:

```yaml
fits:
  goals: [customers]                 # the welcome question's goals it serves: customers, repeat, known, launch
  html: ["/wp-content/"]             # text in the site's HTML that shows the business already uses the service; when given, it must match (a goal alone is not enough)
```

A network the owner names in that conversation matches the ability's `kinds` (kind or name) without any `fits`. Put the same `fits` in the marketplace index entry, so it works before the ability is installed. HTML is matched case-insensitively against the pages the crawl kept; pick text only that service puts there (a script URL, a generator tag), never a common word.

### Signing in

A connection can offer one button instead of typed keys. `connect` names a generic flow, and everything specific to the service stays in the manifest; the values land in the variables `auth` declares, in the `.env` of the one account being signed in (a new one is added as the flow starts), so readiness, `${…}` and hosts work as for typed keys, and typing them stays possible.

```yaml
connect:
  label: Connect with WordPress
  flow: redirect                 # redirect | oauth2 | service_account | cloud
  needs: [WP_SITE]               # asked first; the page lives on it
  url: "${WP_SITE}/wp-admin/authorize-application.php?app_name=KIQR&app_id={{app_id}}"
  callback_params: [success_url, reject_url]   # where KIQR's callback URL goes
  callback: https                # the service only calls back over https
  returns: { WP_SITE: site_url, WP_USER: user_login, WP_APP_PASSWORD: password }
  denied: { param: success, equals: "false" }
  grant: { label: Install KIQR on your repositories, url: "https://...", help: "..." }   # optional: a page at the service where the person grants access to more of it after signing in (a GitHub App installed on a repository); shown under a key picked from the account's list when what they look for is not in it, with "look again"
```

- **redirect**: the browser opens `url`; the service comes back to KIQR's callback (`/api/connect/callback`, under the address the UI runs on) with the values as query parameters, mapped by `returns`. `callback: https` means the service refuses a plain-http callback. When KIQR is signed in to kiqr.cloud, the callback is a single-use relay there (`https://kiqr.cloud/r/<id>`) that sends the browser straight on to KIQR, and nothing is pasted; otherwise KIQR opens the page without a callback and the user pastes what it shows.
- **oauth2**: authorization code with PKCE. `authorize_url`, `token_url`, `client_id` (and `client_secret` when the provider wants one, both usually `${VAR}`), `scopes`, extra `params`. The access token goes to `token_env` (declared in `auth`), the refresh token to `refresh_env`, the expiry to `<token_env>_EXPIRES_AT`; KIQR refreshes before a call when it is about to run out.
- **service_account**: no browser. `key_file` names the variable holding the path of a JSON key (`client_email`, `private_key`); KIQR signs a JWT for `scopes`, exchanges it at `token_url`, and offers the token to requests as `${<token_env>}` (never stored; `auth` may name it only as optional), minted again when it runs out.
- **cloud**: oauth2 for a provider whose app needs a client secret (Google first). kiqr.cloud holds the app, so the manifest names only the `provider` and the `scopes` (no URLs, no client): kiqr.cloud builds the consent page, the provider comes back through its relay to this KIQR's callback, and kiqr.cloud exchanges the code (with KIQR's PKCE verifier) and refreshes the token with the secret added, keeping nothing. The token lands in `token_env` (declared in `auth`), the refresh token in `refresh_env`, as for oauth2. It needs KIQR signed in to kiqr.cloud (Settings → kiqr.cloud); the button says it goes through kiqr.cloud.

`connect` may be a list of ways to sign in, like typed keys beside a button: the first with a button is the button, the rest are alternatives (a service account's key file instead of signing in). An account is ready with its required keys and any one way set up; a call uses the one set up, the first when several are. Keys a sign-in keeps (a token) are never shown as fields. Google Analytics in the marketplace is the worked example:

```yaml
auth:
  - { env: GA4_PROPERTY_ID, label: GA4 property, choices: { … } }
  - { env: GA4_TOKEN, label: Google sign-in, optional: true }
  - { env: GOOGLE_SERVICE_ACCOUNT_FILE, label: Service account key file, optional: true }
connect:
  - { label: Connect with Google, flow: cloud, provider: google, scopes: [https://www.googleapis.com/auth/analytics.readonly], token_env: GA4_TOKEN, refresh_env: GA4_REFRESH_TOKEN }
  - { flow: service_account, key_file: GOOGLE_SERVICE_ACCOUNT_FILE, token_url: https://oauth2.googleapis.com/token, scopes: [https://www.googleapis.com/auth/analytics.readonly], token_env: GA4_TOKEN }
```

A key may read its values from the service once the account is signed in (`choices`), so the user picks instead of typing (the analytics property): a request through the account, a pointer to the items (`*` steps into every element of an array), and in each item the value, its label and a prefix to strip. With `pages: N` (at most 20) the request names `{{page}}` and is read page by page until one comes back empty (every repository, not the first 100). The UI shows a list, a search field when it is long; typing stays possible. `GET /api/projects/{slug}/connections/{id}/choices/{name}`.

```yaml
choices:
  request: { method: GET, url: "https://analyticsadmin.googleapis.com/v1beta/accountSummaries?pageSize=200", headers: { Authorization: "Bearer ${GA4_TOKEN}" } }
  list: /accountSummaries/*/propertySummaries/*
  value: /property
  label: /displayName
  strip: "properties/"
```

Every URL's host must be in `hosts` (or come from a `${VAR}` host). Variables in `needs` and `returns` must be declared in `auth`.

### Placeholders

- `{{arg|json}}`: the argument as JSON (also valid YAML), `null` when it is missing or empty: `title: {{title|json}}` and `tags: {{tags|json}}` in frontmatter, whatever the title holds.
- `{{arg}}` and `{{arg|default}}`: the tool's arguments. A string that is exactly one placeholder takes the argument's own JSON value (numbers stay numbers). A default may hold placeholders of its own: `{{repo|${GITHUB_REPO}}}` (the argument, else the account's key), `{{title|{{tag}}}}`.
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

When a publish goes through an account, the publish tool receives `title`, `text` (Markdown), `html`, `summary`, `tags`, `asset` (`type`, `id`; the id is a slug of the title), `date` (YYYY-MM-DD) and `datetime` (RFC 3339) of the publish, `image` (the asset's first image as named in `{{files}}`, when it has one), and the Audience entry's target fields (`subreddit`). Its `result.text` or a `url` in the answer becomes the publication's link.

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

### Learning

A connection whose accounts hold what the product is (a repository's README and docs) declares `learn:`. With the account's Source switch on, KIQR reads those documents through the ability's own tools, extracts each into facts like a page of the site, and writes the profile and the brief again from everything; each fact cites the document's link. It reads them when the switch goes on (or once the account is signed in), with every crawl, on "Learn now" (`POST …/connections/{id}/learn`), and every week.

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
- `title` and `url` may name the account's keys (`${GITHUB_REPO}`), never a secret, and under `each` `{{value}}`. The text is what the tool said: a `result.text`, a text file (a raw-text answer such as GitHub's `application/vnd.github.raw` is text), else the JSON.

The GitHub manifest in the marketplace is the worked example.

### A hosted program, a sign-in, a live view

An MCP ability may run on another installed ability's program instead of a command of its own: `program: browser` takes that ability's `command`, `args`, `env_set` and `requires`, and one server runs per account and program, shared by every ability of the service on it (one browser profile, never opened twice). `env_set: { NAME: template }` sets the program's environment from the account's keys, `${KIQR_ACCOUNT_DIR}` (the account's `data/` folder) and `${KIQR_PROJECT_DIR}`. `requires:` lists programs KIQR installs itself, only when the user says so (the connect wizard names them and asks before anything downloads; installing an ability never fetches them on its own) and with the installer's output shown line by line (`program_log` events, `GET /api/programs/log`), pinned: a `version`, a `check` command, and per platform a download (`os`, `arch`, an https `url`, its `sha256`, `archive` zip or tar.gz, `bin`; `unpack: true` keeps the whole archive, a program with its libraries and data, and `bin` is the program's path inside it), or `uv_tool` (a pinned spec installed through the `uv` KIQR installed the same way), or `system` (a package per package manager, shown to the user as the one command for their machine, never run by the daemon; on a Linux desktop the web UI can run it through pkexec, the system's own password dialog). They are tried in that order: a download first, then uv, the system's package as the last word. Downloads go into `~/.kiqr/bin`, which comes first on the daemon's PATH; a checksum that does not match installs nothing. An account of an ability whose program lacks a requirement is not ready, and the wizard says what is missing. Xvfb, the browser's virtual display on Linux, is a system requirement on purpose: the system's package is the right build for the OS; `install.sh` installs it (asks once; `-y` says yes), and the app shows the one command or installs it through pkexec. `requires` is honoured only for abilities from KIQR's own marketplace or shipped in the binary: a manifest the user or Seemo wrote never gets to download and run anything. A tool or a provided ability on a hosted program may be `steps` too: each step `call`s one of the program's tools with `args` (a template over the arguments, `{{file}}`, and what earlier steps saved), with `save`, `when` and `for_each` as for HTTP steps; `url_args` names arguments that are URLs, each checked against `hosts`.

`session:` says how an account signs in on the program and stays so: `check` steps (signed in when the last answer holds `signed_in_when`, or when none of `signed_out_when` shows; with neither, the model reads the page and says), `login_url` where signing in starts (the model reads each page's visible controls from the program's snapshot, in whatever language the site serves, and names the user, password and one-time code fields and the button; the runner types only into controls the page listed, the password only into a `type=password` field, only on the ability's `hosts`, and follows up to six pages; the model never gets a value, and a captcha, a check or a wrong-password notice it sees makes the account need a hand) or `login` steps that type the credentials into fixed fields (`{{credential.username}}`, `{{credential.password}}`, `{{credential.otp}}`: available here and nowhere else; for a site whose form never changes), `blocked_when` texts that mean a person is needed, `every` (how often the check runs). The account's login lives in `login: { store, ref }` on its record: `kiqr` (the values in the account's own `.env` as `LOGIN_USERNAME`, `LOGIN_PASSWORD`, `LOGIN_TOTP_SECRET`, set with `PUT …/secrets/<name>` or `kiqr accounts login`; the one-time code is computed from the secret) or a password manager account's id with the item's `ref` (`provides.credential`: a tool taking `{op: get, ref}` and answering `{username, password, otp}`; only the runner calls it). The runner (`POST …/connections/{id}/signin`, `kiqr accounts signin`, and the scheduler every `every`) holds the account's turn, checks, types the login when signed out, checks again; what it typed is struck from every program answer, error and event while in use. Blocked or failing, the account is `needs_hand` (`GET …/connections/{id}/session`, `SessionChanged`): a badge, a To do item, one Seemo line, and tools that act refuse until a person finished the sign-in. `POST …/signout` stops the program and wipes what it kept (the profile); the login stays.

`live_view:` names the program's tools a person's live view is built from, each a step: `frame` answers with an image, `metrics` (optional) answers the window's geometry as JSON (`ow`, `oh`, `iw`, `ih`: the window's and the page's width and height in CSS pixels, and `ox`, `oy`: where the page's top-left corner sits in the window, else taken as centred below the bars) so that a click on a frame of the whole window lands on the page, `click` gets `{{px}}`/`{{py}}` (page pixels when `metrics` is there, else pixels of the frame) or `{{x}}`/`{{y}}` (fractions), `key` gets `{{key}}`, `type` gets `{{text}}`, or `{{char}}` to be called once per character (a real keystroke each; a newline is Enter), `navigate` gets `{{url}}` (checked against `hosts`). The input route answers `{ ok, message }`; a refusal is shown to the person. `POST …/connections/{id}/hands` (a person in the web UI only, like a factory reset) opens a session: the account's turn is held, frames go out as `HandsFrame` events a few times a second and are never kept, `POST …/hands/{id}/input {token, kind, …}` sends a click, a key, text or a URL, and `POST …/hands/{id}/end {token, signed_in}` closes it and, when the person says they signed in, runs the check. Ten idle minutes end it.

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

## Publishing: kinds and the Audience

A kind says where content goes out through an ability: a `section` (`page`, `community`, `list`), the `target` fields that name a spot (`subreddit`, with `label`, `help`, `placeholder`, `optional`, `enum`, `pattern`), `target_from` (how a URL argument names them: `post_url: "reddit.com/r/{subreddit}/"`), or `account_is_target` (the account itself is the spot: an X profile, a blog), a `url` template for a spot's link, and the `publish` tool. A kind without `publish` is one an ability only reads or replies in:

```yaml
service: { id: reddit, name: Reddit, group: social, profile_hosts: [reddit.com] }
kinds:
  - kind: reddit
    label: Subreddit
    section: community
    publish: post
    url: "https://www.reddit.com/r/{{subreddit}}"
    target: { subreddit: { label: Subreddit, pattern: "^[A-Za-z0-9_]{2,21}$" } }
    target_from: { post_url: "reddit.com/r/{subreddit}/" }
```

The ability that publishes to an entry is the one of the account's service that declares its kind with `publish`. Publish with the asset's buttons (pick an Audience entry, or "Somewhere else…" for a new spot, which joins the Audience with the click), `kiqr publish <slug> <type>/<id> <entry>`, or Seemo's `publish_asset` (`target`: an entry; or `spot`: `{kind, fields, account}`), which opens a card unless the account allows publishing on its own and the spot is in the Audience. The card carries the target and the publish runs what it showed. A task run that reaches a publish gets the card in the task's chat and waits. `asset.target` remembers the entry it was approved for; a `Publication` keeps the entry (`audience`), the `account` and the `ability`.

**Email.** A mail account is the sender, declared by whichever ability sends mail: the marketplace's "Send email from your mail account" (`transport: smtp`, keys `SMTP_HOST`, `SMTP_USER`, `SMTP_PASSWORD` (a Gmail app password), optional `SMTP_PORT`, `SMTP_FROM`, `SMTP_SECURITY`), or an HTTP sender with `provides.email`. Its kind's `section` is `list`: lists in the Audience are sent from such an account. `send_email` goes to `to` (addresses) or to a `list`, from the account named by `from` (else the list's own, else the default); publishing an asset to a list is one message per person. A send to a list is one message per person, in the background: the list's send log (`audience/<id>/sends/<key>.jsonl`, never committed) remembers who got it, so a stopped run picks up where it left and the same subject never goes to one person twice; every message ends with why it came and how to leave; the account's activity gets one `publish` entry for the whole send; `ListSending` events say how far it got.

**Lists** are Audience entries of kind `list` (`projects/<slug>/audience/<id>/entry.yaml`, committed, naming the account it is sent from) with `contacts.csv` (email, name, consent, added, unsubscribed; never committed, mode 0600, so personal data stays on the machine and out of history). `GET/POST /api/projects/{slug}/lists`, `…/lists/{id}` (`PUT`, `DELETE`), `…/contacts` (`GET`, `POST {contacts, consent}`, `DELETE {email}`), `…/import {csv, consent}`, `…/unsubscribe {email}`, `…/sends`; `kiqr lists ls|add|import|unsubscribe|rm`. Seemo sees counts, never addresses (`list_lists`), and proposes a new, empty list with `create_list` (a card); the user adds people on the Audience page.
