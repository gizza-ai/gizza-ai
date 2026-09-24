# amortization-schedule — competitor analysis (2026-09-24)

Scan run BEFORE implementation, per `/create-next-tool` step 4. Everything below is a
**paraphrased** feature/parameter observation — no competitor copy, branding, or trademarks are
reproduced, and no competitor markup/assets were used.

## Competitors reviewed

| # | Tool (paraphrased role) | What it is |
|---|---|---|
| 1 | A large general-purpose calculator portal's amortization page | Loan amount + term (years **and** months) + rate + start month; extra monthly / yearly / one-time payments; monthly **and** annual schedule views; balance/interest chart; print |
| 2 | A finance-calculator site's amortization page | Multi-currency symbol picker; term as years + months; recurring extra payments at weekly / monthly / quarterly / half-yearly / yearly frequency; one-off lump sum; table columns date · payment · principal · interest · balance; print/share |
| 3 | A mortgage-portal "additional payments" calculator | Extra payments of several kinds (one-time, weekly, biweekly, monthly, quarterly, yearly) with date ranges; headline **interest saved** and **time saved** vs. the base loan; optional detailed schedule toggle |
| 4 | A bank's amortization calculator | Month-by-month split of each payment into principal vs. interest; "with vs. without extra payments" side-by-side; cumulative-interest column |
| 5 | Spreadsheet/CSV-oriented schedule builders | Payment frequency beyond monthly (weekly, biweekly, semi-monthly, quarterly, semi-annual, annual); balloon payment; **CSV / spreadsheet export** of the whole table |

## Table stakes extracted

| Capability | In gizza's model? | Decision |
|---|---|---|
| Loan amount, annual rate %, term in years **+ extra months** | yes | built — `loan_amount`, `annual_interest_rate_percent`, `loan_years`, `loan_months` |
| Level payment derived from the standard amortizing formula, 0% handled | yes | built — `payment_per_period`, zero-rate falls back to principal ÷ periods |
| Per-period rows: number, date, payment, principal, interest, balance | yes | built — plus an `Extra` column and a cumulative-interest total |
| Payment frequency beyond monthly | yes | built — weekly / biweekly / monthly / quarterly / semiannual / annual |
| Payment dates from a start date | yes | built — `start_date` (blank ⇒ date column omitted, period numbers only) |
| Recurring extra payment toward principal | yes | built — `extra_payment` |
| One-time lump sum at a chosen period | yes | built — `extra_one_time` + `extra_one_time_period` |
| Interest saved + periods saved vs. the no-extra baseline | yes | built — reported in the totals of every output format |
| Annual (year-by-year) summary view as well as the full table | yes | built — `schedule_view = period \| annual` |
| CSV export of the schedule | yes | built — `format = csv` (the page's Download button saves it); `format = json` for programmatic use |
| Currency symbol on money figures | yes | built — `currency_symbol`, default `$` |
| Defaults that show a result before you type | yes | built — 300,000 at 6% over 30 years, matching the backlog row's example query |
| Preset one-click scenarios | yes | built — four `[[example]]` chips (30-yr mortgage, 15-yr, +$200/mo, 5-yr auto loan) |
| Worked example, stated limits, ≥3 FAQ accordions | yes | built — in `page/content.md` |
| Balance/interest chart | no (would need a chart runtime on the page) | **considered, not built** — the annual view plus the totals cover the same question in text |
| Print / save / share / email buttons | no | **considered, not built** — out of model; the page already has Copy + Download |
| Multiple extra-payment streams with independent date ranges | partly | **considered, rejected** — one recurring stream + one lump sum covers the common case; N streams would need a repeating-group control the generator has no declarative kind for |
| Semi-monthly (24/yr) frequency | partly | **considered, rejected** — the two dates in a month are not an equal period, so both the per-period rate and the date column become ambiguous; the six supported frequencies all have a well-defined period length |
| Balloon payment | yes, but | **considered, rejected** for v1 — a balloon changes the payment-solving model (payment sized to a target remaining balance); the lump-sum parameter covers the "pay a chunk" case |
| Taxes / insurance / HOA / PMI escrow rows | yes, but | **considered, rejected** — `mortgage-calculator` already owns escrow; this tool stays a pure loan-amortization table |
| Accounts, saved scenarios, server-side PDF | no | **out of model** — gizza tools are browser-local, no account, no server |

## Not a duplicate of an existing block

- `blocks/mortgage-calculator` returns scalar totals (monthly PITI, payoff months, total interest) and
  never emits a row-per-period schedule; the backlog row for this tool calls that gap out explicitly.
- `blocks/debt-payoff` plans a multi-debt snowball/avalanche rollover, not a single loan's table.
- `blocks/compound-interest-calculator` grows a balance; it does not amortize a loan.

## Verification notes

Every capability above is exercised by the unit tests, the CLI checks, or the Playwright spec
(`tests/tool-page-amortization-schedule.spec.ts`), including one run per enum value of
`payment_frequency`, `schedule_view`, and `format`, plus the period cap boundary.
