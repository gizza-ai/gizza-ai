## About this tool

Bank Statement Reconcile compares a bank-statement CSV with a bookkeeping ledger
CSV. It matches transactions by **signed amount** and **date window**, then uses a
fuzzy token score on the memo/description fields to separate confident matches
from suggested matches that need review.

Use it when exported bank transactions and accounting rows use slightly different
memos such as `COFFEE SHOP` versus `Coffee Shop downtown`, or when ledger entries
land a few days away from the bank posting date.

### Worked example

Statement CSV:

```csv
date,amount,memo
2024-01-02,-42.50,COFFEE SHOP
2024-01-05,-1200.00,RENT PAYMENT
```

Ledger CSV:

```csv
date,amount,memo
2024-01-02,-42.50,Coffee Shop downtown
2024-01-05,-1200.00,January rent
```

With a three-day date tolerance and a moderate memo threshold, the tool lists the
coffee row as a match and the rent row as a lower-scoring suggested match for
human review.

## Limits & edge cases

- Each CSV is capped at **1000 data rows** to keep the O(statement × ledger)
  matching pass bounded.
- Dates can be `YYYY-MM-DD`, `YYYY/MM/DD`, or common slash/dot/dash dates with an
  optional time part ignored.
- Amounts may include currency symbols, thousands separators, signs, or
  accounting parentheses like `(1,234.56)`.
- Matching is one-to-one and greedy in statement row order: each ledger row can be
  claimed once.
- Memo similarity is token-based. It is useful for descriptions, not a guarantee
  that two transactions are semantically identical.

## FAQ

<details>
<summary>Do amounts need to be signed the same way on both files?</summary>

Yes. The matcher compares signed amounts. If the statement exports debits as
negative values but your ledger stores expenses as positive values, normalize one
file first or use a CSV calculation tool before reconciling.

</details>

<details>
<summary>What is the difference between matched and suggested matches?</summary>

Both pass the amount and date tolerance gates. A matched row also reaches the
memo similarity threshold. A suggested match has the right amount and date but a
weaker memo score, so it is listed separately for manual review.

</details>

<details>
<summary>Can I use column numbers instead of header names?</summary>

Yes. Every date, amount, and memo column field accepts a header name or a 1-based
index such as `1`, `2`, or `3`. Header names are usually clearer for saved URLs
and CLI commands.

</details>

<details>
<summary>Why did one obvious match remain unmatched?</summary>

A ledger row can only be claimed once. If duplicate amounts and dates exist, the
greedy pass picks the best memo score in statement order. Tighten the date window,
review the suggested matches, or split duplicate-heavy periods into smaller
batches.

</details>
