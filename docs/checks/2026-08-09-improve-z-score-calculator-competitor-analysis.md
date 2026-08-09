# z-score-calculator — competitor analysis (2026-08-09)

Scan run before implementing, per `/improve-tool` Phase 2. All findings are **paraphrased
observations of capability**; no competitor copy, wording, or branding was reproduced or reused.

## Tools skimmed

| # | Tool | What it is |
|---|------|-----------|
| 1 | calculator.net — z-score calculator | Three-mode statistics calculator (raw→z, z↔probability, area between two bounds) plus a printed z-table |
| 2 | gigacalculator — z-score calculator | Two-mode calculator (raw→z with p-values, and inverse p→z critical value) with a precision selector |
| 3 | calculatorsoup — z-score calculator | Single-mode raw→z that accepts *many* data points at once, plus a sample-mean variant using the standard error |

Also noted in passing from the result set (not deep-read): several sites bundle a
z→p-value converter as a separate page, and one advertises percentile output alongside the tails.

## Table stakes observed

1. **Raw score → z given a known mean and standard deviation.** Universal; the formula
   `z = (x − μ) / σ` is stated on every page. This is the defining capability and the one
   no existing gizza block covers.
2. **Accept more than one raw score at a time**, comma/space/newline separated (competitor 3
   explicitly supports pasted columns and mixed delimiters).
3. **Probabilities from the z-score off the standard normal curve** — left tail P(X < x),
   right tail P(X > x), two-tailed p-value, and the percentile. Competitors 1 and 2 both make
   this the headline output rather than an extra.
4. **Inverse direction: z → raw score** (`x = μ + zσ`). Competitor 1 exposes it as a mode.
5. **Inverse normal: probability → critical z.** Competitor 2's second mode; competitor 1
   folds it into its z↔probability converter.
6. **Area between two bounds** — P(z₁ < Z < z₂). Competitor 1 ships it as its own mode.
7. **Sample-mean variant** — when testing a sample mean against a known population mean, the
   denominator is the standard error `σ/√n`, not `σ`. Competitor 3 documents both forms.
8. **A precision / decimal-places control.** Competitor 2 exposes it directly; the others
   fix it silently.
9. **Standardize a pasted dataset** (derive μ and σ from the data itself). Competitor 3
   supports it implicitly by taking multiple points.

## Decisions taken for this block

| Table stake | Decision |
|---|---|
| 1, 2 | `mode = "score"` (default): `values` takes one *or many* raw scores with the repo's usual space/comma/semicolon/newline splitting. |
| 3 | Every score row returns z, percentile, left tail, right tail and two-tailed p — always, not behind a toggle. Normal CDF via erf computed from the incomplete gamma series/continued fraction (≈1e-14), so results agree with R/SciPy well past the exposed precision. |
| 4 | `mode = "raw"` — treats `values` as z-scores and returns `x = μ + zσ`. |
| 5 | `mode = "critical"` — treats `values` as probabilities in (0, 1) and returns the z with that left-tail area (Acklam's inverse-normal seed plus one Halley refinement against our own CDF). |
| 6 | `mode = "between"` — takes exactly two bounds and returns the area between them. Because `mean` defaults to 0 and `std_dev` to 1, the same mode answers both "area between two z-scores" and "area between two raw values" without a second control. |
| 7 | `n` (sample size) param, default 1. When `n > 1` the denominator becomes the standard error `σ/√n` and the output reports it; at `n = 1` it is the plain z-score, so the default path is unchanged. |
| 8 | `decimals` param, default 6, range 0–12. |
| 9 | `mode = "dataset"` — derives μ and σ from the pasted numbers, with `sample` selecting ÷N−1. **Deliberately minimal**: this is the overlap with the existing `z-score-normalize` / `data-normalize` blocks, so this block adds only what those lack (percentile and tail probabilities per value) and does *not* re-implement min-max / max-abs / robust scaling or CSV column selection. Bulk feature scaling stays their job. |

### Deliberately not built (out of scope, not out of model)

- **A rendered z-table.** Competitor 1 prints a static lookup table. The calculator supersedes
  it, and a wall of static numbers is a content artifact rather than a computation.
- **Min-max / max-abs / robust scaling, CSV column selection.** Already shipped by
  `blocks/z-score-normalize` and `blocks/data-normalize`; duplicating them here would split one
  function across three pages.
- **Outlier flagging by z threshold.** Already shipped by `blocks/outlier-detector`.
- **Full descriptive summaries (median, quartiles, mode).** Already shipped by
  `blocks/descriptive-stats`; `mode = "dataset"` reports only the μ/σ it actually used.

### Duplicate check (recorded)

`blocks/z-score-normalize` covers "standardize a dataset" verbatim, and `blocks/data-normalize`
covers the CSV-column form. Neither accepts an explicit mean and standard deviation — a
`grep` for a `mean`/`std_dev`/`sigma` number param across all 1007 blocks returned nothing — so
the backlog row's primary clause ("z-scores for values *given* a mean and standard deviation"),
along with the normal-CDF probability outputs, the inverse directions and the standard-error
variant, is genuinely uncovered. Built rather than skiplisted, with the overlapping dataset mode
kept intentionally thin.
