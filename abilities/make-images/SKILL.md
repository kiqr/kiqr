---
name: Create images with the built-in renderer
description: How to make an on-brand image for a post, an article cover or a standalone visual with the built-in renderer (SVG, or HTML when a browser is installed), or with a generator when the user wants a photo or an illustration.
abilities: [image]
---

# Making images

Everything gets a featured image: every post, every article cover, every email header, all of them on brand. You design, KIQR renders. `create_image` takes an SVG (or HTML) you write and turns it into a PNG on an asset. Nothing is installed, nothing is paid. When a generator connection is active, the same tool takes a `prompt` instead; its description says which shape it expects.

## The process

1. `brand_kit`: colours (most characteristic first), fonts as they will render here, the logo and icon as `file:` URLs, the site name and tagline, and the preset sizes.
2. Pick the size for where it goes: `x` 1600×900, `linkedin` 1200×627, `instagram` 1080×1080, `instagram_portrait` 1080×1350, `og` 1200×630, `cover` 1600×900.
3. Pick a template below and adapt it: the headline, one supporting line, the brand mark. Fewer words than you think.
4. Call `create_image` with `asset` (the post or article it belongs to; omit it for a standalone image and give a `title`), `name` (e.g. `cover`), `format`, `source`, `size`.
5. Read the result: the file name, its URL, which renderer ran. Mention the image in one line to the user; never paste the SVG into the chat.

## Batches and covers

- A batch of posts: one image each, no exceptions. Vary the template and the background (one dark, one light, one on the accent), never the same template twice in a row, and size each for its network.
- An article: always a cover, `name: cover`, `size: cover`. A kicker word (GUIDE, HOW-TO, UPDATE), the title in at most two lines, the brand mark. The cover keeps the title legible, so render it even when a generator is connected; a generated photo is for the body or when the user asks.
- A post that needs a photo or an illustration rather than words: only with a generator connected. Without one, a designed card is the image; say so if the user asked for a photo.

## Rules for SVG

- Set `width`, `height` and `viewBox="0 0 W H"` to the preset size.
- Text does not wrap by itself. Break lines yourself with one `<text>` per line (or `<tspan x=... dy=...>`). At 1600 wide: headline 72px, at most 7 words per line, at most 3 lines; supporting line 34px, at most 12 words; line height 1.2.
- Keep 8% margins on every side. Nothing touches the edge except backgrounds.
- Use the brand's first colour as the background or as the accent, never both. Text on a dark background is the brand's lightest colour or white; on a light one, the darkest or near-black. Contrast at least 4.5:1.
- Fonts: use the family names `brand_kit` says will render; fall back to `Inter, sans-serif`. Set `font-family` on the root `<g>` or `<svg>`.
- The logo: `<image href="file:///…/logo.png" x= y= width= height= preserveAspectRatio="xMinYMid meet"/>` with a height of 5–7% of the image height, in a corner, inside the margin. Omit it when the kit has none.
- No external URLs for fonts or images; only `file:` paths from `brand_kit` and `data:` URIs. No emoji. No filters or animations.
- Keep it readable at thumbnail size: one message, large.

## HTML, when a browser is available

`brand_kit` says whether HTML rendering is on. Then you may write a full HTML document sized with `<html style="width:Wpx;height:Hpx;margin:0">`, `body{margin:0}`, CSS gradients, flex layout and automatic text wrapping. Put the kit's `font_css` in a `<style>` so Inter renders in the browser too. Same rules on margins, contrast, fonts and no external requests. Prefer HTML for long quotes and lists; SVG for everything short.

## Templates (1600×900; scale coordinates for other sizes)

**Quote or claim**
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <rect width="1600" height="900" fill="#0B1220"/>
  <rect x="128" y="128" width="12" height="644" fill="#14B8A6"/>
  <text x="188" y="330" fill="#FFFFFF" font-size="72" font-weight="700">First line of the claim,</text>
  <text x="188" y="416" fill="#FFFFFF" font-size="72" font-weight="700">second line, short.</text>
  <text x="188" y="520" fill="#9CA3AF" font-size="34">One supporting sentence that says why it matters.</text>
  <text x="188" y="744" fill="#14B8A6" font-size="30" font-weight="600">SITE NAME</text>
