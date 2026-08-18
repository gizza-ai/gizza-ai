# churn-cohort-retention — competitor analysis (2026-08-18)

Scan run BEFORE implementation (new tool, so the "improve" pass folds into the build).
One web search (`cohort retention analysis calculator tool CSV churn rate table online`),
then the top 3 reachable, real competitor *tools* were read. Everything below is
**paraphrased** — no competitor copy, branding, or trademarks are reproduced or reused.

## Competitor profiles

### 1. Jawda Designs — Cohort Retention Calculator & Heatmap
`https://jawdadesigns.com/tools/cohort-retention-calculator-heatmap/`

- **features:** editable cohort grid, colour-coded retention heatmap, retention-curve chart
  with a per-cohort trace plus an average line, summary panel (M1/M3/M6/M12 averages,
  best/worst cohort, trend direction), health badge, plain-language insight sentence.
- **params/options:** cohort labels (seeded with three sample months), month columns M0–M11
  with M0 pinned at 100%, retention percentage per cell; "advanced" mode adds cohort customer
  count, per-cohort MRR, a date-range picker and a benchmark overlay. Max 6 cohorts.
- **defaults:** three sample cohorts of 100 customers, M0–M2 visible.
- **input formats:** manual grid entry of an ALREADY-AGGREGATED retention matrix — no raw
  event/signup data.
- **output formats:** heatmap table, curve chart, summary metrics, CSV copy of the table,
  shareable URL that encodes the inputs.
- **limits:** ≤6 cohorts, 12 periods; blank cells allowed for cohorts that have not aged that
  far yet.
- **ux patterns:** simple vs advanced mode, live recompute on edit, add/remove cohort rows,
  one-click CSV copy, stated local-only computation.
- **seo/copy angles:** cohort vs blended churn, how retention is computed, benchmark ranges by
  business model, M0/M1/M2 terminology, why cohorts beat a single blended number, handling
  cohorts with incomplete tenure, implied-churn derivation, privacy/export.
- **free vs paid:** free, no signup.

### 2. MetricGate — Retention Cohort Analysis (Extended)
`https://metricgate.com/docs/retention-cohort-analysis-extended/`

- **features:** retention matrix, text/symbol-density heatmap, period-over-period churn rates,
  average retention per period lag, per-cohort ranking with final retention.
- **params/options:** essentially none — paste the matrix and it computes.
- **input formats:** a pre-aggregated matrix (semicolon-separated rows of comma-separated
  active-user counts), positioned for people exporting cohort tables out of product-analytics
  suites.
- **output formats:** matrix + text heatmap + churn table + averages, shareable as text.
- **limits (stated honestly, and worth mirroring):** pre-aggregated input rules out
  user-level analysis; ragged matrices (unequal observation windows) make late-period
  comparisons misleading; assumes a consistent definition of "active" across cohorts;
  seasonality distorts cohort-to-cohort comparison.
- **ux patterns:** single paste box, instant result, plain-text output that pastes into chat.
- **free vs paid:** free docs calculator.

### 3. Product Growth — Cohort Analysis Calculator
`https://www.productgrowth.blog/tools/cohort-analysis`

- **features:** projects a retention curve from three numbers rather than measuring one.
- **params/options:** cohort size, month-1 retention, terminal monthly retention (default 95%).
- **defaults:** 1,000 users, 60% month-1 retention, 95% terminal retention.
- **input formats:** three numeric fields; no data import at all.
- **output formats:** month-by-month retention percentages and derived user counts through
  month 12, drawn as a two-stage curve (steep first drop, then slow decay).
- **limits:** monthly only, 12 months, a model — not a measurement.
- **seo/copy angles:** the M0=100% / M1=input / Mn = M(n-1) × terminal formula, a worked
  example (≈46% at month 6, ≈34% at month 12).
- **free vs paid:** free.

## Table stakes extracted (and where each landed)

| Table stake | Decision |
| --- | --- |
| Cohort × period retention matrix with cohort sizes | **in-model** — the core output (`format=table`/`csv`/`json`) |
| Percentages rounded for reading | **in-model** — 2 decimals everywhere |
| Churn view, not just retention | **in-model** — `metric=churn` (period-over-period loss) |
| Average retention per period lag | **in-model** — weighted `average` row across observable cohorts |
| Retention curve | **in-model** — text bar curve of the weighted average under the table |
| Absolute active-user counts, not only % | **in-model** — `values=percent\|count\|both` |
| Blank cells for cohorts too young to observe a period | **in-model** — `-` in table, empty in CSV, `null` in JSON, driven by `as_of` |
| CSV export of the table | **in-model** — `format=csv` (plus the page's Copy/Download) |
| Monthly cohorts | **in-model** — `granularity=month` (default) |
| Weekly / daily cohorts (product analytics norm) | **in-model** — `granularity=week\|day`; weeks start Monday |
| Configurable period count | **in-model** — `periods` (1–36, default 6) |
| Raw event data in (not a pre-aggregated matrix) | **in-model, our differentiator** — signup table + activity table, cohorting done here |
| Signup/cohort dates from a separate users table | **in-model** — optional `signups` input; falls back to each user's first activity |
| Explicit "as of" analysis date | **in-model** — `as_of`, defaults to the latest date in the data |
| Local-only / no upload | **in-model** — already true (wasm in the browser) |
| Stated limits about ragged matrices, "active" definition, seasonality | **in-model (copy)** — page Limits + FAQ |
| Shareable URL with the inputs | **already ours** — the page pre-fills from `?param=` deep links |
| Colour heatmap cells / chart canvas | **considered, rejected** — the page output is a text pane platform-wide; a colour heatmap would need a per-tool custom renderer. The bar curve gives the same shape read. Revisit only as a shared generator control. |
| Per-cohort MRR / revenue retention (NDR/GDR) | **considered, rejected for v1** — needs a revenue column and a second metric family; it would double the schema for a different question (revenue retention ≠ user retention). Listed as a follow-up. |
| Benchmark overlays by business model | **out-of-model** — an opinionated dataset we would have to source, maintain, and defend; not a computation. |
| Curve projection/modelling from assumed inputs (Product Growth's angle) | **out-of-model here** — that is a forecaster, not a measurement of pasted data; `time-series-forecaster` is the adjacent shipped tool. |
| Accounts, saved reports, warehouse/SQL connectors | **out-of-model** — needs a backend; gizza tools are browser-local with no account. |

## UX patterns adopted

- Example chips for the three real starting points (monthly retention from an events-only
  paste, signup-table + churn view, weekly cohorts) — competitors all ship seeded sample data.
- Placeholders show a real, runnable paste, so the empty page teaches the input shape.
- `<select>` (`Param::enumv`) for granularity/metric/values/delimiter/format; native date
  picker for `as_of`; checkbox for `header`.
- Limits and the "active" definition stated on the page, not discovered via an error.
- Errors name the expected form: ambiguous `03/04/2024` dates are rejected with an
  ISO-8601 instruction rather than silently guessed — determinism over convenience.
