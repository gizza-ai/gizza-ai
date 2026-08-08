# cumulative-percent-builder — competitor analysis (2026-08-08)

Pre-implementation scan for the new tool "Sorts values descending and adds running count,
cumulative sum, and cumulative percentage columns for Pareto analysis."

**All notes below are paraphrased.** No competitor copy, branding, assets, or trademarks were
copied into this repo.

Search: one `WebSearch` for online Pareto / 80-20 cumulative-percentage calculators.

## Reachability

Six candidates surfaced. Three were reachable and skimmed; three hard-blocked the fetcher and
are recorded as unreachable rather than silently dropped.

| candidate | status |
| --- | --- |
| SimplicityHub — Pareto Chart Calculator | reachable, profiled |
| 5xWhys — Pareto Chart Maker | reachable, profiled |
| Quantopia — Pareto analysis method article (reference for the canonical table) | reachable, profiled |
| CalculatorLib — Pareto Chart Calculator | HTTP 403 to the fetcher |
| ImageOnline graphmaker — Pareto Chart Maker | HTTP 403 to the fetcher |
| LearnLeanSigma — Pareto 80/20 template | HTTP 403 to the fetcher |

## Profiles (paraphrased)

### 1. SimplicityHub — Pareto Chart Calculator

- **Input:** one `category,value` pair per line; entry order irrelevant, the tool ranks for you.
  Minimum two rows; the guidance suggests at least five categories for a meaningful read.
- **Options/defaults:** no user-facing controls found — sorting is forced high-to-low and the
  cutoff is fixed at the 80% Pareto boundary.
- **Output:** ranked descending bar chart with a cumulative-percentage overlay line and a
  highlighted 80% threshold line, plus headline metrics: vital-few count, total of all
  observations, top category's share, and how many categories are needed to cover 80%.
- **Tail handling:** advises AGAINST a catch-all bucket, on the grounds it can mask a real top
  cause — i.e. bucketing should be opt-in, not automatic.
- **UX:** a preloaded worked scenario (customer complaints across six categories) with a
  one-click "load this scenario" button; downloadable reference guide. No data export noted.

### 2. 5xWhys — Pareto Chart Maker

- **Input:** row-by-row manual entry with a category column and a count column; guidance
  recommends roughly six to twelve categories, and at least two rows with counts above zero.
- **Options/defaults:** optional chart title; automatic high-to-low sort; automatic 80% cutoff.
- **Output columns:** category, count, percent of total, cumulative percentage (drawn as the
  overlay line). Highlights the categories where the cumulative total first crosses 80% as the
  "vital few".
- **Export:** one-click PNG download; work-in-progress persisted in browser storage with a
  "continue where you left off" banner.
- **UX:** live preview while typing, edit-after-generate, start-over reset, worked example built
  around customer complaints.

### 3. Quantopia — Pareto analysis method reference

- **Canonical table columns:** cause/category · frequency (count or impact value) · cumulative
  frequency (running total) · cumulative percentage, computed as cumulative frequency ÷ grand
  total × 100.
- **Method:** sort most- to least-impactful BEFORE any cumulative column is computed; read the
  "elbow" where the cumulative line flattens; 80% is treated as a guideline, not a hard rule.
- **Worked example:** 1,000 inspected chairs — scratches 400 (40%), dents 250 (25%),
  misalignment 150 (15%), remainder grouped as "Others"; the top three make up 80%.
- **Tail handling:** minor factors aggregated into an "Other"/"Others" row, placed last in the
  descending table.
- **Rounding:** examples display whole-number cumulative percentages.

## Table stakes → decisions

| # | Table stake (≥1 competitor) | verdict |
| --- | --- | --- |
| 1 | `label,value` pairs, one per line, order-independent | **in-model** — `data`, multiline |
| 2 | Automatic descending sort before cumulating | **in-model** — `sort`, default `desc` |
| 3 | Columns: value, % of total, cumulative sum, cumulative % | **in-model** — plus `cum_n` running item count, per the tool's brief |
| 4 | 80% threshold + vital-few identification | **in-model** — `threshold` (default 80), `zone` column = vital/trivial |
| 5 | Headline metrics: total, category count, vital-few count, top share | **in-model** — summary block under the table |
| 6 | "Other"/tail bucketing (opt-in, per SimplicityHub's warning) | **in-model** — `top_n`, default `0` = off |
| 7 | Cumulative-percentage rounding shown as whole numbers in some tools, 1dp in others | **in-model** — `decimals`, default 1 |
| 8 | Chart with descending bars + cumulative line + 80% marker | **in-model, adapted** — deterministic fixed-width text Pareto chart (`chart`); a raster chart is not this tool's output shape |
| 9 | Sample/demo data loaded in one click | **in-model** — `[[example]]` preset chips |
| 10 | Minimum-rows validation with a clear message | **in-model** — actionable parse/validation errors |
| 11 | Spreadsheet paste (tab-separated, thousands separators, currency symbols) | **in-model** — `delimiter` auto-detect + value cleaning |
| 12 | Header row in pasted data | **in-model** — `header` = auto/yes/no |
| 13 | Reusable output for reports/spreadsheets | **in-model, beyond competitors** — `output` = table/csv/markdown |
| 14 | PNG chart download | **out-of-model** — this tool's surface is text output (chat/CLI/page share one renderer); a raster chart export would need a canvas renderer that the chat and CLI surfaces cannot consume |
| 15 | Save/restore work in progress via browser storage | **out-of-model** — no per-tool persistence layer; deep-linkable `?param=` URLs cover the share/restore case instead |
| 16 | Chart title field | **considered, rejected** — pure decoration on a text table; adds a schema param that changes no computation |

## Positioning notes

Every reachable competitor is a chart-first web page. This tool's differentiator is the
**machine-readable ranked table** — same Pareto math, but emitted as aligned text, CSV, or a
markdown table, reachable from chat, CLI, and a deep-linkable page, with a configurable
threshold rather than a hard-wired 80%.