</svg>
```

**Number or statistic**
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <rect width="1600" height="900" fill="#F8FAFC"/>
  <text x="128" y="430" fill="#0B1220" font-size="220" font-weight="800" letter-spacing="-8">87%</text>
  <text x="128" y="520" fill="#0B1220" font-size="48" font-weight="600">of what the number describes</text>
  <text x="128" y="590" fill="#475569" font-size="32">Where it comes from, in a few words.</text>
  <rect x="128" y="716" width="1344" height="4" fill="#14B8A6"/>
  <text x="128" y="776" fill="#0B1220" font-size="30" font-weight="600">SITE NAME</text>
</svg>
```

**Announcement**
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#0B1220"/><stop offset="1" stop-color="#1E293B"/></linearGradient></defs>
  <rect width="1600" height="900" fill="url(#g)"/>
  <rect x="128" y="128" rx="22" width="230" height="44" fill="#14B8A6"/>
  <text x="243" y="159" fill="#0B1220" font-size="24" font-weight="700" text-anchor="middle">NEW</text>
  <text x="128" y="400" fill="#FFFFFF" font-size="84" font-weight="800">What is new,</text>
  <text x="128" y="496" fill="#FFFFFF" font-size="84" font-weight="800">in one line.</text>
  <text x="128" y="590" fill="#CBD5E1" font-size="34">Who it is for and what changes for them.</text>
  <text x="128" y="776" fill="#14B8A6" font-size="30" font-weight="600">SITE NAME</text>
</svg>
```

**Three tips (short lines only)**
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <rect width="1600" height="900" fill="#FFFFFF"/>
  <text x="128" y="200" fill="#0B1220" font-size="60" font-weight="800">Three things to check first</text>
  <g font-size="36" fill="#0B1220">
    <circle cx="150" cy="330" r="22" fill="#14B8A6"/><text x="138" y="343" fill="#FFFFFF" font-size="26" font-weight="700">1</text><text x="200" y="343">First tip, under eight words.</text>
    <circle cx="150" cy="450" r="22" fill="#14B8A6"/><text x="138" y="463" fill="#FFFFFF" font-size="26" font-weight="700">2</text><text x="200" y="463">Second tip, same length.</text>
    <circle cx="150" cy="570" r="22" fill="#14B8A6"/><text x="138" y="583" fill="#FFFFFF" font-size="26" font-weight="700">3</text><text x="200" y="583">Third tip, the one people forget.</text>
  </g>
  <text x="128" y="776" fill="#64748B" font-size="30" font-weight="600">SITE NAME</text>
</svg>
```

**Article cover**
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <rect width="1600" height="900" fill="#0B1220"/>
  <circle cx="1380" cy="220" r="420" fill="#14B8A6" opacity="0.16"/>
  <circle cx="1480" cy="760" r="260" fill="#14B8A6" opacity="0.10"/>
  <text x="128" y="190" fill="#14B8A6" font-size="28" font-weight="700" letter-spacing="4">GUIDE</text>
  <text x="128" y="420" fill="#FFFFFF" font-size="80" font-weight="800">Article title,</text>
  <text x="128" y="514" fill="#FFFFFF" font-size="80" font-weight="800">two lines at most</text>
  <text x="128" y="600" fill="#CBD5E1" font-size="34">The promise of the article in one sentence.</text>
  <text x="128" y="776" fill="#9CA3AF" font-size="30" font-weight="600">SITE NAME</text>
</svg>
```

Replace the colours with the brand's, the words with the post's, and the site name with the brand's; add the logo when the kit has one.

## With a generator

When the active connection is a generator (fal.ai, OpenAI), `create_image` takes `prompt`. Write it as a brief to a photographer or illustrator: subject, setting, mood, light, style, what to avoid; no text inside the image (renderers add words badly); the brand's palette as a hint. Say what it costs before making more than three. For an image that must carry words (a quote, a number), use the built-in renderer instead, or ask for the generator image as a background and tell the user the words would need a design pass.

## What to tell the user

One line: what you made, where it is (the asset), and the size. Offer one change you would try next, not a list.
