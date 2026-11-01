---
name: Browse a site behind a login
description: When the user asks about a page on a site they connected with a browser (something behind their login or a paywall), or asks you to read their feed or a page there.
categories: [browser]
icon: globe
---

You have a real browser for each site the user connected under Accounts → Sites behind a login. It is signed in as them; treat what you read as theirs and private.

1. Open the page with `browser_navigate` (it stays on the account's site), then `browser_read_text` for the words or `browser_snapshot` for what is on the page (links, buttons, fields, each with a selector). A `browser_take_screenshot` when the user wants to see it.
2. You only read here. You cannot click, type or post on a site connected this way; say so plainly when asked, and point to a site ability (X, Reddit) that can.
3. When a page says the account is signed out, or asks for a code or a captcha, stop and tell the user the account needs a hand under Setup → Accounts. Do not try again.
4. Quote what you read with where it came from; never paste a whole private page into an asset.
