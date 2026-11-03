# Changelog

Every KIQR release, newest first. Versions follow [semver](https://semver.org): before 1.0, a minor release (0.x.0) brings new features and may change the `~/.kiqr` layout or the API, and a patch release (0.x.y) only fixes things. Write notes for the next release under "Unreleased", or leave it empty and `scripts/release.sh` lists the commits since the last one.

## Unreleased

## v0.6.0 — 2026-11-03

- **Seemo remembers.** Seemo → Memory lists what Seemo keeps for this project: rules, preferences, facts and lessons, each holding everywhere, on your own channels or in other people's places. Tell Seemo how things should always be done and it notes it ("Noted: …" in the chat); after chats and task runs it saves what it learned, and a lesson from a task run waits for your Keep. Add, edit and delete anything on the page, or switch learning off. `kiqr memory`.
- **Everything is configurable through Seemo.** Whatever you can change in KIQR's pages, Seemo can propose on a card: `update_channels` changes several channels at once (publisher, what Seemo may do, name, link, a website's ability), and `change_kiqr` covers the rest (which ability may use which login, ability settings, installing, updating, pausing and removing abilities, their instructions, your setup answers, memory, Vault labels, renaming chats). Secret values, the people on a list and your own decisions stay yours.
- **In a browser, abilities say what, not how.** Replying and posting on X and Reddit is a goal ("publish a post with the approved text and image") that a model reaches page by page, so a site's new markup no longer breaks it. It types only the text you approved, reads it back before it sends, and stops for anything it was not asked to do. A route that worked is remembered per login and replayed without the model while the page still matches, and steps are picked by a fast model, so a post needs far fewer model calls and a cheaper model for the rest.
- **A post that does not fit is fixed, not refused.** Seemo cannot save a post longer than its network allows; an approved post that is too long anyway is shortened before it goes out, and keeps its approval only on a channel that lets Seemo publish on its own; elsewhere it comes back for your yes, keeping its channel and time.
- **A publish proposal is the post's own card**: the post as it will look, with where, as whom and when, and Publish it / Decline, instead of a second card below it.
- **A website is a channel** published through an ability it picks (a repository that builds it, a blog engine), with its own fields; WordPress connects from the channel. Signed-in apps show their account, and the Vault lists logins that need a hand.
- A chat renamed while Seemo is answering keeps its new name.
- Update the X and Reddit abilities after updating KIQR (0.5.0 needs this release).

## v0.5.0 — 2026-11-02

- **The Vault, Channels and abilities replace accounts and the Audience.** There is no migration: accounts made by an earlier KIQR are not carried over; add their logins and keys to the Vault once.
  - **The Vault** (Setup → Vault) keeps every credential Seemo can be given (logins, API keys, app passwords, signed-in apps, mail servers, or any fields) encrypted in one file on this machine, each scoped to the domains it is for and shown with that site's icon. Values are never shown again; a login's password and two-factor key are typed by Seemo's sign-in runner and handed to nothing else. "Sign in with Google" and "Sign in with GitHub" (through kiqr.cloud) come first in its Add panel. `kiqr vault ls|add|set|rm|signin|signout|activity|recommend`.
  - **Channels** (Marketing → Channels) are everywhere your people are: your own profiles, pages, blog and mailing lists, and the communities you take part in. Each says what Seemo may do there (read, publish and reply: off, ask me first, or on its own) and who it posts as (its publisher, a credential from the Vault), or the one thing missing with its fix. The crawl adds the profiles it finds on your site as channels. Anywhere that is not a channel, Seemo asks first, and your yes adds it. `kiqr channels`.
  - **Abilities** ask for the credentials they need ("a login for x.com", "an API key for fal.ai") and never own one: each has an **Access** tab where you allow it credentials from the Vault for this project, add one, or sign in with one click ("Sign in with Google", "Connect with WordPress"). Picking a channel's publisher allows its ability too. Credentials and abilities recommend each other by site: a Google sign-in suggests Google Analytics and Search Console. `kiqr access ls|allow|revoke`.
  - A credential's values only ever go to that credential's own domains, whatever an ability asks. A login's browser session, its activity log and daily cap belong to the credential, so two projects never drive one browser at once. To do lists the channels that cannot be published to yet, with the fix.
  - Every marketplace ability is updated for this; update them after updating KIQR.
