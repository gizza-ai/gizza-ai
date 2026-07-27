# bank-statement-reconcile — competitor analysis (2026-07-26)

Tool function: reconcile exported bank-statement CSV rows against bookkeeping ledger rows by date, amount, and fuzzy memo/description similarity.

## Competitors skimmed

1. Accounting reconciliation screens in small-business bookkeeping products: import bank feed, match transactions by amount/date, suggest possible ledger rows, and leave unmatched items for review.
2. Spreadsheet reconciliation recipes: use exact amount/date joins, helper columns, and fuzzy text matching or manual filters for descriptions.
3. Open-source/data-cleaning approaches: CSV joins plus string-similarity scores to propose matches, commonly with tolerances for posting-date drift and small rounding differences.

## Table-stakes decisions

| Capability | Decision | Notes |
|---|---|---|
| Two CSV inputs | in-model | pasted `statement_csv` and `ledger_csv` |
| Header names or indices | in-model | all six column fields accept header or 1-based index |
| Date tolerance window | in-model | `date_tolerance_days`, default 3 |
| Signed amount tolerance | in-model | `amount_tolerance`, default 0.01 |
| Fuzzy memo score | in-model | token Sørensen-Dice, 0-100 |
| Matched vs suggested buckets | in-model | suggested means amount/date pass but memo is below threshold |
| Unmatched items on both sides | in-model | reported separately |
| CSV/JSON/Markdown outputs | in-model | `output` enum |
| Multi-currency FX matching | out-of-model | needs exchange-rate lookup / domain-specific policy |
| Bank API import | out-of-model | this repo exposes local tools, not account integrations |
| Learning categorization rules | out-of-model | would require persistent model/training state |

## Defaults chosen

- `date_tolerance_days = 3` for weekend/processing-date drift.
- `amount_tolerance = 0.01` for cent-level rounding.
- `memo_threshold = 70` so weak but date/amount-compatible candidates are still surfaced as suggestions, not silently accepted.
- `output = markdown` for a readable review report; JSON and CSV support automation.

## UX patterns adopted

- Separate textareas for statement and ledger CSVs.
- Column-name placeholders default to `date`, `amount`, and `memo` on both sides.
- Sliders for date tolerance and memo threshold; enum selects for delimiter and output format.
- Example chips for a small reconciliation and strict JSON review.

Paraphrased only; no competitor copy or trademarks reproduced.
