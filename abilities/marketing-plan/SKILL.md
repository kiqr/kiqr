---
name: Plan the marketing
description: When the user asks about strategy, priorities or what to do next, or when a project has no plan yet; before proposing or changing the plan or a campaign.
---

You own the marketing plan. It is short, measurable and built from the brief, never from marketing clichés. Read `get_brief` and `get_plan` before you touch it.

## What a plan is

- A period in the user's words (a quarter, a month, "until the launch").
- A summary of two or three sentences: where the business stands and what this period is for.
- Two to four objectives. Each has a title, a `why`, one `kpi` from the fixed list, a `target` with its `unit` when something can measure it, and a date. Without a metrics source, leave the target out and say so in the summary rather than inventing a number.
- **The title is the result its KPI counts**, in words the owner would use, with the target when there is one. The plan shows the title above a bar of that KPI, so they must tell the same story: someone who reads only the title knows what the bar measures.
  - Good: "Double organic visits to 150 a month" (`organic_traffic`), "Win 10 café enquiries from the wholesale page" (`qualified_leads`), "Publish two posts a week on Instagram" (`published`), "Rank on page one for the five template queries" (`rank_positions`).
  - Not: "Own the real cost per 1,000 rendered requests comparison" (a tactic, in jargon, and the bar under it counts visits), "Do SEO", "Build authority".
- **The `why` is two short sentences**: why this result matters, from the brief (an offering, an audience, a gap, a differentiator), and how you will get there (the tactic, e.g. comparison and migration pages). Bigger tactics become campaigns under the objective.
- The channels in play: only ones the project has or the user named. The brief's `channels` section and `list_channels` say what exists.

## How to build it

1. Start from the brief: offerings, audiences, positioning, differentiators, content gaps, competitors. Pick the objectives a sharp marketer would pick for this business, not generic ones.
2. One KPI per objective. Prefer what can be measured here: `published` always can; `organic_traffic`, `qualified_leads`, `signups`, `conversion_rate` need a metrics source; `brand_mentions` needs community channels; `rank_positions` needs rank tracking.
3. Name trade-offs in the summary: what this period is not about.
4. Call `propose_plan`. The card is the question; do not paste the plan in text and ask "shall I?". When it replaces a plan, say in `reason` what changes and why.
5. After a yes, answer in two lines: the plan is in, and the first thing you will do for it.

## Campaigns

A campaign is one initiative towards one objective: a launch, a content series, a community push, a newsletter sequence. It has a goal, the channels it uses and dates. Propose it with `create_campaign` when a piece of work is bigger than one asset or one task, and put what you make for it into it (`campaign` on `create_asset` and `create_task`, or `attach_to_campaign`). Keep campaigns few; most work is a task under an objective, not a campaign.

## When asked how it is going

Compare against the objectives: what was made, what went out, what the numbers say when there are any, what you would change. Say plainly when something did not work. Propose changes to the plan with `propose_plan` only when the facts changed, not every week.
