# subscription-tracker — competitor analysis (2026-07-29)

Scan run **before** implementation to fix table-stakes. Paraphrased only — no competitor
copy, branding, or trademarks reproduced.

## Competitors skimmed

- Miniwebtool — "Subscription Cost Tracker" (true annual cost)
- MySubscriptionCost.com — normalizes everything to a monthly equivalent
- Costlarity — monthly, annual & 5-year costs, potential annual-plan savings
- financecalccenter — monthly / yearly / weekly / daily cost breakdown
- usfinancecalculators — subscription audit / annual cost calculator

## Table-stakes (params / defaults / behaviours)

| Capability | Competitor norm | In our model? |
|---|---|---|
| Enter each subscription: name + cost + billing cycle | all | **yes** — one `Name: amount [cycle]` per line |
| Billing cycles: weekly / monthly / quarterly / yearly | all | **yes** — plus daily, biweekly, semiannual synonyms |
| Annualize: weekly×52, monthly×12, quarterly×4, yearly×1 | Miniwebtool exact | **yes** — same multipliers, integer-cent math |
| Normalize to a monthly equivalent (annual/12) | MySubscriptionCost | **yes** — per-line and total |
| Combined monthly total + annual total | all | **yes** |
| Cost per day | financecalccenter, Costlarity | **yes** — total daily cost line |
| 5-year projection | Costlarity | **yes** — annual × 5 total line |
| Each line's share (%) of total spend | several | **yes** — % column |
| Highlight/sort by biggest cost | several | **yes** — default sort by annual cost, cancel-candidate callout |
| Currency symbol | most | **yes** — `currency` param |
| Runs locally, nothing uploaded | most | **yes** — pure wasm |

## Worked example the tool must reproduce

Input:
```
Netflix: 15.99 monthly
Spotify: 10.99
Amazon Prime: 139 yearly
Adobe: 59.99 quarterly
```
(default cycle = monthly, so Spotify is monthly.) Expected: each line annualized
(Netflix 191.88, Spotify 131.88, Amazon Prime 139.00, Adobe 239.96), a monthly + annual +
5-year total, a per-day figure, and the biggest annual spend (Adobe) flagged to cancel.

## Marked in-model, built this pass

- `default_cycle` for lines that omit a cycle (weekly/biweekly/monthly/quarterly/semiannual/yearly)
- per-line monthly + annual + % of total, cancel-candidate callout with annual savings
- totals: monthly, annual, 5-year, per-day
- `sort` control (cost / name / input order)
- rich per-line cycle synonyms (mo, /yr, wk, pa, annually, fortnightly, …)

## Out-of-model / considered, not built

- **"Hours of work to pay for it"** — needs an hourly-wage input; considered, rejected as
  schema bloat for a spend-summary tool (a wage belongs in a paycheck tool).
- **Annual-plan-vs-monthly break-even / switch savings** — needs two prices per subscription;
  considered, rejected (doubles every line's input surface).
- **Saved lists / accounts / bank linking / reminders** — out of model (no server, no
  accounts, browser-local by design).
