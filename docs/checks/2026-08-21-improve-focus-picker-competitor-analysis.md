# focus-picker — competitor analysis (2026-08-21)

Scan run **before** implementation, per `/create-next-tool` step 4. All findings are
**paraphrased** — no competitor copy, branding, or trademarked wording was reused. The
tool's own copy, scoring model, and page design are original.

Backlog row: `focus-picker` — "Selects the single most important task from a list by
priority, due date, and effort, with a brief justification." Type hint: `pure`.

## Competitors reviewed

| # | Tool | Shape | Free / no-account |
|---|------|-------|-------------------|
| 1 | mytimecalculator.com — Task Priority Calculator | per-task form rows, Eisenhower + weighted + RICE-style blend | yes |
| 2 | aipmtools.org — Prioritization Calculator | multi-framework grid (RICE / WSJF / MoSCoW / ICE) | yes |
| 3 | easyretro.io — WSJF Calculator | single-framework grid, WSJF only | yes |
| 4 | productlift.dev — RICE Score Calculator | single-item RICE score + interpretation bands | yes |
| 5 | calculatorr.com — Task Prioritization Matrix | Eisenhower quadrants + optional due date / est. minutes / category | yes |

## What they collectively ship (table stakes)

**Scoring methods.** Every tool commits to a named, published formula rather than a
black box. The recurring four:

