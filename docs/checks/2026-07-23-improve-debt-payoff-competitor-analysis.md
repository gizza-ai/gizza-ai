# debt-payoff — competitor analysis (2026-07-23)

Pre-build competitor scan for the new `debt-payoff` tool. All findings paraphrased;
no competitor copy, branding, or trademarks reproduced.

## Competitors scanned (top real tools)

1. **Financial Mentor — Debt Snowball Calculator** — up to 10 debts (creditor name,
   balance, interest rate, monthly payment) + a single "extra monthly amount" field.
   Offers snowball (smallest balance first) and avalanche (highest rate first) ordering
   plus "as entered". Uses the rollover method (freed-up payments cascade). Outputs:
   interest cost per debt, payments remaining per debt, current-vs-accelerated totals,
   time & interest savings, an optional payment schedule, printable summary.
   (ryanoconnellfinance.com returned HTTP 403 and was replaced by calculator.me.)
2. **Undebt.it** — unlimited debt accounts (balance, rate, minimum payment). Eight
   ordering strategies (snowball, avalanche, debt-to-interest hybrid, cash-flow index,
   highest payment, highest utilization, highest monthly interest, custom). One-off extra
   "snowflake" payments. Outputs: projected debt-free date, total interest, progress
   tracking, Excel export. Tracking/accounts are out-of-model (server + login).
3. **calculator.me — Accelerated Debt Payoff** — up to 10 debts (creditor, principal,
   rate, minimum payment) + extra monthly contribution. Snowball & avalanche ordering.
   Rollover method. Outputs: interest cost & payments-remaining per debt, side-by-side
   minimum-only vs accelerated totals, and the time & interest saved.

## Table-stakes params (each tagged in-model / out-of-model)

| Table-stake | Decision |
| ----------- | -------- |
| Multiple debts: name, balance, APR%, minimum payment | **in-model** → `debts` multiline field (one debt per line, comma-separated) |
| Method: snowball (smallest balance) / avalanche (highest rate) | **in-model** → `method` enum |
| Extra monthly payment (rolled into target debt) | **in-model** → `extra_payment` number |
| Rollover / cascade of freed minimums | **in-model** → core simulation |
| Per-debt payoff order + months + interest paid | **in-model** → structured output |
| Debt-free date | **in-model** → `start_date` (defaults to today) + month math |
| Snowball vs avalanche comparison | **in-model** → both simulated, interest/months diff reported |
| Savings vs minimum-only baseline | **in-model** → minimum-only sim + interest/months saved |
| One-off "snowflake" extra payments on specific months | **out-of-model** — needs a per-month schedule UI; recurring extra covers the common case |
| Progress tracking / logging payments over time | **out-of-model** — needs accounts + a server backend |
| Excel/PDF export, printable/emailable report | **out-of-model** — server/account; page offers copy-result instead |
| 6+ exotic orderings (CFI, utilization, hybrid) | **out-of-model (rejected)** — niche; snowball+avalanche are the named, understood pair |

## Design decisions

- Debts entered as a `debts` multiline textarea, one per line: `name, balance, APR%, min payment`.
  `$` and `%` symbols are stripped on parse; thousands separators are not allowed (comma is the field delimiter).
- `method` is `Param::enumv("snowball"|"avalanche")`, default `snowball`.
- `extra_payment` (number ≥ 0) is added to the constant monthly budget and always cascades to the current target debt.
- `start_date` (date, defaults to today in the browser) drives the debt-free date + per-debt payoff dates.
- Output (JSON) includes the chosen plan (months, debt-free date, total interest/paid, per-debt breakdown in payoff order)
  plus a comparison block (snowball vs avalanche totals, minimum-only baseline, interest & months saved, recommended method).
- Cap the simulation at 1200 months; if the budget can't out-run interest, return an actionable error.
