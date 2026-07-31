## About this tool

Plain-text-accounting journals are readable in Git, but spreadsheet users often need one row per posting for filtering, pivot tables, reconciliation, or review. This converter flattens common Beancount and Ledger transactions into a fixed CSV schema, then rebuilds a simple journal from that same schema when you are done editing.

The CSV columns are:

```csv
date,flag,payee,narration,account,amount,currency,cost,price,comment
```

`Journal → CSV` repeats the transaction header fields on each posting row. `CSV → journal` groups consecutive rows with the same header fields into transactions; a blank `date` continues the previous transaction. The conversion is local and deterministic. It does not evaluate balances, fetch prices, load plugins, or read included files.

## Worked examples

Beancount input:

```beancount
2024-01-15 * "Starbucks" "Morning coffee"
  Expenses:Food:Coffee    4.50 USD
  Assets:Bank:Checking   -4.50 USD
```

CSV output:

```csv
date,flag,payee,narration,account,amount,currency,cost,price,comment
2024-01-15,*,Starbucks,Morning coffee,Expenses:Food:Coffee,4.50,USD,,,
2024-01-15,*,Starbucks,Morning coffee,Assets:Bank:Checking,-4.50,USD,,,
```

CSV input can also be rebuilt as a Ledger-style journal:

```csv
date,flag,payee,narration,account,amount,currency
2024-01-16,*,,Grocery Store,Expenses:Groceries,25.00,$
2024-01-16,*,,Grocery Store,Assets:Bank:Checking,-25.00,$
```

Ledger output:

```ledger
2024-01-16 * Grocery Store
    Expenses:Groceries  $25.00
    Assets:Bank:Checking  $-25.00
```

## Limits and edge cases

- This is a spreadsheet reshaper, not a full Beancount or Ledger interpreter.
- Non-transaction directives such as `open`, `close`, `balance`, `price`, `option`, `plugin`, and `include` are skipped.
- Elided posting amounts are left blank; inventory, balance assertions, plugins, and includes are not evaluated.
- Cost (`{...}`) and price (`@ ...`) annotations are carried through as text, not computed.
- Amount parsing assumes a dot decimal separator. Thousands commas are stripped from parsed amounts.
- The per-call cap is 20,000 postings/CSV rows.

## FAQ

<details>
<summary>Is this a complete Beancount parser?</summary>

No. It handles common dated transactions and indented postings for spreadsheet work. It does not run Beancount, evaluate inventory lots, assert balances, load plugins, or preserve every directive.

</details>

<details>
<summary>How are multiple postings represented in CSV?</summary>

Each posting becomes one CSV row. The transaction-level fields (`date`, `flag`, `payee`, and `narration`) repeat on each row so the table can be sorted or filtered without losing context.

</details>

<details>
<summary>Can I edit the CSV and convert it back?</summary>

Yes, if you keep at least `date` and `account` columns. `CSV → journal` groups consecutive rows with the same transaction header into one transaction. A blank `date` row continues the previous transaction.

</details>

<details>
<summary>What happens to cost, price, and comments?</summary>

Posting cost expressions such as `{100.00 USD}`, price expressions such as `@ 120.00 USD`, and trailing posting comments are copied into separate CSV columns and emitted again when converting back.

</details>