- *Eisenhower quadrants* — urgent × important → Do first / Schedule / Delegate /
  Eliminate, each with an action label (#1, #5).
- *Weighted blend* — a 0–100 composite of urgency and importance, with a user-selectable
  weight preference (#1 offers balanced / lean-urgency / lean-importance).
- *WSJF* — `(business value + time criticality + risk) ÷ job size`, all on 1–10; job size
  is inverse effort (#2, #3).
- *RICE / ICE* — `reach × impact × confidence ÷ effort`, with confidence as an explicit
  discount factor; #4 publishes interpretation bands (roughly: triple digits = do it,
  under ~20 = shelve it).

**Scales and defaults.** 1–5 (urgency/importance) and 1–10 (impact/effort/confidence/
job size) dominate; effort is always "bigger = worse" and is always the divisor. #2
documents concrete anchors per scale point (confidence 100 % = validated, 50 % = a hunch;
effort in person-months). #4 publishes an anchor table per scale point. Several tools ship
NO defaults at all, which means an empty grid computes nothing — a UX weakness, not a
model to copy.

**Per-task optional metadata.** Due date, estimated time (minutes), and a category /
project tag (#5, #1). Optional-by-design: rows with partial data still score (#1).

**Outputs.** Ranked table (rank, task, quadrant, score, tier, suggested action, due
date), a designated top pick, and roll-ups — task count, total estimated time, share of
tasks in the "do first" quadrant, average score (#1, #5).

**UX patterns.** Add/remove row buttons; instant recompute; sort-by-score toggle; reset;
share-link that encodes the whole grid in the URL; CSV and Markdown export; print view;
a worked example with a real number attached (#4's example lands at 216); a
common-mistakes section; FAQ covering framework choice, recalculation cadence, and
interpretation.

## In-model decisions (what focus-picker ships)

gizza is browser-local, wasm, no account, no server, one text field in / text out. That
rules out grid UIs, but the *substance* of every table-stake above is expressible in a
line-oriented syntax, so nothing was dropped silently.

| Table stake | Decision |
|---|---|
| Named, published formulas | **Built.** `method` = `balanced` (default) / `deadline` / `wsjf` / `quick-wins` / `eisenhower`. Every weight is printed on the page, not hidden. |
| Eisenhower quadrants + action labels | **Built** as `method=eisenhower`; each ranked row carries its quadrant and the matching action (Do first / Schedule / Delegate / Drop). |
| Weight preference (lean urgency vs importance) | **Built** as distinct methods rather than a slider — `deadline` leans urgency, `quick-wins` leans effort, `balanced` splits 45/35/20. Same expressive power, one fewer knob. |
| WSJF `(value + criticality) ÷ job size` | **Built.** Priority supplies value, due-date proximity supplies time criticality, parsed effort supplies job size. |
| Effort as the divisor, "bigger = worse" | **Built.** `est:`/`effort:` accepts `90m`, `1.5h`, `2d`, or a bare hour count; effort always lowers the score. |
| Optional per-task metadata (due, estimate) | **Built** as inline tags (`due:2026-08-25`, `est:2h`, `!p1`) *and* as `|`/tab-delimited columns, so a spreadsheet paste works unchanged. |
| Partial rows still score | **Built** via `default_priority` + `default_effort`, both user-visible params. Tasks with no due date get a neutral urgency, not a zero. |
| Overdue handling | **Built** as `overdue_boost` (on by default) — competitors mostly treat a past due date as merely "very urgent"; pinning overdue work to the top is the differentiator. |
| Designated top pick + justification | **Built** — this is the tool's whole point, and the one thing no competitor does directly: they hand back a sorted grid and leave the reading to you. Output leads with the pick and a one-sentence reason naming the priority, the due date, the effort, and the score. |
| Ranked table of the rest | **Built** via `show_ranking` (on by default). |
| Roll-ups (counts, total effort) | **Built** — the summary line reports task count, total estimated effort, and the overdue count. |
| Markdown / CSV export | **Partly built.** `format` = `text` / `markdown` / `json`. Markdown gives a paste-able table; JSON is the machine-readable export. A separate CSV mode was judged redundant next to JSON on a one-field tool. |
| Share link encoding the grid | **Already ours.** Every page param is a query param and the page auto-runs, so a deep link *is* the share link — no bespoke encoder needed. |
| Anchor tables for each scale point | **Built as page copy** — the priority ladder, the due-date urgency curve, and the effort-ease curve are all documented with worked numbers. |
| Preset chips / sample data | **Built** as four `[[example]]` chips, each pinning an explicit `today` so the output is reproducible. |
| Reset / copy result | **Already ours** — the shared page chrome provides both. |

## Considered, not built (out of model)

- **Multi-user team calibration / voting** (#3's core guidance) — needs accounts and a
  backend.
- **Persisted boards, saved sessions, revision history** (#1, #2) — no server, no
  storage; the deep link is the durable artifact.
- **Print-formatted matrix view** (#5) — the browser's own print of the rendered page
  covers this; a bespoke print stylesheet is site-repo chrome, not toolkit scope.
- **True RICE with a `reach` term** — reach (users per quarter) is a product-management
  input, not a personal-task input; forcing it onto a to-do line would be schema bloat
  for the audience this tool serves. WSJF already covers the value ÷ size shape.
- **CSV export mode** — see above; JSON covers the machine-readable case.
- **Dependency detection between tasks** (the Prioritize123 app) — would need a task
  graph syntax; out of scope for a one-line-per-task paste.

## Considered, rejected (in model, declined on judgment)

- **A free-form weights parameter** (`weights=0.5,0.3,0.2`) — expressible, but it turns a
  self-explanatory `method` enum into an opaque triple and makes the printed formula
  unverifiable. The five named methods cover the useful corners of that space.
- **`kind = "tag-list"` for the task field** — task lines routinely contain commas, which
  is exactly the case the pill control mangles (the same call recorded for other
  bulk-paste list fields). Plain `multiline` textarea kept.

## Limits worth stating on the page (all are)

Deterministic keyword/tag parsing only — nothing is inferred by a model, and no date is
invented. 500-task cap. `due:` accepts ISO `YYYY-MM-DD`, `today`/`tomorrow`/`yesterday`,
`+Nd`/`+Nw`, and weekday names; ambiguous `M/D` forms are deliberately unsupported
because they mean different dates in different locales. `2d` of effort means two 8-hour
working days.
