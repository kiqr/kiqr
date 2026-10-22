---
name: build-profile
description: Merge the facts extracted from every crawled page into one section of the business profile, resolving conflicts and keeping evidence, as JSON.
---

You are writing one section of a business profile from facts extracted page by page. The result is what a marketing team will rely on, so it must be accurate, specific and complete, and every value must keep its evidence.

You receive: the section name and description, the current fields in that section (some may be locked), and the new facts for it, each with a quote, URL and confidence.

Rules:

- Merge facts about the same field into one field with one clear value. Prefer the most specific and most recent statement; when facts disagree, keep the best-supported value and mention the disagreement in the value ("The pricing page says $49; a 2025 blog post says $39").
- Keep every supporting quote and URL as evidence, up to five per field, most relevant first.
- `confidence` of a field is the confidence of its best evidence, lowered if facts disagree.
- Locked fields are set by a person. Return them unchanged, with their evidence unchanged.
- Write values as plain sentences or short lists, in the third person, without marketing adjectives the site did not use. Keep each value under 80 words except for lists.
- Field names are short, lowercase, and reused from the input where they exist.
- Do not invent a field the facts do not support. Do not drop a field that has evidence.

Return only JSON:

```json
{
  "fields": [
    {
      "name": "plans",
      "value": "Three tiers: Free with 1,000 credits, Pro at $49 per month with 50,000 credits, Enterprise on request.",
      "evidence": [{"url": "https://example.com/pricing", "quote": "Start free with 1,000 credits."}],
      "confidence": 0.95,
      "locked": false
    }
  ]
}
```
