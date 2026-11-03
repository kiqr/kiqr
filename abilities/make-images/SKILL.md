---
name: Create images with the built-in renderer
description: "How to make the featured image for a post, an article cover or a standalone visual: a real picture of the business from Library → Media when there is one, a designed card otherwise, a generated picture only for a mood or a concept, never passed off as the business's own. With the built-in renderer (SVG, or HTML when a browser is installed) and, when one is connected, a generator."
abilities: [image]
---

# Making images

Everything gets a featured image: every post, every article cover, every email header, all of them on brand. You design, KIQR renders. `create_image` takes an SVG (or HTML) you write and turns it into a PNG on an asset. Nothing is installed, nothing is paid. When a generator is connected, the same tool also takes a `prompt`; `source` still renders here, free.

The one rule above every other: **an image never claims something that is not true.** A real picture of the business comes only from the business itself, through Library → Media. A generated picture shows a mood or a concept and is never a stand-in for the business's products, premises, people or results. When the honest image is a card of words, a card of words it is.

## The process

1. `brand_kit`: colours with roles, fonts as they will render here, the logo and icon as `file:` URLs, the site name and tagline, the preset sizes, and `media`: the business's own pictures from Library → Media, each a `file:` URL with its alt, size and orientation.
2. Pick the picture, in this order (next section).
3. Pick the size for where it goes: the channel kind it is for (a post's tag; `brand_kit` lists the sizes the project's networks want), else `landscape` 1600×900, `square` 1080×1080, `portrait` 1080×1350, `og` 1200×630 or `cover` 1600×900.
4. Pick a layout from the library below and make it this image's own: the words, the picture, the alignment, the scale. Fewer words than you think.
5. Call `create_image` with `asset` (the post or article it belongs to; omit it for a standalone image and give a `title`), `name` (e.g. `cover`), `format`, `source`, `size`.
6. Read the result: the file name, its URL, which renderer ran. Mention the image in one line to the user; never paste the SVG into the chat.

## Which picture

Take the first that applies.

1. **A real picture of the thing the post is about.** Look through `media` for a picture whose alt or page matches the subject (the product, the place, the team at work, the result). None there but the post links to a page on the business's own site? `read_url` it: its `images` list the page's pictures; `add_media` with the right `url` puts one in the Library (own hosts only; anything else is refused with the reason), and it is yours to use. Use it as it is: crop, never alter, and let the caption say only what the alt or the page says.
2. **A real picture of the business for mood.** Its premises, its people at work, its materials, from the same `media`, when the post is about something that has no picture of its own but the business does.
3. **A generated mood or concept picture**, only when a generator is connected and the subject is abstract (planning, a season, a feeling, an idea). Rules under "With a generator". Never for the product.
4. **A designed card.** Words on brand colour, with the brand's mark. For claims, numbers, tips, announcements, quotes, and for a product with no real picture. In that last case tell the user, in your one line, that a picture of it under Library → Media would do better next time.

## Honesty

- A real picture is shown as it is. Cropping and a colour wash over it are fine; nothing is added, removed or retouched.
- A generated picture never shows what the business sells, its place, its people or its results, and never a photoreal scene a reader would take for the business. Illustrative, abstract or graphic styles for anything near the business's domain; no text, no logos, no products, no packaging, no faces presented as customers or staff.
- Nothing invented on a card either: no numbers, awards, badges, "as seen on", testimonials or quotes that are not in the brief.
- When a post is about a product and there is no real picture, the image is a card. Say so, and that a real picture would do better.

## Variety across a batch

A batch of posts from one business must look like that business, not like a template.

- Never the layout of either of the two previous images. At least one picture-led image when `media` is not empty.
- Backgrounds rotate: dark, light, accent, a picture.
- Change the alignment (left, centred, low on the page), the scale (one huge word, three calm lines), and the brand's presence (the bar, the rule, the mark large and faded, the logo small).
- One bold move per image, not three.
- The brand's own fonts when `brand_kit` says they render; Inter otherwise.

## Rules for SVG

- Set `width`, `height` and `viewBox="0 0 W H"` to the preset size.
- Text does not wrap by itself. Break lines yourself with one `<text>` per line (or `<tspan x=... dy=...>`). At 1600 wide: headline 72px, at most 7 words per line, at most 3 lines; supporting line 34px, at most 12 words; line height 1.2.
- Keep 8% margins on every side. Nothing touches the edge except backgrounds and full-bleed pictures.
- Use the brand's first colour as the background or as the accent, never both. Text on a dark background is the brand's lightest colour or white; on a light one, the darkest or near-black. Contrast at least 4.5:1. Words over a picture always sit on a scrim.
- Fonts: use the family names `brand_kit` says will render; fall back to `Inter, sans-serif`. Set `font-family` on the root `<g>` or `<svg>`.
- Pictures: `<image href="file:///…" preserveAspectRatio="xMidYMid slice"/>` fills a box and crops; `meet` fits inside. Only `file:` URLs from `brand_kit` (`media`, `logo`, `icon`) and `data:` URIs. No external URLs for fonts or images.
- The logo: height 5–7% of the image height, in a corner, inside the margin. Omit it when the kit has none.
- No emoji. No animations. The one filter allowed is `feGaussianBlur` on a background picture (the "picture under a wash" layout).
- Keep it readable at thumbnail size: one message, large.

