# covariance-matrix-builder — competitor analysis (2026-09-24)

Scan run BEFORE implementing, per `.claude/skills/create-next-tool/SKILL.md` step 3 /
`/improve-tool` Phase 2. Everything below is a paraphrased summary of publicly visible
behaviour — no competitor copy, branding or trademark is reproduced or reused.

## Scanned competitors

| # | Tool | What it does |
|---|------|--------------|
| 1 | Statology "Covariance Matrix Calculator" (statology.org) | Up to **five** variables, each pasted as a comma-separated list into its own box; one "Calculate" button; output is a Var1..Var5 labelled covariance matrix table. No sample/population switch, no decimals control, no correlation output, no FAQ. |
| 2 | scientificcalculatoronline.io "Covariance Matrix Calculator" | Three entry modes (manual grid, CSV upload, spreadsheet paste as CSV/TSV), optional header row, sample (n−1) vs population (n) toggle, **weighted covariance** mode, outputs covariance matrix **and** correlation matrix plus a colour heatmap tab, exports CSV and JSON. No worked example, no FAQ. |
| 3 | mlforbeginners.com "Covariance Matrix Calculator" | Grid entry or CSV paste with auto-detected comma/tab/space separators, sample (N−1) vs population (N) toggle (Bessel's correction explained), random-data button, outputs the symmetric matrix plus per-variable means, mentions eigenvalues; FAQ covers spreadsheet import, why the matrix is symmetric, what negative entries mean, covariance vs correlation. |
| 4 (pairwise reference) | gigacalculator.com "Covariance Calculator" | Two variables only (X and Y columns), accepts space/tab/comma separation and spreadsheet paste, reports sample **and** population covariance side by side plus mean of X, mean of Y and the sample count. |

Result: the category is uniformly "paste numbers → get a symmetric matrix". The
differentiators between them are (a) how flexible the paste parser is, (b) whether the
sample/population denominator is exposed, and (c) whether the standardized (correlation)
variant is offered next to the raw covariance.

## Table stakes → where each one landed

| Capability | Seen at | Decision |
|---|---|---|
| Paste a whole data block, one row per observation, one column per variable | 1,2,3,4 | **In** — `data`, multiline textarea |
| Comma / tab / space separated paste (spreadsheet copy-paste) | 2,3,4 | **In** — `delimiter = auto` splits on comma, tab, semicolon, pipe or runs of whitespace; explicit `comma/tab/semicolon/space/pipe` available when a value could be ambiguous |
| Optional header row of variable names | 2,3 | **In** — `header = auto` (a first row that is not all-numeric is a header), forceable with `yes`/`no`; `labels` overrides either way |
| Sample (n−1) vs population (n) denominator | 2,3,4 | **In** — `denominator` enum, default `sample` (Bessel's correction, matches `numpy.cov` / Excel `COVARIANCE.S`) |
| Correlation matrix alongside the covariance matrix | 2 | **In** — `matrix = correlation` (the standardized covariance) |
| Centered / standardized data variants | (the brief; implied by 2's "standardized relationship measure") | **In** — `matrix = centered` (x − mean) and `matrix = standardized` (z-scores); the covariance of the standardized data *is* the correlation matrix, and the page says so |
| Per-variable means (and variances / SDs) | 3,4 | **In** — `stats = true` appends an n / mean / variance / std-dev summary; the variances are the matrix diagonal |
| More than 5 variables | 2,3 (1 caps at 5) | **In** — up to 100 variables × 20 000 observations, stated on the page |
| Weighted covariance | 2 | **In** — optional `weights` (one non-negative weight per row). Frequency-weight convention: denominator Σw − 1 (sample) or Σw (population), matching `numpy.cov(fweights=…)`. Documented in the FAQ |
| Decimal-places control | (none exposed; all round silently) | **In** — `decimals` 0–12, default 6, rendered as a slider. A genuine gap in all four |
| Export CSV / JSON | 2 | **In** — `format = csv` / `json`; plus `markdown` for pasting into docs, and the page's built-in Download link |
| Preset / random example data | 3 (random button) | **In** — four `[[example]]` chips (body measurements, two-asset returns, correlation variant, CSV export) |
| Worked example + real FAQ on the page | 3 only | **In** — one fully worked 2-variable example with the arithmetic shown, 6 `<details>` FAQs |

## Out of model / deliberately not built

- **Interactive heatmap tab** (2) — a rendered visual, and gizza already ships
  `blocks/correlation-heatmap` (correlation matrix → SVG heatmap). Linked in the page copy
  instead of duplicated.
- **Eigenvalues / PCA of the covariance matrix** (3 mentions it) — already shipped as
  `blocks/principal-component-analysis` (eigenvalues, explained variance, loadings, scores,
  with a `scale=false` covariance mode). Cross-referenced, not re-implemented.
- **Manual spreadsheet grid entry with ± row/column steppers** (1,2,3) — a stateful grid
  widget; the generic tool-page generator renders declarative fields, not editable grids.
  Paste (including direct spreadsheet paste, which is TSV) covers the same job.
- **CSV *file upload*** (2) — pure blocks take text params, not file inputs; paste and the
  CLI (`data="$(cat file.csv)"`) cover it. Noted on the page.
- **Pairwise-only "covariance between X and Y" mode** (4) — a 2-column input already
  produces exactly that as the off-diagonal entry of a 2×2 matrix; no separate mode needed.

## Verification notes

Everything in the "In" column is exercised by the committed tests: unit tests in
`blocks/covariance-matrix-builder/core/src/lib.rs`, the schema drift guard in
`blocks/covariance-matrix-builder/src/lib.rs`, and the advertised-values matrix in
`tests/tool-page-covariance-matrix-builder.spec.ts` (one real run per `matrix`,
`denominator`, `header`, `delimiter` and `format` choice, the non-default `stats`
checkbox state, the `decimals` cap boundary at 12/13, a `?param=` deep link, and the
page's generated CLI example).
