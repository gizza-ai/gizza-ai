# monte-carlo-sim — competitor analysis (2026-07-25)

New pure tool: run a Monte Carlo simulation of a user-defined model — declare uncertain
input variables as probability distributions, combine them in a formula, sample N trials,
and report the outcome distribution (mean, std dev, min/max), percentiles, a probability of
meeting a target, and a text histogram. Runs entirely in the browser (deterministic given a
seed). All findings below are paraphrased — no competitor copy, branding, or trademarks were
reproduced.

## Competitor scan (top real tools skimmed)

1. **Numiqo — Monte Carlo Simulation Calculator** (numiqo.com). Browser-based. You define input
   distributions (uniform, normal, triangular, lognormal, …) and an output equation combining
   them, choose iteration count, and it returns percentiles, a histogram, and summary stats.
   Nothing to install; runs client-side.
2. **PMP Calculators / project-risk Monte Carlo** (pmpcalculators.com). Three-point
   (min/most-likely/max, i.e. triangular / PERT) task estimates summed across a project; outputs
   Pxx confidence levels ("P80 finish date", "P95 budget") and a distribution chart. Emphasises
   confidence percentiles as the practical deliverable.
3. **CountingMethods / Six-Sigma online Monte Carlo** (countingmethods.com). Fit or specify an
   input distribution per X, provide the transfer-function equation, and get percentiles, PPM
   defective vs spec limits, Ppk, a histogram, and sensitivity/tornado analysis — all in-browser.

(Analytica, Minitab Engage, XLSat, Vose ModelRisk are desktop/enterprise packages — reviewed for
feature vocabulary only, not as head-to-head browser tools.)

## Table-stakes params / features and where each landed

| Capability | Competitors | Our decision | Surface |
|---|---|---|---|
| Per-input probability distributions | all | **in-model**: `normal`, `uniform`, `triangular`, `lognormal`, `constant` | `variables` field grammar |
| Output equation / transfer function combining inputs | all | **in-model**: `model` expression (meval, same engine as `calculator`/`function-grapher`) | `model` field |
| Iteration / trial count | all | **in-model**: `trials` (default 10 000, 100–1 000 000) | `trials` field |
| Reproducible runs (seed) | Numiqo, most | **in-model**: `seed` (deterministic SplitMix64 PRNG — no `getrandom`, wasm-safe) | `seed` field |
| Summary stats (mean, std dev, min, max) | all | **in-model** | output |
| Percentiles (P5/P10/P25/P50/P75/P90/P95/P99) | all | **in-model** | output |
| Probability of meeting a target (P(X ≥ / ≤ t)) | PMP, CountingMethods (PPM vs spec) | **in-model**: `threshold` + `threshold_direction` enum | fields + output |
| Histogram of the outcome distribution | all | **in-model**: text histogram, `histogram_bins` (0 hides) | output |
| Preset examples (project cost, revenue-minus-cost) | Numiqo, PMP | **in-model**: `[[example]]` chips | page |

## UX controls / presets adopted

- `trials` / `seed` / `histogram_bins` render as number fields; `threshold_direction` is a
  `Param::enumv` → native `<select>` with friendly `[input.labels]`.
- `variables` and `model` are `multiline` textareas so pasted multi-line models keep newlines.
- `[[example]]` preset chips (revenue − cost, three-point project cost, portfolio return) double
  as the page's worked examples and match competitors that ship starter templates.
- Percentile-as-confidence framing (P80/P95) called out in the copy, since project-risk tools
  lead with it.

## Considered, not built (out-of-model or rejected)

- **PERT / beta distributions, correlated inputs (copulas), distribution fitting from data,
  tornado / sensitivity analysis, cloud batch runs, saved projects/accounts.** Out of gizza's
  browser-local, no-account, single-formula model or beyond a focused first release. PERT and
  sensitivity/tornado are the strongest future additions; triangular already covers three-point
  estimation. Listed here, not silently dropped.
- **Distribution charts as images.** The page renders a text histogram (pure-Rust, no chart
  runtime); a rendered chart image would need the media envelope and is deferred.

No competitor copy, layout, or branding was reproduced; all page copy and the distribution grammar
are original.