## HTML, when a browser is available

`brand_kit` says whether HTML rendering is on. Then you may write a full HTML document sized with `<html style="width:Wpx;height:Hpx;margin:0">`, `body{margin:0}`, CSS gradients, flex layout, `object-fit: cover` for pictures and automatic text wrapping. Put the kit's `font_css` in a `<style>` so Inter renders in the browser too. Same rules on margins, contrast, fonts, pictures and no external requests. Prefer HTML for long quotes and lists; SVG for everything short.

## Layouts (1600×900; scale coordinates for other sizes)

Each is a skeleton. Replace the colours with the brand's, the words with the post's, the picture with one from `media`, the site name with the brand's; add the logo when the kit has one.

### Picture-led

**Full bleed with a scrim** – the picture is the message; a few words low on it.
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <image href="file:///…/media/team-at-work/team.jpg" width="1600" height="900" preserveAspectRatio="xMidYMid slice"/>
  <defs><linearGradient id="s" x1="0" y1="0" x2="0" y2="1"><stop offset="0.35" stop-color="#0B1220" stop-opacity="0"/><stop offset="1" stop-color="#0B1220" stop-opacity="0.9"/></linearGradient></defs>
  <rect width="1600" height="900" fill="url(#s)"/>
  <rect x="128" y="600" width="10" height="120" rx="5" fill="#14B8A6"/>
  <text x="172" y="660" fill="#FFFFFF" font-size="68" font-weight="700">Made by hand, every Tuesday,</text>
  <text x="172" y="738" fill="#FFFFFF" font-size="68" font-weight="700">in our own kitchen.</text>
  <text x="128" y="776" fill="#FFFFFF" font-size="30" font-weight="600" letter-spacing="1">SITE NAME</text>
</svg>
```

**Split** – the picture one side, the words the other; swap sides between posts.
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <rect width="1600" height="900" fill="#F8FAFC"/>
  <defs><clipPath id="p"><rect x="0" y="0" width="720" height="900"/></clipPath></defs>
  <image href="file:///…/media/workshop/bench.jpg" x="0" y="0" width="720" height="900" preserveAspectRatio="xMidYMid slice" clip-path="url(#p)"/>
  <rect x="816" y="300" width="64" height="8" rx="4" fill="#14B8A6"/>
  <text x="816" y="400" fill="#0B1220" font-size="60" font-weight="700">What a good</text>
  <text x="816" y="470" fill="#0B1220" font-size="60" font-weight="700">workbench needs</text>
  <text x="816" y="540" fill="#475569" font-size="30">Three things, none of them expensive.</text>
  <text x="816" y="776" fill="#14B8A6" font-size="26" font-weight="600" letter-spacing="2">SITE NAME</text>
</svg>
```

**Picture with a panel** – the picture fills the frame, the words sit in a solid block in one corner.
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <image href="file:///…/media/storefront/front.jpg" width="1600" height="900" preserveAspectRatio="xMidYMid slice"/>
  <rect x="128" y="500" width="760" height="272" rx="16" fill="#0B1220"/>
  <text x="176" y="580" fill="#14B8A6" font-size="24" font-weight="700" letter-spacing="3">NOW OPEN</text>
  <text x="176" y="650" fill="#FFFFFF" font-size="52" font-weight="700">Saturdays too,</text>
  <text x="176" y="712" fill="#FFFFFF" font-size="52" font-weight="700">from 10.</text>
</svg>
```

**Framed picture on colour** – a smaller picture in a frame, a caption beside it; for a product shot that must stay whole (`meet`, not `slice`).
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <rect width="1600" height="900" fill="#14B8A6"/>
  <rect x="128" y="128" width="700" height="644" rx="20" fill="#FFFFFF"/>
  <image href="file:///…/media/the-small-kit/kit.jpg" x="152" y="152" width="652" height="596" preserveAspectRatio="xMidYMid meet"/>
  <text x="900" y="400" fill="#0B1220" font-size="64" font-weight="800">The small kit,</text>
  <text x="900" y="476" fill="#0B1220" font-size="64" font-weight="800">back in stock.</text>
  <text x="900" y="550" fill="#0B1220" font-size="30">Ships this week.</text>
  <text x="900" y="776" fill="#0B1220" font-size="30" font-weight="600" letter-spacing="2">SITE NAME</text>
</svg>
```

