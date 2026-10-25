---
name: extract-facts
description: Read one crawled page as Markdown and return every fact about the business it states, as JSON, each fact with a verbatim quote as evidence.
---

You are reading one page of a company's website to learn about the business. Return every fact the page states or clearly shows about the company, its products, pricing, audience, positioning, brand voice, competitors, SEO, channels, content, integrations and trust signals.

Rules:

- Only facts the page supports. Never guess or fill in from general knowledge. If the page is a blog post, the facts are about the company that published it, not about the post's subject.
- Each fact gets a verbatim `quote` from the page, at most 30 words, that supports it.
- `confidence` is 0 to 1: 1.0 for a plain statement ("Pro costs $49/month"), 0.5 for something implied, 0.3 for a hint.
- `section` is one of: company, products, pricing, audience, positioning, voice, competitors, seo, channels, content, integrations, trust.
- `field` is a short lowercase name for what the fact is about, reused consistently: "name", "tagline", "founded", "plans", "free_tier", "trial", "personas", "industries", "value_props", "differentiators", "tone", "sample_sentences", "named_competitors", "keywords", "social", "newsletter", "posts", "integrations", "stack", "testimonials", "logos", "certifications". Invent a new one only when none fits.
- For `voice`, the value is a description of the tone and the quote is a sentence that shows it. Return at most three voice facts per page.
- For `content`, return one fact per article or case study, with its title and date if shown.
- Skip navigation, cookie notices, legal boilerplate, and anything about the website itself rather than the business.

Return only a JSON array:

```json
[
  {"section": "pricing", "field": "plans", "value": "Three tiers: Free, Pro at $49 per month, Enterprise on request", "quote": "Start free. Upgrade to Pro for $49/mo.", "confidence": 1.0}
]
```

An empty page or a page with nothing about the business returns `[]`.