- **Library → Media**: the business's own pictures as `media` assets, fed by sources (the website after each crawl, abilities with a `media:` block) and listed in the brand kit as local files; `add_media`, `list_media`, `read_url` images, `kiqr media`. The posts job and the image instructions pick a real picture first, vary layouts (photo, split, card) and never pass a generated picture off as the business's own; `source` always renders locally.
- A real browser of Seemo's own, for sites behind a login. "Browse with a real browser" ships in the binary as a core ability; it hosts invisible_playwright_mcp (a Firefox patched below page scripts, headless), one copy per login with its sign-in kept. KIQR installs the pinned programs itself, only after you say so, and shows the install's log as it runs; the virtual display the browser needs on Linux (Xvfb) comes from the system's own packages (`install.sh` installs it, or the app installs it through the desktop's password dialog). "Read, reply and post on X" and "… on Reddit" run on it: Seemo reads timelines, searches and threads, and replies and posts through cards.
- The sign-in on a hosted browser is led by the model: a manifest names `login_url` and the model reads each page's controls (any site, any language) and says which are the user, password and code fields; the runner types into those only, and when a site asks for a person (a captcha, a device check) the login shows "Needs a hand" and the live view in the Vault lets you finish it from anywhere KIQR is reachable. A check that finds an empty page loads it again instead of reading it as signed out.
- The live view works: a click lands where it was aimed (the page's geometry, read through a new `metrics` step, moves the click onto the page), typing goes in as real keystrokes, and what the browser refuses is said in the view.
- A hosted program is stopped politely, as a process group, so the browser lets go of its profile and the next start is not refused with "another Firefox already has the profile open".
- Posts are written for the networks your channels are on: three for a network when its first channel is added, once per network.
- Publishing to a site built from a repository: the Publish button says "Ask Seemo to publish" where a publish needs what only Seemo writes (the site's source files as a pull request), and opens a chat that asks for it.
- Chat: a card Seemo puts in mid-turn shows below its words, and the steps fold stays the same box while working, so an unfolded list does not jump.
- The Abilities page lists every installed marketplace ability with its Update button, and the marketplace shelf lists every ability not installed yet.

## v0.4.1 — 2026-11-01

- An account is the service, not an ability: "Google", not "Read Google Analytics". Google Analytics and Search Console share one Google account (one sign-in); the property and the Search Console property are places on it, picked from the lists the account can see, and Seemo reads each through its place. Accounts from before move along on their own (tokens renamed, the property into a place). The connect grid has one tile per service and installs whatever the service needs. The Abilities page keeps only what is not an account: built-in abilities, what you taught Seemo and instructions from the marketplace; services live under Setup → Accounts. Manifests: `places` (an ability's own place on the account, with `choices`), `moved` (what older accounts called things), `{{x|urlencode}}`.

## v0.4.0 — 2026-11-01

- Accounts, not channels. The user sees three things: accounts (everything Seemo signs in to), abilities (what Seemo knows how to do) and lists (the people the project emails, under Audience). Publishing goes to an account, and to a place in it where the service has several (a subreddit, a branch, a list); email goes from a mail account to people or a list. Each account says what Seemo may do there, by role: publish, reply and manage are on its own, ask me first or off (asking by default); read, learn and measure are on or off. This replaces "Seemo may publish here on its own" and the Source switch; existing records are read into the new shape when loaded (a trusted channel keeps publishing on its own; reply and manage ask again). A mail account is the sender: its recipients are per send or a list, so one mail account serves every list and no keys are copied. Sending to a list is one message per person in the background, with a footer saying why it came and how to leave, a send log that picks up where a stopped run left, and progress on the list page. `kiqr accounts allow|places`, `kiqr lists`, `kiqr publish --place`, `kiqr email --list`; `kiqr channels` and `kiqr trust` stay as shortcuts. Manifests: `place` on a kind (older `destination` still read), a `service` block (name, icon, group) for the grid of things to connect, `allow` defaults. Seemo: `list_lists`, `create_list`, `place` on `publish_asset`, `list` and `from` on `send_email`, `create_channel` proposes a place.
- Seemo learns the business from an account, not only from the website: allow "learn" on it and what it holds goes into the profile and the brief. A GitHub repository is the first: its About line, README, docs, changelog and releases, read again with every crawl, on "Learn now" and every week. Abilities declare what to read with `learn:`.
- Abilities that publish to files a site builds from (a static site through a pull request) get the publish's `date`, `datetime` and the asset's first `image`, and `{{value|json}}` writes a value safely into frontmatter.
- A list read from the service (an account's repositories, properties) reads every page, not the first 100, and a long one is a search field.
- A publish tool can require an option Seemo prepares for each publish (the files a site is built from): a publish without it is refused with what is missing, before the card.

## v0.3.5 — 2026-11-01

- A factory reset on a machine KIQR is reached at over the network can keep the network settings ("Keep the network settings", checked when it listens beyond this machine), so the web UI comes back at the same address.
- The build screen is one window that never scrolls: the site map is the backdrop, fitted with room around it and without controls, the ring sits on the centre, and what Seemo learns comes in one sentence at a time.
- Writing to Seemo under its questions and suggestions carries on in the same window, and Seemo knows what it suggested. A suggestion has an × to drop it for good.
- An ability that names what to look for in a site's HTML (WordPress) is suggested only when it is found there.
- Favicons: the app has one (with kiqr.cloud and kiqr.dev in the same family, each its own colour).

## v0.3.4 — 2026-10-31

- Signing in to kiqr.cloud from a browser shows kiqr.cloud's approval page and comes straight back, also when KIQR is opened at a server's plain http address (no activation code). Seemo's set-up suggestions and the getting-started steps offer it first, marked Optional.

## v0.3.3 — 2026-10-31

- The first build: the site map fills the screen behind the progress on a wide screen. Seemo's suggestions open straight to sign in, typed networks are read as names, and after the last question you can write to Seemo in the same window.
- Getting started asks for your first posts after a channel is connected, since a post is approved for the channel it goes out on.
- Fixed: the crawl's updates in Seemo's window stayed below every message; they now sit among the messages by when they happened.

## v0.3.2 — 2026-10-31

- Fixed: a factory reset is refused for a person on a server opened over http or through a proxy

## v0.3.1 — 2026-10-31

- Everything in v0.3.0 below, which was never published: its macOS builds stopped on GitHub's retiring macOS 14 runners. macOS builds now run on macOS 15.

## v0.3.0 — 2026-10-31

- A new first run: Seemo checks it has a model before it starts, then reads the site with one overall progress, the time left and what it is learning as it goes, while it asks you four quick questions. Then it shows you your business as it sees it, to confirm or fix, and makes your first plan and three posts in your brand. The rest of KIQR opens when the profile is ready.
- The getting-started steps on To do follow from what you told Seemo while it read your site: approve the plan and your first posts, connect the networks you named, one piece of work for your goal (an article for new customers, a letter to the customers you have, conversations to join, a launch announcement), connect where the plan's numbers live, and a weekly rhythm sized to the time you have. Each one that needs Seemo opens a chat in the corner.
- Factory reset, under Settings → Storage: erases everything KIQR keeps on this machine (projects, history, settings, keys, abilities) and starts again as a new installation. It asks four times (what goes, a sum drawn as a picture by KIQR itself, the phrase typed out, a button held for two seconds) and only takes the request from the web page: Seemo, the CLI and abilities can't do it.
- Seemo's first conversation ends with what it would set up, from your answers and what it saw on your site (Google Analytics or WordPress on the pages, a newsletter for selling to the customers you have), each added with one tap; the getting-started steps then ask you to sign in to them. Name your networks in your own words; where Seemo can't post yet, it says so.
- Plans, posts and tasks wait until the business profile is ready, and Seemo says so instead of guessing.
- Fixed: after a one-click sign-in (Google Analytics and the like) the property picker appears right away, without reloading the page.

## v0.2.0 — 2026-10-30

- Prebuilt binaries for Linux, macOS and Windows (x86_64 and arm64), published by CI for every release.
- The installer downloads the release for your platform and checks its SHA-256; running it again updates KIQR. Windows has its own installer, `install.ps1`.
- KIQR tells you when a new version is out (Settings, `kiqr status`) and updates itself on your word: the Update button, or `kiqr update`.
- `kiqr start`, `kiqr stop` and `kiqr restart` manage the background service on every platform, and `kiqr status` says whether KIQR is running and how.
- Optional automatic updates: `kiqr update --auto on` (or Updates in Settings) installs new releases on their own, when Seemo is not working. Off by default.
- On a server, the installer keeps KIQR running after you log out of SSH and shows how to open it through an SSH tunnel.
- KIQR runs on Windows.
- KIQR now opens at http://127.0.0.1:4040 (it was 7777; an existing home moves over by itself). Change the address under Settings → Network: KIQR restarts there and the page follows.
