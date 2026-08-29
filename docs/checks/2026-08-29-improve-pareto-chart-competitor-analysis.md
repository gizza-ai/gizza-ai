# pareto-chart — competitor analysis (2026-08-29)

Scan run **before** implementation so the descriptor could be shaped by real table-stakes.
One `WebSearch` ("Pareto chart maker online free 80/20 cumulative percentage") → the four
reachable competitor tools below were skimmed with `WebFetch`. Two search hits
(cleanchart.app, calculatorlib.com) returned **HTTP 403** to the fetcher and were replaced
rather than counted.

**Everything here is paraphrased.** No competitor copy, branding, asset, or trademark is
reproduced or reused; all gizza page copy, defaults, and design are original.

---

## Competitor profiles (paraphrased)

### 1. MakeCharts — Pareto generator (`make-charts.com/pareto`)
- **Input:** free-text / natural-language description of categories and counts, parsed for you
  (e.g. "software bugs by type: UI 45, performance 30, crashes 20"). No spreadsheet needed.
- **Params/options:** automatic descending sort (not user-controllable); cumulative overlay;
  a *configurable* horizontal reference line at the 80% threshold; value and percentage
  labels; custom colours for bars, cumulative line, and reference line.
- **Limits stated:** recommends ≤ ~10 categories for legibility.
- **Output:** "export a polished chart"; concrete formats not documented on the page.
- **UX:** six preset example prompts as one-click starters; text-to-chart generation.
- **Copy/SEO angles:** use cases, common mistakes, best practices. No FAQ accordion.

### 2. 5xWhys — Pareto maker (`5xwhys.com/tools/pareto/`)
- **Input:** a small editable table — optional chart title, a category/defect-type text column,
  a numeric count column. Percent-of-total is computed per row as you type.
- **Params/options:** descending sort is automatic and non-optional; the 80% line is drawn
  automatically and is *not* configurable; cumulative % line on a secondary axis.
- **Limits stated:** ≥2 categories with counts > 0 required; "six to twelve rows" called the
  sweet spot.
- **Output:** one-click PNG. Excel handled by pointing at a separate tutorial.
- **UX:** a dedicated "vital few" panel that names the categories crossing the threshold;
  no theme/colour/label customisation exposed at all.
- **Privacy angle:** states data stays in browser localStorage, nothing sent to a server.
- **Copy/SEO angles:** three worked case studies (solder-joint defects, SaaS complaint
  reasons, ED wait-time drivers); FAQ covering the 80/20 rule, when to use a Pareto chart,
  Pareto vs plain bar chart, how many categories, non-numeric data, Excel export, privacy.

### 3. ChartLoad — Pareto chart (`chartload.com/charts/pareto-chart/`)
- **Input:** one categorical column plus one numeric value per category (count, cost, or
  frequency).
- **Params/options:** automatic largest→smallest sort; cumulative % line; an 80% horizontal
  reference line; bar colour plus a distinct **accent shade for the categories inside the 80%
  cumulative threshold**; font controls; editable titles for *both* Y axes (absolute left,
  cumulative-percent right).
- **Output:** PNG, PDF, SVG.
- **UX:** explicit dual-Y-axis presentation (left absolute, right 0–100%); vital-few vs
  trivial-many distinguished by colour.
- **Copy/SEO angles:** a numeric worked example narrated in prose (a top defect at ~47% of
  rejects, top two reaching ~77% of 300 units); FAQ on free/signup, how it's built, use
  cases, vs Excel, vs a bar chart.

### 4. AECharts — Pareto maker (`aecharts.com/pareto-chart-maker/`)
- **Input:** two columns (category, value); paste from Excel/Sheets/CSV or upload a file.
- **Params/options:** auto-sort; auto cumulative line; then a deep styling surface — bar
  colour, bar stroke colour, stroke width, bar width, **separate cumulative-line colour and
  width**, label/tick/axis/grid customisation, font size and colour.
