# Changing KIQR itself

Everything a person can change in KIQR's pages, you can change too, with `change_kiqr`: the same request the page makes, on one card the user approves. Prefer a dedicated tool when there is one (`update_channels`, `update_task`, `change_config`, `create_channel`, `remember`, `update_asset`, `update_campaign`): its card reads better. For everything else use the routes below. Read the current state first (`list_channels`, `list_credentials`, `list_abilities`, `read_metrics`, …), change only what the user asked for, and say in `summary` what changes in plain words ("Let X (UnblockingAPI) use the unblockingapi login"). `{slug}` is this project; the daemon fills it in. One card is one request; for several, make one call each, or use the batch tools.

What you never do, and `change_kiqr` refuses: anything with a secret value (a password, a key, a token: the user types those in the Vault or under Settings), the people on a list, answering an approval, saying whether an uncertain publish went out, another project, deleting the project, sign-ins that need the user's browser, KIQR's update, telemetry, a reset.

## Channels

- `PUT /api/projects/{slug}/channels/{id}` — prefer `update_channels`. Body: any of `label`, `description`, `url`, `ownership` (owned, third_party), `publisher` (credential id), `ability`, `fields`, `kind: "website"`, `read`, `publish`, `reply` (off, ask, auto), `learn`, `add: true` (a suggestion becomes a channel).
- `DELETE /api/projects/{slug}/channels/{id}` — remove a channel. A profile the crawl found is not suggested again.
- `POST /api/projects/{slug}/channels/{id}/learn` — read an owned channel again and learn from it now (learning must be on).

## Vault (no values, ever)

- `POST /api/vault` — a new credential without its values, for the user to fill in: `{label, template, domains: [..], login_url?, notes?}`. `template` from `list_credentials` → `templates`. Tell the user to open Setup → Vault to type the password or key.
- `PUT /api/vault/{id}` — change `label`, `domains`, `login_url` or `notes`. Never `values`.
- `DELETE /api/vault/{id}` — remove a credential and every grant of it, in every project. Only when the user asks by name.
- `POST /api/projects/{slug}/credentials/{id}/signin` — sign a login in again on its browser (it uses the stored values; you never see them). `.../signout` ends that session.

## Which ability may use which credential

- `POST /api/projects/{slug}/access/grant` `{ability, credential}` — the ability may use the credential in this project.
- `DELETE /api/projects/{slug}/access/grant` `{ability, credential}` — no longer.
- `PUT /api/projects/{slug}/abilities/{ability}/settings` `{credential, values: {NAME: value}}` — what the project picks once for an ability (a GA4 property, a site's address); the names and choices are in `list_abilities`.

## Abilities

- `POST /api/abilities/{id}/install` — install one from the marketplace (`list_abilities` → marketplace).
- `POST /api/abilities/install` `{source, scope: "global" | "project", project?, name?}` — from a git URL or a folder.
- `POST /api/abilities/{id}/programs` — install the programs an ability needs.
- `POST /api/abilities/{id}/update` — update it (`?project={slug}` for one installed in this project).
- `POST /api/projects/{slug}/abilities/{id}/pause` and `.../resume` — off or on in this project. `POST /api/abilities/{id}/pause` and `.../resume` — in every project.
- `DELETE /api/abilities/{id}` — uninstall (`?project={slug}` for this project's copy). Core abilities switch off, never leave.
- `PUT /api/projects/{slug}/abilities/{id}/instructions` `{text, file?}` — rewrite an ability's instructions for this project (read them first with `read_instructions`, change what was asked, send the whole text). `PUT /api/abilities/{id}/instructions` — for every project. `DELETE` on either removes that version (a shipped ability falls back to its own text).
- `POST /api/abilities/refresh-index` — read the marketplace again.

## The project

- `PUT /api/projects/{slug}/intake` — the owner's setup answers, whole: `{goal: customers|repeat|known|launch, goal_note, customers, time: lt1|1to3|3plus, networks: [..], skipped: [..], dismissed: [..]}`. Read them first and keep what is not changing.
- `PUT /api/projects/{slug}/settings` — prefer `change_project_settings`.
- `PUT /api/projects/{slug}/crawl/limit` `{max_pages}` — the page limit, in the running crawl too.
- `PUT /api/projects/{slug}/profile/{section}` — replace a whole profile section; prefer `update_section_field`.
- `POST /api/projects/{slug}/media/import-site` — import every picture from the business's site into Media.
- `POST /api/projects/{slug}/performance/refresh` `{source?}` — pull the numbers now.
- `PUT /api/projects/{slug}/chats/{id}` `{title}` — rename a chat.

## Memory

- `PUT /api/projects/{slug}/memory/{id}` `{text?, kind?, scope?}` — change a memory (it becomes the owner's).
- `DELETE /api/projects/{slug}/memory/{id}` — forget one.
- `PUT /api/projects/{slug}/memory/learning` `{on}` — learning from chats and tasks on or off.

## Library and marketing

- `DELETE /api/projects/{slug}/assets/{type}/{id}/schedule` — drop a scheduled time; it stays approved.
- `DELETE /api/projects/{slug}/assets/{type}/{id}` — delete an asset for good (archiving with `update_asset` is usually what is meant).
- `DELETE /api/projects/{slug}/campaigns/{id}` — delete a campaign (`update_campaign` with status dropped keeps its history).
- `PATCH /api/projects/{slug}/tasks/{id}` — prefer `update_task`; `allowed` (tool groups) and `campaign` are set here too.

## KIQR's settings

- `PUT /api/config` — prefer `change_config`.
