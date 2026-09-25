# nps-csat-calculator — competitor analysis (2026-09-04)

Scan run **before** implementing, per `create-next-tool` step 4. All notes are paraphrased
observations of what each tool *does*; no competitor copy, wording, branding or trademarks are
reproduced or reused anywhere in this block.

## Tools reviewed

| # | Tool | Scope | Reachable |
|---|------|-------|-----------|
| 1 | SupportBee "NPS, CSAT & CES calculator" (supportbee.com/tools/nps-calculator) | all three metrics, tabbed | yes |
| 2 | miniwebtool NPS calculator (miniwebtool.com/nps-net-promoter-score-calculator/) | NPS only, deepest stats | yes |
| 3 | Formbricks CES calculator (formbricks.com/m/ces-calculator) | CES, 1–5 / 1–7 scales | yes |

Also skimmed for corroboration of formulas/benchmarks: MetricGate NPS docs (standard error + 95%
CI), Kalkulero NPS (margin of error), Standard Insights CSAT/CES, SurveyMonkey CSAT explainer.

## In/out model summary

Our gizza model is a single pure block: one text blob of ratings in, one deterministic text/JSON/CSV
report out, no accounts, no charts-as-images, no hosted survey collection.

### Table stakes observed → where each one lands

| Table stake | Seen in | Decision |
|---|---|---|
| NPS = %promoters − %detractors, on a 0–10 scale, reported as −100…+100 points | 1, 2 | **In** — `metric=nps` (the default) |
| Promoter (9–10) / passive (7–8) / detractor (0–6) counts **and** percentages | 1, 2 | **In** — always printed as a three-band breakdown table |
| CSAT = satisfied ÷ total × 100, with a configurable "satisfied" cut-off (top-2-box by default) | 1, 3 | **In** — `metric=csat` + `threshold` (`-1` = auto top-2-box) |
| CES = mean effort score, plus the % at/above the "easy" cut-off | 1, 3 | **In** — `metric=ces` reports both the mean and the easy-share |
| Multiple scales: 0–10 for NPS, 1–5 and 1–7 for CSAT/CES | 3 | **In** — `scale` enum `auto`/`0-10`/`1-5`/`1-7`/`1-10` |
| Aggregated **counts** entry (promoters/passives/detractors, or rating→count tallies) instead of a raw column | 1, 2, 3 | **In** — `input=counts`, one `rating,count` (or `rating: count`) row per line |
| Raw pasted column / comma-separated list of individual responses | 1, 3 | **In** — `input=values` (the default); newline, comma, semicolon, tab or space separated, CSV header row auto-skipped |
| 95% confidence interval / margin of error on the score | 2 (+ MetricGate, Kalkulero) | **In** — `confidence` enum `95` (default) / `90` / `99` / `none`; NPS uses the standard-error form for the promoter−detractor difference, CSAT a proportion interval, CES a mean (t-free normal) interval |
| Score health tier / verdict band for NPS | 2 | **In** — a rating line using the widely published −100…0 / 0–30 / 30–50 / 50–70 / 70+ bands, described in our own words |
| Rating distribution (how many gave each score) | 2 (stacked bar), 3 (breakdown) | **In** — a per-rating distribution table with counts, % and a plain-text bar |
| Decimal-precision control | 3 (implicit) | **In** — `decimals` 0–6, default 1 |
| Preset/example scenarios in one click | 2 ("quick examples") | **In** — four `[[example]]` chips on the page |
| Live recalculation as you type | 1, 2, 3 | **In** — the generator's page runtime already re-runs on input |
| Industry benchmark tables (SaaS, retail, telecom, …) | 1, 2, 3 | **Out of model** — benchmark tables are editorial data that goes stale and is not computable from the user's input; a static list would be unmaintained. The rating tier is computed instead, and the page copy says benchmarks vary by industry. |
| Animated gauge / stacked colour bar / charts | 2 | **Partly in** — the report includes a plain-text distribution bar; an animated SVG gauge is out of model for a text-output pure block |
| "Path to next tier" coaching (how many detractors to convert) | 2 | **In (small)** — for NPS the report states how many more promoter responses (at the current n) would reach the next tier |
| Response goal / sample-size planner | 2 (FAQ only) | **Out of model** — a separate sample-size tool, not this one's input→output shape |
| Saved history, team dashboards, survey hosting/collection | 1, 3 | **Out of model** — gizza blocks are stateless and offline |
| Segment/date filtering, trend over time | 1 | **Out of model** — needs a multi-column dataset with dates; that is `survey-tabulator`'s crosstab shape, not a single ratings column |

### Formula decisions (from the scan, cross-checked)

* **NPS** = (promoters − detractors) / n × 100, rounded to `decimals`.
* **NPS confidence band** = ±z · √((p + d − (p − d)²) / n) × 100 score points — the standard error of
  the promoter-minus-detractor difference, which is what the statistically-serious NPS calculators
  publish.
* **CSAT confidence band** = ±z · √(p(1 − p) / n) × 100 percentage points (Wald proportion interval),
  clamped to 0–100 %.
* **CES confidence band** = ±z · s / √n on the mean (s = sample standard deviation, n − 1
  denominator), clamped to the scale bounds.
* z: 1.645 (90 %), 1.960 (95 %), 2.576 (99 %).
* **Auto thresholds** (top-2-box): 9+ on 0–10, 4+ on 1–5, 5+ on 1–7, 9+ on 1–10 — matching the
  cut-offs the CSAT/CES calculators default to.

## UX patterns adopted

* Metric switch (their tabs) → a `metric` `<select>` with friendly labels, since one gizza page has
  one form.
* Preset scenarios → `[[example]]` chips (raw NPS column, aggregated counts, CSAT 1–5, CES 1–7).
* Paste-a-column as the primary field → `multiline = true` textarea with a realistic placeholder.
* Sliders for the bounded numeric fields (`threshold`, `decimals`).
* Output-format switch (`report` / `json` / `csv`) — beyond what any of the three offer, and it is
  what makes the result pipe-able from the CLI.

## Not copied

No competitor text, benchmark table, tier name, brand or trademark was copied. Tier wording, FAQ
answers, and page copy are written fresh here; only the public statistical formulas (which are
standard and unowned) are shared.
