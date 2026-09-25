# t-test-calculator — competitor analysis (2026-09-24)

Scan run **before** implementing `blocks/t-test-calculator`, so the descriptor, the page
controls and the page copy could be designed against what real t-test calculators ship.
Everything below is **paraphrased** from public product surfaces; no competitor copy,
branding, trademarks or assets were reused.

## Competitors reviewed

| # | Tool | What it is |
|---|------|------------|
| 1 | Statistics Kingdom — t-test calculator | The most feature-dense free t-test page: four test variants, three tail choices, raw or summary input, power, normality, outliers, step-by-step working. |
| 2 | GraphPad QuickCalcs — t test | The best-known "trusted lab tool" version: unpaired / Welch / paired, four input modes (typed rows, pasted rows, mean+SEM+n, mean+SD+n), two-tailed only, terse result block. |
| 3 | 8gwifi — t-test calculator | One-sample / two-sample / paired / Welch, alpha from a fixed set, three alternatives, reports t, df, p, critical value, CI and Cohen's d with a small/medium/large reading, plus a distribution plot and a Python snippet. |
| 4 | Social Science Statistics — independent / paired / single-sample t-test | Three separate single-purpose pages, raw-data boxes, significance-level picker, one- vs two-tailed, plain-language verdict. Optimised for students. |
| 5 | Statistics Fundamentals / r-statistics.co style calculators | "Instant result" pages: t, df, exact p, 95% CI, Cohen's d, one/two-tailed switch, adjustable alpha, and a plain-English verdict with the working shown. |

## Table-stakes checklist

| Capability | Competitors that ship it | Our decision |
|---|---|---|
| One-sample t-test | all | **in-model — built** (`test=one-sample`) |
| Two-sample pooled (Student) t-test | all | **in-model — built** (`test=two-sample`) |
| Welch unequal-variance t-test | 1, 2, 3, 5 | **in-model — built** (`test=welch`, and the `auto` default for two samples, matching the modern recommendation) |
| Paired / dependent t-test | all | **in-model — built** (`test=paired`, from two columns or from a single column of pre-computed differences) |
| One-tailed (left / right) alternatives | 1, 3, 4, 5 | **in-model — built** (`tails=two\|left\|right`) |
| Adjustable significance level | 1, 3, 4, 5 | **in-model — built** (`alpha`, 0.0001–0.5, slider) |
| Hypothesised mean / expected difference ≠ 0 | 1 | **in-model — built** (`mu`) |
| Raw-data paste with delimiter choice | 1, 2, 4 | **in-model — built** (`delimiter` = auto/comma/tab/semicolon/pipe/space, plus header auto-detection) |
| Summary-statistics input (mean, sd, n) | 1, 2 | **in-model — built** (`format=summary`, `name,n,mean,sd` rows) |
| Summary input as mean + **SEM** + n | 2 | **considered, rejected** — a second summary layout doubles the ambiguity of auto-detection; the page and the `data` description show the one-line conversion `sd = sem × sqrt(n)` instead |
| Rounding / decimal-place control | 1 | **in-model — built** (`decimals`, 0–10) |
| t statistic, df, exact p-value | all | **in-model — built** (fractional Welch–Satterthwaite df included) |
| Critical t at alpha | 1, 3 | **in-model — built** |
| Confidence interval for the difference | all | **in-model — built**, and one-tailed runs report the matching one-sided bound rather than a two-sided interval |
| Cohen's d (+ small/medium/large reading) | 1, 3, 5 | **in-model — built**, with the standardiser stated (pooled sd, average sd for Welch, sd of the differences for paired) |
| Hedges' g (bias-corrected d) | 1 | **in-model — built** |
| Effect-size confidence interval | 1 | **in-model — built**, as the large-sample normal approximation, labelled approximate on the page |
| Statistical power | 1 | **in-model — built** (observed/post-hoc power from the noncentral t distribution, with an explicit caveat that observed power is a restatement of p) |
| Equal-variance check before choosing pooled vs Welch | 1 | **in-model — built** (variance-ratio F test + a recommendation note) |
| Normality test (Shapiro–Wilk) | 1 | **considered, not built** — the AS R94 polynomial approximation cannot be verified offline against reference values here, and shipping an unverified p-value in a significance tool is worse than omitting it. The variance-ratio check, the descriptives and an explicit small-sample note cover the practical need; the page says so plainly. |
| Automatic outlier exclusion | 1 | **considered, rejected** — silently dropping observations changes the hypothesis being tested. Out-of-range values stay in; the page documents how to remove them deliberately. |
| Step-by-step working | 1, 5 | **partially built** — the readable summary shows every intermediate (n, mean, sd, sem, standard error, df, critical t), which is the useful part of "show your working" without a second rendering mode |
| Distribution chart | 1, 3 | **out-of-model for this page** — the generic tool-page renderer outputs text/media, not plots |
| Python / R snippet export | 3 | **considered, rejected** — `output=json` already gives a machine-readable result any script can consume, without pretending to generate code we can't test |
| Excel / CSV file upload | 1 | **out-of-model** — pure tools take pasted text; the paste path already accepts spreadsheet copy (tab-delimited, header row auto-detected) |
| Accounts / saved analyses / paid tiers | — | **out-of-model** — browser-local, no backend |

## UX / controls observed and what we did

- Competitors lead with a **test-type selector**; ours is the second field (`test`) with an
  `auto` default that picks one-sample for one column and Welch for two, and says so in a note.
- Competitors expose alpha as a small fixed list (0.01 / 0.05 / 0.10). We use a **slider** over a
  continuous range so 0.001-style thresholds stay expressible, with 0.05 as the default.
- Competitors ship worked demo data. We ship **three one-click example chips** (paired
  before/after, two-sample wide columns, published means as summary rows) — the declarative
  `[[example]]` mechanism.
- Competitors bury the standardiser behind "Cohen's d". We **name the standardiser** on every
  effect-size line, because d computed from pooled sd, average sd and sd-of-differences are three
  different numbers and the mismatch is a common reporting error.
- Every fixed choice is a `Param::enumv`, so the page renders real `<select>` controls with
  friendly labels rather than free-text boxes.
- Output mode (`summary` / `table` / `json`) mirrors the sibling ANOVA tool so the two stats tools
  behave the same way.

## Non-goals recorded

Three-or-more-group comparisons (the page points at one-way ANOVA), z-tests, nonparametric
alternatives (Mann–Whitney, Wilcoxon), repeated-measures / mixed models, and multi-factor designs
are all outside this tool's model and are stated as such on the page rather than approximated.
