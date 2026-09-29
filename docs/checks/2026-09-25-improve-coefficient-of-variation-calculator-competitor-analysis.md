# coefficient-of-variation-calculator — competitor scan (2026-09-25)

Scan run **before** implementing, so the descriptor could ship the table-stakes from the start.
All notes are paraphrased observations of publicly visible behaviour; no competitor copy,
branding, or trademark is reproduced here or in the tool.

## Competitors skimmed

1. **gigacalculator.com — Coefficient of Variation Calculator.** Two entry modes side by side:
   summary data (type a standard deviation and a mean directly) or raw data pasted from a
   spreadsheet. Radio buttons switch between a means-type and a proportions-type outcome, plus a
   sample-size box and an outcome-format selector (proportion / percent / event count). Output is
   the CV as a ratio and as a percentage. Documents that the CV blows up as the mean approaches
   zero. No worked numeric example on the page.
2. **statskingdom.com — Coefficient of variation calculator (with solution).** One paste-from-Excel
   textarea; a "more options" panel exposes a rounding-precision selector (1–9 digits), an
   include/exclude-outliers toggle, and an explicit delimiter choice (line break, comma, tab,
   space, custom). Reports BOTH the population CV (σ/x̄) and the sample CV (s/x̄) alongside mean,
   standard deviation and sum of squares, and adds a worked "solution" section plus a histogram.
   Explicitly states it ignores empty/non-numeric cells, that CV is misleading when the data mix
   positive and negative values around a near-zero mean, and that CV is meaningless on
   non-ratio scales (e.g. degrees Celsius). Worked comparison used on the page: a small-animal
   weight set (mean 4.97, sd 0.73 → CV 0.147) against a large-animal weight set (mean 797.42,
   sd 4.47 → CV 0.0056), i.e. the classic "same units, wildly different scale" argument for CV.
   Single dataset per run — the comparison is narrated in prose, not computed.
3. **inchcalculator.com — Coefficient of Variation Calculator.** Comma-separated number entry with
   a population/sample toggle. Shows CV as a decimal *and* as a percentage, plus the standard
   deviation, the mean, the sum of squares and N (or n). Renders a four-step derivation (mean →
   sum of squares → standard deviation with the right divisor → sd ÷ mean). Worked example:
   sd 0.783, mean 23.41 → CV 0.0334 = 3.34%. States that CV is undefined at a mean of 0, unstable
   near 0, and safest on all-positive data, and offers interpretation guidance (lower CV = more
   consistent; CV can exceed 1 when the sd exceeds the mean).

## Table stakes → where each one landed

| Table stake | Seen at | Decision |
| --- | --- | --- |
| Paste raw numbers, spreadsheet-friendly | all 3 | **Built** — `data`, multiline textarea |
| Sample (n−1) vs population (N) basis | 2, 3 | **Built** — `basis` enum, default `sample` |
| CV as a ratio **and** as a percentage | all 3 | **Built** — both printed for every dataset |
| Supporting stats: n, mean, sd, sum of squares, min/max | 2, 3 | **Built** — every dataset block lists them, so the derivation is checkable |
| Enter a known sd + mean instead of raw data | 1, 3 | **Built** — optional `mean` + `std_dev` params (summary mode) |
| Explicit delimiter choice | 2 | **Built** — `delimiter` enum with `auto` default (auto-detect is the better default; the override exists for labels containing the separator) |
| Rounding-precision control | 2 | **Built** — `decimals` (0–10, slider on the page), default 4 |
| Include / exclude outliers | 2 | **Built** — `exclude_outliers` boolean, Tukey 1.5×IQR, removed values reported |
| Ignore empty / non-numeric cells | 2 | **Built** — `ignore_non_numeric` boolean (default off, so a typo is an error by default; on for pasted spreadsheet junk) |
| Interpretation guidance (low vs high relative spread) | 3 | **Built** — a rule-of-thumb band per dataset plus the caveat that the bands are conventions, not tests |
| Step-by-step derivation display | 3, 2 | **Built as intermediates, not as prose steps** — n, mean, sum of squares, sd and the divisor used are all printed, which is every number in the four-step derivation. A narrated step list is not reproduced (it would be copy-shaped). |
| Machine-readable / copy-pasteable output | — (our own CLI/chat need) | **Built** — `output` enum: `summary`, markdown `table`, `json` |
| **Rank several datasets by relative dispersion** | none | **Built — this is the differentiator.** Every competitor does one dataset per run and narrates the comparison by hand; this tool takes one dataset per line, ranks them by \|CV\|, and reports the most consistent, the most variable, and the ratio between them. |

## Considered and deliberately not built

- **Proportions-type CV** (gigacalculator): CV of a proportion from `p` and `n`
  (`sqrt(p(1−p)/n) / p`). Feasible in pure Rust — this is *scope*, not capability. It is a
  different estimator (a sampling-distribution CV of a rate, not the dispersion of a dataset)
  with its own inputs, and folding it in would give one tool two unrelated meanings for "CV".
  It belongs with proportion/confidence-interval tooling, not here. Listed, not built.
- **Histogram / chart rendering** (statskingdom): out of model for this page — the generic tool
  page renders `format = "text"`, and chart rendering already has dedicated blocks
  (`blocks/pareto-chart`, `blocks/histogram-*`). Not built.
- **"Load last run" / saved sessions** (statskingdom): out of model — this repo's pages are
  stateless and store nothing. The equivalent capability here is the shareable `?param=`
  deep link plus the `[[example]]` preset chips, both of which exist.
- **Custom delimiter string** (statskingdom): the `delimiter` enum covers comma, tab, semicolon,
  space and pipe, which is every separator a spreadsheet or CSV export emits. An arbitrary
  user-supplied delimiter string was judged not worth a free-text param whose bad values fail at
  parse time. Named as a gap.

## UX control patterns adopted

- Rounding precision and decimals as a **slider** rather than a bare number box (matches the
  "more options" precision selector, better on touch).
- Friendly `[input.labels]` on every enum so `n-1` / `N` and the delimiter names read as words.
- Three `[[example]]` preset chips — the competitors' worked examples are the discovery path, so
  the page ships one-click presets: a two-group comparison, a single column of readings, and the
  known-sd/known-mean summary path.

## Limits stated on the page (from the competitors' own caveats plus ours)

CV is undefined at a mean of 0 and unstable near it; it is only meaningful on ratio scales with a
true zero; mixed positive/negative data makes it misleading; the sample basis needs n ≥ 2; the
interpretation bands are conventions, not significance tests. Input cap: 200,000 values across at
most 1,000 datasets.
