# KIQR design

The look of the KIQR web UI, the CLI's wording, kiqr.dev and kiqr.cloud. The tokens live in `web/src/tokens.css`; this file says why they are what they are and how to use them. Light theme only: the audience is business owners hiring their first AI marketer, and the product should feel like one bright, calm workspace, not a terminal.

## The idea

Warm paper, black ink, one electric blue, one lime signal. Confident headlines in a grotesk with character, quiet text in Inter, code in mono. Things that are objects (cards, inputs, the Seemo panel, the project selector) are white with a hairline border and a soft shadow; the page ground is warm, so white surfaces read as raised without heavy shadows. The one place the product shows off is the crawl screen: a live map of the site being read, because the user is watching their first AI employee work and that moment should feel like something.

## Color

| Token | Value | Use |
| --- | --- | --- |
| `--bg` | #f7f6f2 | page ground |
| `--bg-2` | #ffffff | cards, panels, inputs, sidebar items |
| `--bg-3` | #ecebe6 | chips, progress tracks, hover, Seemo's bubbles |
| `--line` / `--line-2` | #e2e0d9 / #cfccc3 | hairlines / input and outlined-button borders |
| `--ink` / `--ink-2` / `--ink-3` | #121212 / #55534d / #8a877f | text / secondary / meta and placeholders |
| `--accent` / `--accent-2` / `--accent-soft` | #2b4cff / #1f38d1 / #e6eaff | the blue: primary actions, links, live state, selected |
| `--signal` | #d7ff43 | lime: Seemo's mark, "with Seemo" state, new-fact highlight. Nothing else. |
| `--ok` `--warn` `--bad` (+ `-soft`) | greens, ambers, reds | status chips and dots only |

Rules: blue is for actions and live state, never decoration. Lime belongs to Seemo. Status colours never fill large areas. No gradients except the crawl map's sweep and the new-fact highlight, both transient.

## Type

- Headlines: Bricolage Grotesque 600, tight tracking. `.d1` 36–56px, `.d2` 26–34px, `.d3` 20px.
- Text: Inter 400, 15px, line-height 1.55; 500 for labels and buttons; 600 for field names.
- Code, URLs, commands: JetBrains Mono 500, 13px.
- Sentence case everywhere. No uppercase labels. Numbers in stats use tabular figures.

## Shape and depth

Radii: 8px inputs and menu items, 12px cards, 20px large panels (map, Seemo), pills for buttons and chips. Shadows: `--sh` for raised objects, `--sh-lg` for floating ones (Seemo, menus). Hairline borders on every card so edges hold on the warm ground.

## Layout

Sidebar 240px: wordmark, the project selector (a white card with initial, name, host and a caret; opens a menu listing projects and "Add a project"), then navigation scoped to that project — Profile, Crawl, Competitors, Tasks, Chats with Seemo, Project settings — and at the bottom the global Connections and Settings. Content area max 1180px, 28/32px padding. Below 900px the sidebar stacks on top.

Seemo lives in a black 52px launcher bottom right (lime square inside; it pulses while Seemo is reading) that opens a 380×560 panel; full screen on phones.

## Motion

Motion shows state and is otherwise still.

- Crawl map: the radar sweep runs while the crawl is live; nodes pop in; edges draw from parent to child; a fetching node pulses blue, a node with Seemo pulses lime, a parsed node flashes lime then settles to black; blocked nodes are amber. Stats count up. New facts slide in with a lime wash that fades.
- Everywhere else: list rows and cards rise in over 200–260ms; menus and the Seemo panel scale in over 140–180ms; the launcher pulses only while Seemo is busy.
- `prefers-reduced-motion` turns all of it off.

## Components

- **Buttons** (`.btn`): pill, 14px/500. `.p` black primary, `.a` blue for the one main action on a screen, `.g` outlined, `.t` text. Labels name the action: "Start crawl", "Save and lock", "Crawl again".
- **Chips** (`.chip`): status and counts; `.ok` `.warn` `.bad` `.acc`, `.live` pulses its dot.
- **Field** (`.field`): the onboarding URL input, a pill with the `https://` prefix and the action inside.
- **Profile field** (`.pf`): white card; name with chips (locked by you, needs review), value, evidence line with source path in mono and the quote, confidence bar; Edit / Lock / Remove reveal on hover (always visible on touch). Editing locks the field.
- **Project selector** (`.sel` + `.menu`), **key-value settings** (`.kv`), **Seemo panel** (`.seemo`), **crawl map** (`.map`, `.node`, `.stat`, `.stream`).

## Writing

The interface speaks to the user in the second person, plainly, sentence case. Seemo speaks in the first person as a colleague: specific, calm, no exclamation marks, no emoji. UnblockingAPI is mentioned only where pages could not be read.
