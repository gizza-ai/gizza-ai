# roas-calc competitor analysis (2026-09-24)

## Search

Query: `ROAS calculator break even ROAS ACOS CAC LTV ad spend calculator`

Reviewed public calculator patterns from the top accessible search results:

1. SellerCalcs — ad ROAS and break-even calculator for sellers.
2. Joshua Sanderford — ROAS and break-even calculator including CPL, CAC and LTV concepts.
3. PlanyPals — ROAS calculator covering return on ad spend, ACOS, break-even ROAS and profit after advertising.

The snippets/results consistently describe calculators centered on ad spend, attributed revenue, margin, break-even ROAS, ACOS, CAC/LTV, and seller/ecommerce use cases.

## Table-stakes features and decisions

| Feature / UX pattern | Seen in competitors | Decision for roas-calc |
| --- | --- | --- |
| Ad spend and attributed revenue inputs | Core ROAS inputs everywhere | In model: required `ad_spend`, optional `revenue`; can also forecast revenue from AOV, CPC and conversion rate. |
| Gross margin percentage | Used to compute true break-even rather than revenue-only ROAS | In model: `gross_margin` with slider control and 0–100 validation. |
| Break-even ROAS | Common headline output for sellers/marketers | In model: `break_even_roas`, break-even ad spend and break-even revenue. |
| ACOS | Common for Amazon/ecommerce style calculators | In model: `acos` and cost-per-revenue output. |
| Profit after ads / net profit | Common gap beyond simple ROAS | In model: gross profit, profit after ad spend, fixed-cost-adjusted net profit and net margin. |
| CAC / customer economics | Present in calculators that include leads/orders | In model: conversions/orders drive CAC, max CAC, CAC headroom, LTV, LTV:CAC and payback. |
| LTV-adjusted ROAS | Present in LTV-oriented calculators | In model: `purchases_per_customer` and `purchase_interval_months` drive lifetime margin ROAS and payback. |
| Per-order ecommerce costs | Seller calculators need shipping, payment and return assumptions | In model: `margin_basis = per_order` builds contribution margin from COGS, shipping, payment rate/fixed fee, refund rate and other variable cost. |
| Revenue-goal budget planning | Useful adjacent planning calculation | In model: `revenue_goal` reports budget needed at current, break-even and target ROAS. |
| Preset/example chips | Competitor tools expose common scenarios | In model/page: example chips for quick ROAS, per-order ecommerce break-even, target net margin, forecast mode and LTV/CAC. |
| Sliders for percentages | Common UX for percent inputs | In model/page: sliders for margin, rates, target margin, conversion rate, lifetime purchases, interval and decimals. |
| Live platform imports or ad-account connections | Some marketing tools imply live platform workflows | Out of model: gizza blocks run locally with explicit user inputs; no ad-platform auth or network data import. |
| Advice on "good" ROAS benchmarks | Often included in copy | Out of model as automated advice; included only as neutral FAQ explaining that break-even depends on margin and inputs. |

## Descriptor/page requirements captured

- Every fixed-choice parameter is an enum: `margin_basis` and `format`.
- All parameters have descriptions in the descriptor.
- Page controls use sliders for percentages/ratios and labels for enum choices.
- FAQ and content explain limits: same reporting period, attribution assumptions, refund allowance, LTV margin basis, currency display only and no financial advice.
- Output supports markdown, text, CSV and JSON so CLI/page examples can be exact and machine-checkable.
