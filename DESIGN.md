<!-- Source of truth: the KIQR design system on claude.ai (tokens.json + brand book). Keep this file in sync when it changes. -->

# KIQR

KIQR is a self-hosted marketing agent. You give it a website, it crawls it, builds a business profile, and from then on Seemo — the CMO persona that lives in a widget on every screen — plans and runs marketing with you. This document is the visual and written standard for the web UI, the CLI's text output, kiqr.dev and kiqr.cloud.

Tagline: Your self-hosted CMO.

## Principles

**Whitespace is the luxury.** The product is white, the type is light, and nothing competes. Premium here means calm: generous margins, long measures of quiet, one thing in blue. If a screen feels empty, it is probably right.

**Thin, but solid.** Manrope at 300 for display sizes, 400 for text, 500 for the few things that need weight. Light weights carry the elegance; the even stroke and the deep-navy ink keep them readable. Nothing is set bolder than 500.

**One blue.** `blue` (#1442c4) is the brand, the action colour and Seemo's colour. Each view gets at most one filled-blue element (usually the primary button or Seemo's avatar). Links, selected state and the confidence bar use it as text or line. If two things on a screen are filled blue, demote one.

**Structure is information.** A hairline (`line`) separates things that are different. A box means "this is a distinct object you can act on" — the Seemo widget, a dialog, an input. Content inside a page is never boxed, never carded, never shadowed. Profile sections are separated by `space-8` and a hairline, not by cards.

**Evidence is always visible.** Every fact in a profile shows where it came from. The evidence line (`body-sm`, `ink-muted`, URL in `mono`) is part of the field, not a tooltip.

**Motion answers an action.** The only ambient motion is the live crawl feed: new rows enter from opacity 0 over 160 ms, and the live dot pulses. Nothing else moves unless the user did something, and then only to show what changed (a field saving, the widget opening). Respect `prefers-reduced-motion`.

## Not in this product

These are the common tells of generated UI, and they are out:

- Small pill badges or uppercase eyebrows above headings ("NEW", "PROFILE", "STEP 1").
- Uppercase, letter-spaced labels of any kind. Labels are sentence case at `body-sm` or `heading`.
- Cards with shadows for ordinary content, and cards with a coloured left border.
- Gradients, glass blur, glow.
- Emoji as icons, and icons as decoration next to every heading.
- Accenting one word of a headline in blue or italic.
- Numbered markers (01, 02, 03) on anything that is not an actual sequence. Onboarding is a sequence; a list of profile sections is not.
- Middle-dot meta strings ("Pro · $49 · Monthly"). Use commas or separate lines.
- An arrow glued to link text ("Learn more →").
- Pure black (#000) or tinted near-black. Text is `ink`.
- Monospace for anything that is not literally code, a command or a URL.

## Colour

Light theme only. The page is `surface` white. `surface-raised` is for the sidebar, table headers and code blocks, and it is the only grey fill allowed. `blue-wash` is the only tinted fill: selected rows, the active project, Seemo's messages. Status colours (`success`, `warning`, `danger`) appear as text or a 6px dot beside text, never as filled badges.

Contrast: `ink` 16:1, `ink-muted` 5.6:1, `ink-faint` 3.0:1 on `surface` — so `ink-faint` is reserved for placeholders and disabled states, never for text someone must read. `blue` on `surface` and `on-blue` on `blue` are 8.1:1.

## Typography

One family: Manrope (Google Fonts, weights 300, 400, 500). Mono: JetBrains Mono, only for code.

| Role | Style | Weight |
| --- | --- | --- |
| Onboarding headline | `display-xl` 44/50 | 300 |
| Page title | `display` 32/38 | 300 |
| Section title | `title` 22/28 | 400 |
| Field name, table header | `heading` 16/24 | 500 |
| Reading text | `body` 15/24 | 400 |
| Meta, evidence | `body-sm` 13/20 | 400 |
| Timestamps, confidence | `caption` 12/16 | 400 |
| Commands, URLs | `mono` 13/20 | 400 |

Measure: reading text wraps at 72 characters (about 640px). Display headings may run wider but break on meaning, never mid-phrase. Headings are sentence case. Negative tracking only on display sizes, as in the tokens.

## Layout

Application shell: a 240px sidebar in `surface-raised` holding the project switcher at top, the navigation, and the user at the bottom. Content area is white, 64px top padding, 48px side gutters on desktop, 32px on tablet, 16px on phones. Content max width 960px, left aligned. Reading columns inside it are 640px.

The Seemo widget floats bottom right, 24px from both edges: a 48px `blue` disc launcher with Seemo's mark; opened, a 400 × 560px panel with `radius-lg` and `shadow-float`. On phones it becomes a full-height sheet.

Grid: 8px base. Every spacing value is a token; no 20px, no 36px.

### Onboarding

```
┌──────────────────────────────────────────────────────────┐
│                                                          │
│   Tell us your website.                                  │
│   Seemo does the rest.                     (display-xl)  │
│                                                          │
│   ┌────────────────────────────────┐  ┌──────────────┐   │
│   │ https://                       │  │ Start crawl  │   │
│   └────────────────────────────────┘  └──────────────┘   │
│   We'll read up to 200 pages and build a business        │
│   profile you can edit. Change the limit in settings.    │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

While crawling, the headline shrinks to `display`, the page list grows on the left (URL in `mono`, status dot, timing in `caption`) and extracted facts appear on the right as they are parsed, each with its evidence line. Seemo narrates the same events in the widget.

### Business profile

Sections stack vertically, separated by a hairline and `space-8`. Each section: `title`, a one-line description in `ink-muted`, then fields. A field is the field name at `heading`, the value at `body`, then one evidence line and a 2px confidence bar in `blue` on `blue-line`, 80px wide. Hovering a field reveals Edit and Lock as text buttons; locked fields show a lock glyph at `ink-faint` after the name. No cards.

## Writing

Seemo speaks as a colleague: first person, plain verbs, short sentences, no exclamation marks, no emoji. "I found your pricing page. Three tiers; Pro is $49 a month. Is that current?"

The interface itself is in the second person and sentence case. Buttons say what happens: "Start crawl", "Save changes", "Connect Gmail", "Re-crawl site". A button and its confirmation share a verb ("Save changes" → "Saved").

Errors say what happened and what to do, in one or two sentences, in `danger` text under the field. Empty states are an invitation in `display`, one sentence of help, and one button.

Name things by what the user understands: "project", "profile", "crawl", "connection", "task". Not "ingestion", "entity", "pipeline".

## Iconography

Line icons, 1.5px stroke, 20px grid, `ink-muted` by default, `blue` only when selected. Use the Lucide set so icons are consistent and need no maintenance. Icons sit beside text, never alone in content, and never beside headings. Seemo's mark is a `blue` disc with a white lowercase "s" set in Manrope 500 at 20px; a proper mark is a later task.

## Marketing site (kiqr.dev)

Same tokens. White, one blue, Manrope light at large sizes. The hero is the onboarding input itself, live: a URL field and "Start crawl" that opens the install instructions. No feature grid of six equal cards; feature sections are a `display` heading, a paragraph at `body`, and one screenshot, alternating sides. Code samples (the curl install line, CLI usage) in `mono` on `surface-raised` with `radius-sm`.

## Tokens

The CSS in `web/src/tokens.css` is generated from these values.

```json
{
  "name": "KIQR",
  "version": 1,
  "color": {
    "themes": [{ "id": "light", "name": "Light" }],
    "tokens": [
      { "name": "surface", "value": "#ffffff", "usage": "Page background. The product is white; nothing else is allowed as a page ground." },
      { "name": "surface-raised", "value": "#f6f8fb", "usage": "Sidebar, table header rows, code blocks, the Seemo widget's input bar. Never as a card fill inside content." },
      { "name": "line", "value": "#e3e8ef", "usage": "Hairline dividers between sections and table rows. One hairline, never a box around every element." },
      { "name": "line-strong", "value": "#c9d1dc", "usage": "Borders on inputs, selects and secondary buttons at rest." },
      { "name": "ink", "value": "#10213a", "usage": "Primary text, display headings, the KIQR wordmark. A deep navy, chosen because it sits in the blue family; not a tinted black." },
      { "name": "ink-muted", "value": "#5b687b", "usage": "Secondary text: descriptions, evidence quotes, table meta. 4.5:1 on surface." },
      { "name": "ink-faint", "value": "#8b95a7", "usage": "Placeholders, timestamps, disabled text. Not for anything the user must read." },
      { "name": "blue", "value": "#1442c4", "usage": "The one brand colour. Primary buttons, links, selected navigation, Seemo's avatar, confidence bars, live indicators. Used sparingly: at most one filled blue element per view." },
      { "name": "blue-hover", "value": "#0f36a3", "usage": "Primary button and link hover." },
      { "name": "blue-wash", "value": "#edf1fc", "usage": "Selected rows, the active project in the switcher, Seemo's message bubbles. The only tinted fill in the product." },
      { "name": "blue-line", "value": "#b9c8f2", "usage": "Confidence-bar track, focus ring outer halo, skeleton loaders." },
      { "name": "on-blue", "value": "#ffffff", "usage": "Text and icons on blue." },
      { "name": "success", "value": "#1f7a45", "usage": "Text and dot for 'done', 'connected', 'added' in diffs. Text only; never a filled badge." },
      { "name": "warning", "value": "#9a6a12", "usage": "Text and dot for low confidence and conflicts needing review." },
      { "name": "danger", "value": "#b4322b", "usage": "Destructive buttons, errors, 'removed' in diffs." },
      { "name": "focus", "value": "{blue}", "usage": "Keyboard focus ring: 2px focus + 2px surface offset. Visible on every interactive element." }
    ]
  },
  "type": {
    "fonts": [],
    "families": {
      "sans": "\"Manrope\", \"Segoe UI\", system-ui, sans-serif",
      "mono": "\"JetBrains Mono\", ui-monospace, Menlo, monospace"
    },
    "groups": [
      {
        "name": "Display",
        "family": "sans",
        "styles": [
          { "name": "display-xl", "fontSize": "44px", "lineHeight": "50px", "fontWeight": 300, "letterSpacing": "-0.02em", "sample": "Tell us your website. Seemo does the rest.", "usage": "Onboarding and empty-state headlines only. One per screen." },
          { "name": "display", "fontSize": "32px", "lineHeight": "38px", "fontWeight": 300, "letterSpacing": "-0.015em", "sample": "Business profile", "usage": "Page titles." },
          { "name": "title", "fontSize": "22px", "lineHeight": "28px", "fontWeight": 400, "letterSpacing": "-0.01em", "sample": "Positioning", "usage": "Profile section titles, dialog titles." }
        ]
      },
      {
        "name": "Text",
        "family": "sans",
        "styles": [
          { "name": "heading", "fontSize": "16px", "lineHeight": "24px", "fontWeight": 500, "sample": "Pricing tiers", "usage": "Field names, table headers, widget header. The heaviest weight in the product." },
          { "name": "body", "fontSize": "15px", "lineHeight": "24px", "fontWeight": 400, "sample": "Three tiers: Free, Pro at $49 per month, and Enterprise on request.", "usage": "Everything readable. Max 72 characters per line." },
          { "name": "body-sm", "fontSize": "13px", "lineHeight": "20px", "fontWeight": 400, "sample": "Found on /pricing, 4 minutes ago", "usage": "Meta lines, evidence sources, table cells with dense data." },
          { "name": "caption", "fontSize": "12px", "lineHeight": "16px", "fontWeight": 400, "sample": "Confidence 0.92", "usage": "Timestamps and confidence values, in ink-faint or ink-muted. Sentence case, never uppercase." }
        ]
      },
      {
        "name": "Code",
        "family": "mono",
        "styles": [
          { "name": "mono", "fontSize": "13px", "lineHeight": "20px", "fontWeight": 400, "sample": "kiqr project add https://unblockingapi.com", "usage": "CLI commands, URLs in the crawl feed, JSON. Only for things that are literally code or paths." }
        ]
      }
    ]
  },
  "spacing": {
    "tokens": [
      { "name": "space-1", "value": "4px", "usage": "Icon-to-label gap, dot-to-text gap." },
      { "name": "space-2", "value": "8px", "usage": "Between stacked meta lines; inside chips." },
      { "name": "space-3", "value": "12px", "usage": "Input and button vertical padding; list row padding." },
      { "name": "space-4", "value": "16px", "usage": "Between related fields; widget padding." },
      { "name": "space-6", "value": "24px", "usage": "Between a section title and its content." },
      { "name": "space-8", "value": "32px", "usage": "Between profile sections; page side gutters on small screens." },
      { "name": "space-12", "value": "48px", "usage": "Between page title and content; between major blocks on marketing pages." },
      { "name": "space-16", "value": "64px", "usage": "Page top padding; between marketing sections." }
    ]
  },
  "radius": {
    "tokens": [
      { "name": "radius-sm", "value": "4px", "usage": "Inputs, chips, code blocks, confidence bars." },
      { "name": "radius-md", "value": "6px", "usage": "Buttons, menus, message bubbles." },
      { "name": "radius-lg", "value": "10px", "usage": "The Seemo widget, dialogs. Nothing else is this round." },
      { "name": "radius-pill", "value": "999px", "usage": "Seemo's avatar and the floating widget launcher only." }
    ]
  },
  "shadow": {
    "note": "One shadow exists and it belongs to the one floating surface. Content never casts a shadow; hierarchy comes from spacing and hairlines.",
    "tokens": [
      { "name": "shadow-float", "value": "0 12px 32px rgba(16, 33, 58, 0.14), 0 1px 2px rgba(16, 33, 58, 0.08)", "usage": "The Seemo widget and dialogs. The only shadow in the product." }
    ]
  }
}
```

## Component: Button

Buttons name the action that happens: "Start crawl", "Save changes", "Connect Gmail". One primary (filled blue) per view; everything else is secondary (hairline border), text (blue, no border) or danger (hairline, red text — it only fills red inside a confirmation dialog).

- Weight 500 at `body` size, `radius-md`, 12px vertical and 20px horizontal padding, 48px tall.
- No icons inside buttons except a leading glyph for "Connect …" providers.
- Disabled replaces the label with the in-progress verb ("Saving", "Crawling") instead of a spinner where possible.
- Focus: 2px `focus` ring with 2px offset. Always visible on keyboard focus.

## Component: TextField

A text field is a label at `heading`, a hairline box with `radius-sm`, and an optional help line at `body-sm` in `ink-muted`. Errors turn the border `danger` and replace the help line with what went wrong and how to fix it.

- Labels are sentence case and sit above the field; never floating labels, never placeholder-as-label.
- Placeholders show a format ("https://"), not an instruction.
- Focus ring is the shared 2px `focus` ring; the border also turns `blue`.
- Width follows the content: URLs get the full reading column (640px); short values (a number of pages) get 120px.

## Component: ProfileField

A profile field is one fact about the business: a name, a value, where it came from, and how sure Seemo is. Fields stack inside a section separated by hairlines; there are no cards.

- Name at `heading`; value at `body`; evidence at `body-sm` in `ink-muted` with the source path in `mono` and a short quote.
- Confidence is a 2px, 80px bar in `blue` on `blue-line` with the number in `caption`. Below 0.6 the bar and text turn `warning` and the text says "needs review" — no badge.
- Edit and Lock are text buttons revealed on hover (always visible on touch devices). A locked field shows "Locked by you" after the name in `ink-faint` and keeps its value through re-crawls.
- Editing happens in place: the value becomes a text field in the same spot; Save and Cancel appear as text buttons under it.
- Lists (features, personas, competitors) render as one field per item with the same anatomy, so each item has its own evidence.

## Component: CrawlFeed

The crawl feed shows pages as they are discovered, fetched and parsed, so the user watches the profile being built. It is the one place in the product with ambient motion.

- Each row: a 6px status dot (`line-strong` queued, `blue` pulsing while fetching or parsing, `success` done, `danger` failed), the path in `mono`, and the result in `caption` ("9 facts, 1.1 s").
- New rows fade in over 160 ms; the list scrolls to keep the live row visible. With `prefers-reduced-motion`, rows appear without animation and the dot stops pulsing.
- The heading reads "Reading example.com" at `display` with the running count beside it. When done: "Read 183 pages" and the count disappears.
- The CLI (`kiqr crawl --watch`) prints the same rows as plain text; keep the wording identical.
- Facts extracted from a page appear to the right (desktop) or below (phone) as `ProfileField` rows, newest first.

## Component: SeemoWidget

The Seemo widget is the persona's home: a floating chat, present on every screen, scoped to the active project. It is the only floating, shadowed surface in the product, which is what makes it feel like a person in the room rather than another panel.

- Launcher: a 48px `blue` disc with Seemo's mark, 24px from the bottom-right corner. Opens to a 400 × 560 panel with `radius-lg` and `shadow-float`; a full-height sheet on phones.
- Header: avatar, "Seemo" at `heading`, the project name at `caption` under it, a close control. No status text like "online".
- Seemo's messages sit in `blue-wash` on the left; the user's in a hairline bubble on the right. Timestamps at `caption` in `ink-faint`.
- When Seemo proposes actions, they appear as small hairline buttons under the message, each naming the action ("Review 3 fields", "Re-read /pricing"). Actions that change something outside KIQR (send an email, post) always confirm first.
- Seemo writes in first person, plainly, with no emoji or exclamation marks. During a crawl it narrates what it finds; otherwise it waits.
- Opening and closing animate 160 ms from the launcher's corner. Nothing else in the widget moves.

