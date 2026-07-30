# rent-affordability — competitor analysis (2026-07-30)

Snapshot for the build+improve pass of `blocks/rent-affordability` (compute the maximum
affordable monthly rent from income using the 30% — or custom — rule). All competitor copy is
**paraphrased**; no branding, wording, or trademarks reproduced. Two of the three live tools
(Zillow, Apartments.com) returned HTTP 403 to automated fetch, so their details were reconstructed
from search snippets and public help docs and are marked high-confidence-but-not-fetch-verified;
calculator.net was directly reachable.

## Competitors profiled (top 3 real interactive tools)

Note: NerdWallet has **no** dedicated interactive rent calculator (only educational articles), so
it was replaced with calculator.net's Rent Calculator — a real, reachable interactive tool. gizza's
custom-ratio calculator therefore fills a genuine gap NerdWallet answers only with prose.

| # | Tool | What it does better | Dimension |
|---|------|---------------------|-----------|
| 1 | Zillow Rent Affordability Calculator | Factors recurring **debts, living expenses, and savings goals**, not just income; grosses up net income internally (~25% assumed tax); caps rent at ~40% of gross; pivots into live local listings. | capabilities / copy |
| 2 | Apartments.com Rent Affordability Calculator | **Adjustable rent-to-income slider** (10%–50%, default 30%) with live recompute; optional layered **50/30/20** budget view; instant first result from a single income field. | ux / capabilities |
| 3 | calculator.net Rent Calculator | **Front-end vs back-end DTI** approach with selectable ratios (25% / 33%) and a dedicated **monthly-debt** input; clear numeric worked examples. | capabilities |

## Table-stakes (all/most competitors ship these)

- Income input as the single primary field; instant result. **(in-model)**
- Custom / selectable rent-to-income ratio, default 30%, common range 10%–50%. **(in-model)**
- Factor in existing monthly debt payments (DTI / back-end ratio). **(in-model)**
- Gross vs. net (pre-tax vs take-home) income handling. **(in-model — flat gross-up)**
- A recommended range or multiple scenarios, not one bare number. **(in-model)**
- "Money left over" / leftover-budget framing. **(in-model)**
- Worked dollar examples on round salaries; explanation of the 30% rule + landlord screening. **(in-model, copy)**

## Defaults + worked examples observed

- Apartments.com: rent-to-income slider default **30%**, range **10%–50%**; worked example
  **$60,000/yr (~$5,000/mo) pre-tax → ~$1,500/mo rent** at 30%.
- Zillow: caps at **~40%** of gross; assumes **~25%** tax to convert net→gross; factors debts.
- calculator.net: selectable **25% / 33%** front-end ratios; takes a monthly-debt input.

## UX controls competitors use

- Slider for the rent-to-income percentage (Apartments.com) — mirror with a `kind = "slider"` on the ratio.
- Preset ratio choices 25% / 30% / 33% / 40% (calculator.net + Zillow ceiling) — mirror with `[[example]]` chips.
- Single-income-first, progressive depth (rent number first, budget detail second).
- Grouped inputs (income vs. debt vs. location).

## In-model decisions (built)

- **`income` + `income_period` (annual|monthly)** — accept either, like the competitors that take annual salary or monthly take-home.
- **`income_type` (gross|net) + `tax_rate_percent`** — flat gross-up of net income (Zillow's ~25% approach), default gross so the 30% rule applies to pre-tax income as landlords screen.
- **`rent_to_income_ratio`** (default 30, slider 10–50) — the headline 30%/custom rule.
- **`monthly_debts` + `max_dti_ratio`** (back-end DTI, default 36%) — debt-adjusted ceiling `gross_monthly × max_dti% − debts`; `recommended_max_rent = min(rule, debt_adjusted)`.
- **`guideline_range`** conservative/moderate/aggressive at 25/30/35% of gross monthly — the multi-scenario framing.
- **Leftover framing** — `remaining_after_rent_and_debts` output.
- **Preset chips** for the 25/30/33/40% ratios and a with-debts scenario; **slider** for the ratio.
- **`currency`** symbol in the plain-language summary (numeric fields stay plain for parsing).
- Copy: explanation of the 30% rule, gross vs net, landlord income-multiple, and stated limits.

## Out-of-model (considered, not built — need a backend/data/account)

- **Live local rental listings feed** (Zillow, Apartments.com) — needs a marketplace backend + inventory.
- **City cost-of-living adjustment** to the recommended rent — needs a per-market data set.
- **Real progressive-tax net→gross conversion** with jurisdiction tax tables — gizza is browser-local + currency-agnostic; we offer a flat assumed-rate gross-up instead and say so.
- **Saved profiles / account** — no login/server in the gizza model.
