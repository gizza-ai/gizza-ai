# salary-convert — competitor analysis (2026-07-30)

Snapshot from the create-next-tool build of `salary-convert`. Profiles are **paraphrased**
(features/ideas only — no competitor copy, branding, or trademarks reproduced).

## Competitors surveyed

1. **Omni Calculator — Wage Calculator** (`omnicalculator.com/finance/wage`) — single bidirectional
   input across hourly/daily/weekly/monthly/yearly; expandable hours-per-week + days-per-week;
   defaults 40 h/wk, 5 d/wk, 52 wk/yr; live update; outputs all five periods.
2. **TheCalculatorSite — Hourly to Salary** — hourly rate + hours/week + weeks/year; currency
   selector ($ € £ ₹ ¥); common-rate reference table; outputs weekly/monthly/annual.
3. **CalculatorSoup — Hourly to Salary** — adds optional overtime (1.5×) and double-time (2×)
   blocks and a with/without-overtime "calculated vs effective salary" (PTO) comparison.
4. **Talent.com — Salary Converter** — amount + pay-period dropdown (yearly…hourly) + editable
   hours/week; conversion equivalency table; outputs all six periods incl. biweekly; ties to a
   state tax calculator for net pay.
5. **Indeed Flex — Salary Converter** — "I know my…" period selector, hours-per-week presets
   (Part-time 10 / Full-time 40 / Overtime 60), quick rate chips, explicit `hourly × 40 × 52`
   formula, prominent "gross before tax" disclosure; 2,080-hour work year.

## Table-stakes (all good converters)

- Enter an amount plus the period it represents; convert in every direction.
- Editable **hours per week** and **weeks per year** (the variables that make it meaningful; 52 vs
  50 for PTO).
- Output at least hourly + weekly + monthly + annual; best-in-class add daily + biweekly.
- Live recalculation; clean currency display; state the assumptions and formula; "gross, pre-tax"
  disclaimer; worked examples + FAQ ("$X/hour is how much a year?").

## Gaps vs our build & disposition

| # | Gap | Disposition |
|---|-----|-------------|
| 1 | All six periods incl. **biweekly** (÷26) | **Built** — output has hourly/daily/weekly/biweekly/monthly/annual. |
| 2 | Editable hours/week, days/week, weeks/year with 40/5/52 defaults | **Built** — descriptor params with those defaults + min/max guards. |
| 3 | Preset chips (hours/week 10·37.5·40; weeks 50·52; common inputs) | **Built** — `[[example]]` chips on the page. |
| 4 | Inline assumption labels / formula (`hourly = annual ÷ (h/wk × wk/yr)`, `monthly = annual ÷ 12`) | **Built** — one-line summary states h/wk + wk/yr; page copy shows the formulas; monthly is annual÷12, not weekly×4. |
| 5 | Currency symbol (display only) | **Built** — `currency` param, affects summary text only, not the numbers. |
| 6 | "Gross / before tax" disclaimer | **Built** — stated on the page + FAQ. |
| 7 | Period aliases (hr/day/week/month/year, fortnightly) | **Built** — accepted in core. |
| 8 | Conversion equivalency table (1 hr/day/week/month/year side by side) | Considered — the JSON output already lists all six figures; a separate table widget is a generator-layer feature, not built this pass. |
| 9 | Overtime / double-time (1.5× / 2×) lines | **Considered, rejected** for this pass — expands well beyond the five clean inputs; belongs in a dedicated overtime-pay tool. Listed, not forced in. |
| 10 | Net-pay / tax localization (state tax) | **Out-of-model** — needs jurisdiction tax tables / server; this tool is gross-pay only. |
| 11 | Copy-to-clipboard per figure / permalink | Platform — the generator already ships a Copy-result button and query-param permalinks. |

## Default conventions adopted

- Hours/week **40**, days/week **5**, weeks/year **52** → 2,080-hour work year.
- **monthly = annual ÷ 12** (explicitly not weekly × 4); **biweekly = annual ÷ 26**;
  **weekly = annual ÷ 52**; **daily = annual ÷ (days/week × weeks/year)**;
  **hourly = annual ÷ (hours/week × weeks/year)**.
- All figures are **gross (pre-tax)**.
