# freelance-rate-calc — competitor analysis (2026-09-24)

Scan run **before** implementation, per `create-next-tool` step 4. All findings are paraphrased
observations of publicly visible calculator forms; **no competitor copy, branding or trademark is
reproduced or reused** anywhere in this block.

## Competitors reviewed

| # | Tool | What it is |
|---|------|------------|
| 1 | freelancerates.net | Take-home-income-driven rate calculator with a project-quote add-on |
| 2 | fastlancer.org hourly-rate-calculator | Three-step wizard: net goal → tax rate → overhead/time-off |
| 3 | rize.io/tools/rate-calculator | Reverse direction: given a rate, what is the effective take-home |

(A fourth, plutio.com/tools/rate-calculator, was skimmed and matches #2's parameter set; no
additional table-stakes surfaced.)

## Table-stakes parameters observed

| Capability | 1 | 2 | 3 | Fit | Where it lands here |
|---|---|---|---|---|---|
| Target take-home / net income per year | ✅ | ✅ | — | in-model | `target_income` (required) |
| Annual business expenses / overhead | ✅ | ✅ | — | in-model | `business_expenses` |
| Health insurance as a separate annual cost | ✅ | — | ✅ | in-model | `health_insurance` |
| Retirement contributions as a separate annual cost | ✅ | — | ✅ | in-model | `retirement` |
| Effective tax rate (income + self-employment) | — | ✅ | ✅ | in-model | `tax_rate` |
| Self-employment tax handled as its own 15.3% layer | — | — | ✅ | in-model | `tax_basis = "self_employment"` applies 15.3% on 92.35% of net profit on top of `tax_rate` |
| Vacation weeks | ✅ | ✅ | — | in-model | `vacation_weeks` |
| Public holidays (days) | ✅ | ✅ | — | in-model | `holidays` |
| Sick days | ✅ | — | — | in-model | `sick_days` |
| Billable percentage of worked time | ✅ | ✅ | — | in-model | `billable_percent` |
| Hours per week / days per week | ✅ | ✅ | ✅ | in-model | `hours_per_week`, `days_per_week` |
| Day-rate hour basis (fixed 8 h at #1) | ✅ | ✅ | — | in-model | `hours_per_day` — ours is configurable, theirs is hard-wired |
| Profit / slow-month buffer margin | — | ✅ (as advice) | — | in-model | `buffer_percent` |
| Currency symbol / picker | — | ✅ (USD/EUR/GBP) | — | in-model | `currency` — free-text symbol, so any currency works |
| Project quote from estimated hours | ✅ | — | — | in-model | `project_hours` (optional; 0 = off) |
| Complexity / rush multiplier on the quote | ✅ | — | — | in-model | `complexity` enum (simple/standard/complex/rush) |
| Milestone split on the quote | ✅ | — | — | in-model | emitted with the project quote (30/40/30) |
| Overhead quick-presets (low/medium/high chips) | — | ✅ | — | in-model | `[[example]]` preset chips on the page |
| Reverse check: what a given rate actually nets | — | — | ✅ | in-model | `current_rate` (optional) → gap vs. the required rate |
| Cost/derivation breakdown table | ✅ | ✅ | ✅ | in-model | markdown/text/csv/json breakdown sections |
| Working days, total hours, billable hours reported | ✅ | ✅ | — | in-model | reported in the time-budget section |

## Out-of-model — listed, deliberately NOT built

- **Profession/market-rate benchmarks** (#1's profession dropdown): needs a maintained salary
  dataset per role and region. gizza blocks are pure computation with no bundled market data.
- **W-2 / employee salary equivalence with real benefit loads** (#3): the same dataset problem —
  employer benefit ratios vary by country and employer and would be invented numbers.
- **Live FX conversion between currencies** (#2's USD/EUR/GBP switch converts labels only, but a
  real multi-currency tool needs rates): no network in a pure block. Ours takes a currency *symbol*
  so every currency renders correctly at its own scale.
- **Jurisdiction-specific tax brackets** (progressive federal/state/VAT tables): out of scope for a
  pure offline block; the tool takes an effective rate, and offers the US 15.3% SE-tax layer as an
  explicit opt-in because that one is a flat statutory rate, not a bracket table.
- **Email-gated results / account features / trial CTAs** (#3): not applicable.

## Gaps we close that the competitors leave open

- Every competitor hard-wires an 8-hour day; `hours_per_day` is a parameter here.
- Only #3 models self-employment tax correctly (on 92.35% of net profit); #2 folds it into one flat
  rate. This block supports both (`tax_basis`).
- None of the three expose a machine-readable export; this one offers CSV and JSON alongside
  markdown and plain text, and the whole thing runs offline in the browser or the CLI.
- Rate direction is one-way at every competitor (goal→rate at #1/#2, rate→take-home at #3). Adding
  `current_rate` gives both directions in one run.

## Decisions

- Tax is applied as `gross = (net_goal + costs) / (1 - tax_rate)` — the correct gross-up; a naive
  `net × (1 + rate)` under-charges. Documented in the FAQ.
- `billable_percent` is applied to worked hours *after* time off, which is how #1 and #2 both
  present it.
- The buffer margin is applied as a markup on the final rate, not on the income goal, so it reads
  as "charge this much more to absorb slow months".