- **Output:** PowerPoint, Google Slides, image.
- **Limits stated:** none.
- **UX:** live preview, spreadsheet-style data editor, style templates.
- **Copy/SEO angles:** definition, 80/20 explainer, when to use, import/export questions,
  how many categories, Pareto vs bar chart, "Pareto diagram" as an alias term.

---

## Table stakes → in-model / out-of-model

| # | Table stake (seen at ≥1 competitor) | Verdict | Where it landed |
| - | ----------------------------------- | ------- | --------------- |
| 1 | Paste `label,value` rows; tolerate CSV/TSV/semicolon/pipe/whitespace | **in-model** | `data`, multi-delimiter parser + `delimiter` |
| 2 | Header row handling | **in-model** | `header` (auto/yes/no) |
| 3 | Automatic descending sort | **in-model** (as the *default*, but made controllable) | `sort` = desc/asc/input |
| 4 | Cumulative-percentage line on a secondary 0–100% axis | **in-model** | dual axis, `show_cumulative` |
| 5 | Configurable 80% reference line | **in-model** | `threshold` (0 = hide), `threshold_color` |
| 6 | "Vital few" highlighting in a distinct colour | **in-model** | `highlight_vital_few`, `vital_color` |
| 7 | Data/value labels on bars and on the cumulative points | **in-model** | `show_values`, `show_cumulative_labels` |
| 8 | Bar colour, line colour, threshold colour, background | **in-model** | `color`, `line_color`, `threshold_color`, `background` |
| 9 | Bar width control | **in-model** | `bar_width` (0–1 fraction of slot) |
| 10 | Cumulative line width + point markers | **in-model** | `line_width`, `point_radius` |
| 11 | Chart title + both Y-axis titles + X-axis title | **in-model** | `title`, `value_label`, `percent_label`, `category_label` |
| 12 | Font size / label legibility, rotated category labels | **in-model** | `font_size`, `label_angle` |
| 13 | Canvas size | **in-model** | `width`, `height` |
| 14 | Legend | **in-model** | `legend` |
| 15 | Grid lines | **in-model** | `grid` |
| 16 | "≤10 categories reads best" guidance / long-tail problem | **in-model**, improved: instead of only advising, bucket the tail | `max_categories` → automatic `Other` bar |
| 17 | Vital-few *narrative* ("top two reach 77%") | **in-model** | `output=summary` table + `output=json` |
| 18 | Percent-of-total per row | **in-model** | summary/json columns |
| 19 | Light/dark presentation styles | **in-model** | `theme` |
| 20 | Decimal precision on the percentages | **in-model** | `decimals` |
| 21 | SVG export | **in-model** | `output=svg` is the native return value |
| 22 | PNG / PDF / PPTX / Google-Slides export | **out-of-model** | no rasteriser or Office writer in a pure-Rust local block; SVG is deterministic and converts downstream |
| 23 | Natural-language "describe your data" chart generation | **out-of-model** | needs a hosted LLM; this block is local + deterministic |
| 24 | Spreadsheet-style live data editor / file upload / Sheets import | **out-of-model** for the block; the page's textarea + example chips cover the paste path |
| 25 | localStorage persistence of the last dataset | **out-of-model** (page is stateless by design; deep-link query params are the shareable-state answer) |
| 26 | Style "templates" / theme gallery | **considered, rejected** — `theme` + explicit colour params cover it without schema bloat |

## Gaps we close that no scanned competitor ships

- **`sort=asc|input`** — every scanned tool hard-codes descending. Keeping input order is what
  you need to sanity-check an already-ranked table, and ascending is useful for
  smallest-first triage reviews.
- **Automatic `Other` tail bucketing** at `max_categories` — competitors only *advise* keeping
  the category count low.
- **Non-SVG machine output** — `summary` (aligned text with per-row percent, cumulative
  percent, and vital-few marks) and `json` (bar geometry, cumulative points, threshold
  crossing index) so the tool is usable from the CLI and from an LLM, not just as a picture.
- **Threshold reporting** — the crossing category and the exact cumulative percentage at the
  crossing are returned as data, not just drawn as a line.
