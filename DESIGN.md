# KIQR design

The look of the KIQR web UI, the CLI's wording, kiqr.dev and kiqr.cloud. The tokens live in `web/src/tokens.css`; this file says why they are what they are and how to use them.

KIQR is modelled on ContentWriter (`~/Projects/contentwriter`, `apps/web`): same dark palette, same sidebar, same card language, same live crawl screen. KIQR does everything ContentWriter does and more (evidence for every fact, Seemo, posts as well as articles), so when a screen exists in both, copy ContentWriter's layout and add KIQR's extras below or beside it rather than redesigning it.

## The idea

A deep navy workspace. Panels sit one step lighter with hairline borders; items inside panels one step lighter again. Teal is the one action and live-state colour, indigo the second accent for anything the AI is doing, lime only for Seemo's mark. The crawl screen is where the product shows off: a live radial map of the site, a progress ring, an activity feed and the business profile assembling itself.

## Color

| Token | Value | Use |
| --- | --- | --- |
| `--bg` | #070a12 | page ground |
| `--panel` | #0c1220 | cards (at 80%), sidebar (at 60%), inputs, menus |
| `--panel-2` | #111a2c | items inside cards, chips, hover |
| `--line` / `--line-2` | #1c2740 / #2a3754 | hairlines / input and outlined-button borders |
| `--fg` / `--muted` / `--dim` | #e6ecf7 / #8392ad / #56627c | text / secondary text and card labels / meta and placeholders |
| `--accent` | #5eead4 | teal: primary buttons (dark text on it), links, active nav, live state |
| `--accent-2` | #818cf8 | indigo: AI and learning (the learning panel, empty-state icons, gradients with teal) |
| `--signal` | #d7ff43 | lime: Seemo's mark and launcher. Nothing else. |
| `--ok` `--warn` `--bad` `--skip` | greens, ambers, reds, slate | status pills, graph nodes and dots |

**The business's own colours.** On the business profile the site's first two brand colours (collected from its pages and stylesheets) become `--brand` and `--brand-2`, and only identity decorations use them: the hero glow, card icon tiles, brand-term tokens, avatars, the voice quote and card hover edges. Actions stay teal everywhere. Without brand colours the profile falls back to indigo and teal.

Gradients only teal→indigo, and only for emphasis: the onboarding headline, the progress ring, confidence bars. Topic clusters use ContentWriter's cluster palette (teal, indigo, pink, amber, green, blue, orange, violet).

## Type

- Inter throughout: page titles 600 20–24px, item names 500 14px, body 14px/1.55, descriptions 12px muted.
- Card labels: 11px, uppercase, 0.08em tracking, muted (`.cap`). Nav group labels the same at 10px, dim.
- JetBrains Mono for paths, URLs, counts and timings.

## Shape and depth

Radii: 8px buttons, inputs, nav items; 12px items and the project switcher; 16px cards. Almost no shadows; edges come from hairlines. Floating things (menus, Seemo panel) get `--sh-lg`.

## Layout

Sidebar 256px, as ContentWriter: wordmark; the project switcher (globe tile, "Project" label, name, chevrons; menu lists projects and "Add project"); grouped navigation with lucide icons and a teal highlight that slides between items — Content (Library, then one item per asset type with its count: Articles and Posts always, other types once they have assets), Your business (Business profile, Content map, Content gaps, Website crawls, Competitors), Seemo, which is the automation (Chats, Calendar, Tasks, Connectors), Settings (Project settings); footer with Settings and where data lives. Settings (for the whole installation) has its own sub-navigation: General (status tiles, this installation), Models (provider/model per task, with whether it can run here), Crawling (defaults, the UnblockingAPI switch), Connections (keys and services, set or not; values never shown), Storage (what is and isn't in history, backups). Edits collect in a floating save bar; saving writes config.yaml and commits. What each project publishes to lives under the project's Seemo → Connectors. Content area up to 1360px, 28/40px padding. Below 900px the sidebar stacks on top. On first run (no projects) there is no sidebar: onboarding stands alone, centred on a faint grid.

Business profile, as ContentWriter, opened by a brand hero so the user sees their own business first: the site's share image as a slowly drifting cover, its icon (or initial) on a glowing tile, the business name in the site's own primary font, its tagline, language, regions and social links, with Export and Rescan website in glass buttons; then animated counts (products & services, audiences, pages read, facts with a source, topic clusters, content gaps; zero counts are hidden); then the version line with an Edit button that turns every card editable (save makes a new version); cards, each with a lucide icon tile: What the business does | Brand kit (logo, colour swatches that copy on click, font specimens), Products & services (product/service icons) | Audiences (initial avatars in the brand colours), Tone of voice (the site's own example sentences rotating in a large quote, then traits, examples, avoid), From your website (a gallery of the site's share and hero images), Brand terms | Regions served, then KIQR's additions (What sets it apart | Competitors, Pricing), Existing topic clusters, Content gaps we found, and at the bottom every fact with its source page and quote, by section, with edit and lock.

## Motion

Motion shows state and is otherwise still.

- Crawl screen: ported from ContentWriter. The radial site map grows as pages are found, with a sweeping beam and packets along edges; nodes colour by status; the progress ring and stats tween; the activity feed slides rows in with a teal wash; discoveries pop in; the phase timeline fills Discover → Crawl → Learn → Profile; the learning panel streams page reads, then profile sections.
- Onboarding: headline, text and field rise in one after another; submitting lifts the form away and the crawl screen rises in while the sidebar slides in.
- Route changes: content rises in over 360ms. Cards on the profile rise in, staggered. The active nav highlight springs between items.
- `prefers-reduced-motion` turns all of it off.

## Components

- **Buttons** (`.btn`): 8px radius. `.p` teal primary (one per screen), `.g` outlined secondary, `.t` text. Labels name the action: "Rescan website", "Save new version", "Build the profile".
- **Chips** (`.chip`): status and counts; `.ok` `.warn` `.bad` `.acc`, `.ai` (indigo, scheduled or made by Seemo).
- **Library** (`library.css`): type strip, then a grid of asset cards — 16:9 thumbnail (the asset's first image, otherwise its type icon on an indigo/teal wash), type tag on the thumbnail in the all-types view, title, summary, status chip, where it was published, who made it (person or Seemo) and when. An asset page previews by the type's `preview` (Markdown rendered, posts as a post, images, video, audio, campaign YAML), with status, origin, publications and files beside it. Unknown types fall back to the file list.
- **Cards** (`.pcard`), **items** (`.it`: name, uppercase kind tag, description), **tokens** (`.tk`: brand terms, traits; italic block tokens for quoted sentences).
- **Field** (`.field`): the onboarding URL input with the `https://` prefix and the action inside.

## Writing

The interface speaks to the user in the second person, plainly, sentence case (the uppercase card labels are styling, written in sentence case in code). Seemo speaks in the first person as a colleague: specific, calm, no exclamation marks, no emoji. Screens that are not built yet say so plainly ("Coming in the next release") rather than showing fake content. UnblockingAPI is mentioned only where pages could not be read.
