---
name: crawl-site
description: How KIQR reads a website: which pages first, what it skips, when it escalates to UnblockingAPI, and what it keeps in the cache.
---

The crawler is code, not a model, but these are the rules it follows so you can predict and adjust what it does.

- Starts from the project URL, `sitemap.xml` and `robots.txt`. Follows links on the same host only (`www.` is ignored).
- Reads up to `max_pages` pages (200 by default, per project or in `config.yaml`). Pricing, about, product, features, customers, comparison and FAQ pages are read first; deep paths last.
- Skips files (PDF, images, scripts), `mailto:`, `tel:`, fragments, and tracking parameters (`utm_*`, `fbclid`, `gclid`, `ref`).
- Converts each page to Markdown with navigation, headers, footers, scripts and forms removed, and stores the raw HTML, the Markdown and a small YAML record in `projects/<slug>/cache/pages/` (ignored by git).
- A page that returns 401, 403, 429 or 503, shows a bot challenge, or has almost no text next to a lot of script is marked blocked. If `UNBLOCKINGAPI_KEY` is set and escalation is on, the page is fetched again through UnblockingAPI, and the rest of that crawl goes through it directly. Otherwise the page is reported as unreadable and the crawl continues.
- Every page that was read is passed to `extract-facts` immediately, so facts appear while the crawl is still running.
