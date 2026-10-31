# Changelog

Every KIQR release, newest first. Versions follow [semver](https://semver.org): before 1.0, a minor release (0.x.0) brings new features and may change the `~/.kiqr` layout or the API, and a patch release (0.x.y) only fixes things. Write notes for the next release under "Unreleased", or leave it empty and `scripts/release.sh` lists the commits since the last one.

## Unreleased

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
