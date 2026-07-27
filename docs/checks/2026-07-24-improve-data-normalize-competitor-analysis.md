# data-normalize — competitor analysis (2026-07-24)

Tool: **data-normalize** — scale the numeric columns of a pasted table (CSV/TSV) with
min-max, z-score, max-abs, or robust scaling, each column fitted independently, for ML
preprocessing and cross-feature comparison. This is the *multi-column table* sibling of the
existing single-list `z-score-normalize` block.

## Competitors scanned (paraphrased; no copy/branding reproduced)

1. **codingace — "Advanced Data Normalization Tool"** (`codingace.net/statistics`)
   - Input: paste numbers separated by commas, spaces, semicolons, or line breaks.
   - Methods: min-max, z-score standardization, mean normalization, decimal scaling,
     robust scaling, unit-vector scaling (6 total).
   - Min-max exposes **target minimum / target maximum** fields (custom output range).
   - Standardization exposes a **population vs sample** standard-deviation choice.
   - **Display decimals** precision control.
   - Export as CSV or PDF.
   - Detects zero-spread (identical values) and returns a safe fallback rather than erroring.
   - Categorical labels must be encoded first (numeric-only).

2. **everydaybudd — "Feature Scaling / Normalization Helper"** (`everydaybudd.com/tools`)
   - Input: **CSV upload with a header row**; processes 10–5,000 rows (configurable cap).
   - **Per-column** fitting — μ/σ for z-score, min/max for min-max computed separately per feature.
   - Handles non-numeric columns by letting the user **select which columns** to scale.
   - Methods exposed in UI: z-score, min-max. Robust (median/IQR) and MaxAbs discussed conceptually.
   - Output: side-by-side original vs scaled, parameter display, inverse-transform formulas.
   - No precision toggle, no population/sample toggle, "educational only".

3. **koshegio — "Data Normalization" calculator** (`koshegio.com/data-normalization`)
   - Z-score or min-max on a pasted list, instant results. (Single-list, like our existing
     `z-score-normalize`; page did not expose further controls.)

## Table-stakes → decision (every item lands in the descriptor or is listed here)

| Capability | Competitors | Decision |
|---|---|---|
| Multi-column table input (CSV/TSV paste) | everydaybudd, codingace | **in-model** — `data` param, delimiter auto-detected (`,`/`\t`/`;`) |
| Per-column independent fitting | everydaybudd | **in-model** — each scaled column fitted on its own values |
| Header row handling | everydaybudd | **in-model** — `header` bool (default true); header preserved, names usable |
| Non-numeric / passthrough columns | everydaybudd, codingace | **in-model** — non-selected/non-numeric columns copied through unchanged |
| Column selection | everydaybudd | **in-model** — `columns` param: names or 1-based indices; empty = all numeric |
| min-max scaling | all | **in-model** — `method=min-max` (default) |
| z-score standardization | all | **in-model** — `method=z-score` |
| robust (median/IQR) scaling | codingace, everydaybudd (concept) | **in-model** — `method=robust` |
| max-abs scaling | everydaybudd (concept) | **in-model** — `method=max-abs` (parity with `z-score-normalize`) |
| Custom output range for min-max | codingace | **in-model** — `range_min` / `range_max` (default 0/1) |
| Population vs sample std dev | codingace | **in-model** — `sample` bool (default false = population, matches sklearn) |
| Display precision | codingace | **in-model** — `precision` int (default 6, 0–15) |
| Zero-spread safe fallback | codingace | **in-model** — constant column → scaled to the range floor (0), sklearn-style, not an error |
| CSV export | codingace, everydaybudd | **in-model** — page output IS the normalized CSV; text pages get a Download link |
| Per-column parameter display (μ/σ, min/max, median/IQR) | everydaybudd | **in-model** — returned in chat/CLI JSON `scaled_columns` |
| Row cap for safety | everydaybudd (5,000) | **in-model** — 100,000 data-row cap, stated on the page |

## Listed, NOT built (out-of-scope / out-of-model — never built here)

- **Mean normalization, decimal scaling, unit-vector (L2) scaling** — only one competitor
  (codingace) exposes these extra three methods; feasible in pure Rust but out-of-scope for the
  focused min-max/z-score/max-abs/robust set the description calls for. Listed here, not built.
- **PDF export / distribution charts / side-by-side visual diff** — presentation features that
  need a rendering/plotting model; out-of-model for a pure text tool. Listed, not built.
- **Missing-value handling / imputation** — deliberately out of scope; scaled columns must be
  fully numeric. Covered by the sibling `missing-value-imputer` tool. Documented on the page.
- **Inverse transform** — the per-column parameters are returned so the user can invert manually;
  a dedicated inverse endpoint is not built.

## Relation to existing `z-score-normalize`
`z-score-normalize` normalizes a single 1-D list. `data-normalize` operates on a **2-D table**,
fitting each numeric column independently and preserving header + passthrough columns — the
standard ML-preprocessing shape (sklearn scalers on a DataFrame). Distinct enough to build.
