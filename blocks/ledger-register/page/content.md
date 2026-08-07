## About this tool

Ledger Register runs the posting-by-posting register report for a ledger-cli or
hledger plain-text journal. It is the checkbook view: each matching posting is
shown with its date, transaction description, account, amount, and a running
total in the final column.

The register is the detailed companion to a balance report. Use it when you want
to answer questions like “why did Checking end at $1,950?” or “which postings in
February affected Coffee expenses?” Everything runs locally in your browser via
WebAssembly; the journal you paste is not uploaded.

### What the parser understands

- Dated transactions with optional `*` cleared / `!` pending flags, `(code)` and
  a payee or description.
- Indented postings where the account and amount are separated by two or more
  spaces or a tab.
- One amount-less posting per transaction, inferred so the transaction balances.
- Multiple commodities (`$`, `USD`, `AAPL`, `"my fund"`) and both `1,234.56` and
  `1.234,56` number styles.
- `@` and `@@` price annotations, with an option to show postings at cost basis.
- Virtual postings in `(…)` or `[…]`, balance assertions (`= $100.00`), and the
  `account`, `alias`, `commodity`, `D`, `Y`, `P`, `apply account` and `comment`
  directives.

`include FILE` lines are skipped and reported in the notes because a browser
page and sandboxed command-line tool cannot read your local include tree.

### Worked example

Input:

```
2024-01-05 * Groceries
    Expenses:Food:Groceries   $45.20
    Assets:Bank:Checking

2024-01-10 Salary
    Assets:Bank:Checking      $2,000.00
    Income:Salary            $-2,000.00

2024-02-01 ! Coffee
    Expenses:Food:Coffee      $4.80
    Assets:Bank:Checking     $-4.80
```

With **Account filter** set to `checking`, output is:

```
2024-01-05 Groceries           Assets:Bank:Checking            $-45.20   $-45.20
2024-01-10 Salary              Assets:Bank:Checking           $2000.00  $1954.80
2024-02-01 Coffee              Assets:Bank:Checking             $-4.80  $1950.00
```

The last running total is the same number the balance tool reports for the same
account. Set **Running total mode** to *historical* when a date range starts in
the middle of the journal and you want the total to carry in the earlier balance.

### Useful options

- **Account filter** and **Payee filter** accept comma-separated, case-insensitive
  substrings. Prefix a term with `not:` or `-` to exclude it.
- **Start date** is inclusive; **End date** is exclusive, matching the CLI tools'
  `-b` / `-e` behavior.
- **Related accounts** shows the other side of transactions touching the filtered
  account, which is handy for bank-account spending reports.
- **Invert signs** flips income, liability or bank-account signs for friendlier
  reading.
- **Output format** switches between aligned text, CSV, JSON and Markdown.

### Limits

Up to 5,000 transactions per run. The account depth control accepts 0–10 levels,
row limit accepts 0–10,000, and text width accepts 40–400 columns. Periodic
summary registers, custom format strings, market-value conversion, expression
filters, and balance-assertion checking are out of scope for this posting-level
report.

## FAQ

<details>
<summary>How is this different from Ledger Balance?</summary>

Ledger Balance totals postings by account and shows one balance per account.
Ledger Register keeps the individual postings and adds a running total, so you
can trace how an account moved from one balance to the next. Use Balance for a
summary; use Register to inspect the transactions behind that summary.

</details>

<details>
<summary>Why is the end date exclusive?</summary>

Both ledger-cli and hledger treat the end date as the first date after the
report. To see January only, use start `2024-01-01` and end `2024-02-01`. That
also makes month and quarter ranges chain together without overlapping a day.

</details>

<details>
<summary>What does related accounts do?</summary>

When an account filter matches one side of a transaction, related mode prints the
other side. For a bank account, that turns a checkbook register into a category
register: Groceries, Rent, Salary and the other accounts that explain where the
money came from or went.

</details>

<details>
<summary>What does historical running total mean?</summary>

Period mode starts the running total at zero at the beginning of the report.
Historical mode first totals matching postings before the start date and carries
that balance into the first visible row, so a February-only bank register still
ends at the account's true current balance.

</details>

<details>
<summary>Are my books uploaded anywhere?</summary>

No. The parser and report run locally in your browser through WebAssembly. The
same core also powers the command-line tool, so the output is deterministic and
nothing has to leave your machine.

</details>