### Typographic

**Statement** – a claim in two lines beside a bar (left-aligned).
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

**Number** – one figure from the brief, huge, and what it describes.
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

**List** – three short lines, numbered.
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

**Pull quote** – centred, a large opening mark, the source below. Only words that are in the brief or on the site.
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <rect width="1600" height="900" fill="#0B1220"/>
  <text x="800" y="300" fill="#14B8A6" font-size="200" font-weight="800" text-anchor="middle" opacity="0.5">“</text>
  <text x="800" y="430" fill="#FFFFFF" font-size="56" font-weight="600" text-anchor="middle">We fix it the same day,</text>
  <text x="800" y="500" fill="#FFFFFF" font-size="56" font-weight="600" text-anchor="middle">or we say so by noon.</text>
  <text x="800" y="600" fill="#9CA3AF" font-size="30" text-anchor="middle">The promise on our service page</text>
  <text x="800" y="776" fill="#14B8A6" font-size="26" font-weight="600" letter-spacing="3" text-anchor="middle">SITE NAME</text>
</svg>
```

**One word** – a single word or two, as large as the frame allows, the point in a small line under it.
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <rect width="1600" height="900" fill="#14B8A6"/>
  <text x="128" y="560" fill="#0B1220" font-size="260" font-weight="800" letter-spacing="-12">Slower.</text>
  <text x="136" y="650" fill="#0B1220" font-size="34">Why we stopped doing same-day rush orders.</text>
  <text x="128" y="776" fill="#0B1220" font-size="30" font-weight="600" letter-spacing="2">SITE NAME</text>
</svg>
```

**Editorial with a kicker** – a small kicker word, the title in two lines, the promise; the article cover's shape.
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

### Motif

**The mark, large and faded** – the brand's icon as a shape behind the words; the business's own geometry, not a template's.
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <rect width="1600" height="900" fill="#F8FAFC"/>
  <image href="file:///…/brand/icon.png" x="1000" y="60" width="560" height="560" preserveAspectRatio="xMidYMid meet" opacity="0.12"/>
  <text x="128" y="150" fill="#14B8A6" font-size="26" font-weight="600" letter-spacing="4">SITE NAME</text>
  <text x="128" y="620" fill="#0B1220" font-size="84" font-weight="800" letter-spacing="-2">Open late</text>
  <text x="128" y="714" fill="#0B1220" font-size="84" font-weight="800" letter-spacing="-2">on Thursdays.</text>
</svg>
```

**Picture under a wash** – a real picture blurred and tinted as texture, so even a card of words is this business's; the words carry the message.
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900" viewBox="0 0 1600 900" font-family="Inter, sans-serif">
  <defs><filter id="b"><feGaussianBlur stdDeviation="28"/></filter></defs>
  <image href="file:///…/media/workshop/bench.jpg" x="-80" y="-80" width="1760" height="1060" preserveAspectRatio="xMidYMid slice" filter="url(#b)"/>
  <rect width="1600" height="900" fill="#0B1220" opacity="0.78"/>
  <text x="800" y="420" fill="#FFFFFF" font-size="76" font-weight="800" text-anchor="middle">Repairs, not replacements.</text>
  <text x="800" y="500" fill="#CBD5E1" font-size="34" text-anchor="middle">What we have done since 1998.</text>
  <text x="800" y="776" fill="#14B8A6" font-size="26" font-weight="600" letter-spacing="3" text-anchor="middle">SITE NAME</text>
</svg>
```

## Article covers

Always a cover, `name: cover`, `size: cover`. The editorial layout, or a picture-led one when Media has a picture of the subject; a kicker word (GUIDE, HOW-TO, UPDATE), the title in at most two lines, the brand mark. The cover keeps the title legible, so render it even when a generator is connected; a generated picture is for the body or when the user asks.

## With a generator

When a generator is connected (fal.ai, OpenAI), `create_image` takes `prompt` for a generated picture and still takes `source` for a design rendered here. Write the prompt as a brief to an illustrator or a photographer of moods: subject (a setting, a texture, a season, an abstract idea), mood, light, style, what to avoid; the brand's palette as a hint; no text inside the image. It shows a concept, never the business: not its product, not its premises, not its people, not a result it promises; for anything near its domain, an illustrative or graphic style a reader will not take for a photo of the business. Say what it costs before making more than three. For an image that must carry words (a quote, a number), design it: a generated picture can be the background, landing on the asset first, then a `source` design embeds it with its `file:` path and puts the words on a scrim.

## What to tell the user

One line: what you made, which picture it rests on (a real one from Media, a generated mood, or a card), where it is (the asset), and the size. When a real picture would have done better, say so. Offer one change you would try next, not a list.
