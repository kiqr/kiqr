# Changelog

Every KIQR release, newest first. Versions follow [semver](https://semver.org): before 1.0, a minor release (0.x.0) brings new features and may change the `~/.kiqr` layout or the API, and a patch release (0.x.y) only fixes things. Write notes for the next release under "Unreleased", or leave it empty and `scripts/release.sh` lists the commits since the last one.

## Unreleased

- The sign-in on a hosted browser is led by the model: a manifest names `login_url` and the model reads each page's controls (any site, any language) and says which are the user, password and code fields; the runner types into those only (the password only into a password field, only on the ability's hosts) and never hands the model a value. X and Reddit use it (0.2.0), so a changed page or a page in another language no longer breaks the sign-in. The Abilities list says "signed out" or "needs a hand" instead of "via your X account" when the account does not work right now.
- Connecting an account of a service whose ability is switched off for the project works, and switches the service's abilities back on; it was refused with "… does not publish 'x'".
- The first run writes no posts. Posts come when an account is connected, three for that network at once (X, Reddit, …), written the way that place reads, and only once per network; the first-week screen drafts the plan and points at the connect grid instead. `POST …/assets/generate` takes `network` for one network's posts.
- Chat: a card Seemo puts in mid-turn shows below its words (and below the live bubble while it works), not above them; the steps fold stays the same box while working, last in the bubble, with a spinner in place of the chevron and the count as its label, so an unfolded list does not jump; it stays unfolded when the turn ends. The Guide's "Connect …" step matches the networks you named in your own words ("X account", "Reddit") to your accounts, so a connected Reddit counts.
- A hosted program is stopped politely, as a process group: it and its browser end on their own and the browser lets go of its profile, so the next start for the account is not refused with "another Firefox already has the profile open" (it was, right after saving a login: the kill left the browser running). Saving a login (user name, password, one-time code) no longer restarts the account's program at all, since only the sign-in runner reads those. A bare tool listing with no account opens no browser.
- Publishing to a site built from a repository: an asset's Publish button says "Ask Seemo to publish" for an account whose publish needs what only Seemo writes (the site's source files as a pull request), and opens a chat that asks for it; pressing Publish there used to fail with a note about `changes`. A request's variables are read from the manifest's template only, never from what an argument holds: a page whose code says `${growth.monthly}` goes through as written (it was refused as an unset key).
- The live view works: a click lands where it was aimed (the frame is the whole browser window; the page's geometry, read through a new `metrics` step, moves the click onto the page), typing goes in as real keystrokes one by one instead of a script the browser program refuses, and what the browser refuses is said in the view. Connecting Reddit no longer asks for a subreddit first: places come after the sign-in and can wait.
- A sign-in may name where more access is granted (`connect.grant`): under a key picked from the account's list (the GitHub repository) the card says "Not in the list? Install KIQR on your repositories, then look again". GitHub is a GitHub App: it sees only the repositories it is installed on, and its sign-in page never offers the installation, so the link is the missing step.
- The Abilities page lists every installed marketplace ability again, the ones that work through an account included (Google Analytics, Search Console, WordPress, GitHub, X), each saying which account it uses, with its Update button; they had been hidden behind their accounts, which left no way to update them.
- Accounts an older KIQR made for one ability each (Read Google Analytics, Read Google Search Console) join as one account of the service when their abilities update: one Google card with both places; where the two sign-ins held different tokens (one scope each) the token is cleared, and one Connect with Google sets both. "Browse with a real browser" has the on/off switch like every core ability: it needs no account to be on, and the sites it browses are its accounts, connected under Accounts.
- Programs KIQR installs (the browser's) are asked about first: the connect wizard names them and where they go, and nothing downloads before "Install now"; installing an ability no longer fetches them on its own. The install shows its log as it runs (what KIQR did, what uv printed), in the wizard and in the ability's window, and a page opened again while an install runs picks it up. An ability that runs on another's program (X, Reddit on the browser) installs that program's requirements, which left the wizard waiting before.
- Account cards and the connect wizard show the service's own mark (the ability's avatar) in place of a generic icon; X and Reddit got theirs.
- The marketplace shelf lists every ability not installed yet, a service's abilities (X, Reddit, email, WordPress…) with Connect in place of Install: it showed nothing when everything left in the index connected an account.
- A real browser of Seemo's own, for sites behind a login (M2 of "Accounts, not channels"). "Browse with a real browser" ships in the binary as a core ability, on from the start and doing nothing until a site account is connected; it hosts invisible_playwright_mcp (a Firefox patched below page scripts, headless), one copy per account with its login kept in the account's own folder; KIQR installs the pinned programs itself (`requires`: checked downloads into `~/.kiqr/bin`, honoured only for abilities from KIQR's own marketplace); the virtual display the browser needs on Linux (Xvfb) comes from the system's own packages: `install.sh` installs it (asks once; `-y` says yes), and the app shows the one command for your machine or installs it for you through the desktop's own password dialog. An account is not ready until everything its program needs is there. "Read, reply and post on X" and "… on Reddit" run on it: Seemo reads timelines, searches and threads, and replies and posts through cards; a subreddit is a place on the Reddit account. An account signs in by itself from a login saved in KIQR (user name, password, a one-time code secret: only the sign-in runner ever reads them, what it types is struck from every answer and log), and when a site asks for a person (a captcha, a device check) the account shows "Needs a hand": the live view under the account streams the browser to the web UI and sends your clicks and keys back, from anywhere KIQR is reachable. `kiqr accounts login|signin|signout`. Manifests: `program` (run on another ability's program), `env_set`, `requires`, steps on a hosted program (`call`), `url_args`, `session`, `live_view`, the `credential` contract.

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
