## About this tool

CSV to Ledger converts a bank or credit-card **CSV export** into **double-entry**
journal transactions for [ledger-cli](https://ledger-cli.org/) and
[hledger](https://hledger.org/) — the plain-text accounting tools where your books
are just a text file you own. Paste a statement, and every row becomes a balanced
transaction: one posting to your bank/card account and a balancing posting to a
*guessed* category account.

Everything runs locally in your browser through WebAssembly. Your statement never
leaves the page — nothing is uploaded to a server — and the output is fully
deterministic: the same CSV and settings always produce the same journal.

### How it reads your CSV

- **Columns are auto-detected.** The tool finds the date, description/payee, and
  amount columns by common header names (Date, Posted, Description, Payee, Memo,
  Amount, Value …). If a header is unusual, name it explicitly in the matching field.
- **One signed column or two.** Statements that use a single signed `Amount`
  (negative = money out) work out of the box; statements that split money out and
  money in into separate `Debit` / `Credit` columns are handled too.
- **Dates and numbers are normalized.** ISO, US (`mm/dd/yyyy`), European
  (`dd.mm.yyyy`), and month-name dates all parse to ISO `YYYY-MM-DD`; both
  `1,234.56` and `1.234,56` amounts, parentheses `(20.00)` negatives, and trailing
  `DR`/`CR` markers are understood. Pick a **date format** if auto-detection is
  ambiguous, or a **delimiter** if the separator sniff guesses wrong.

### How accounts are guessed

The balancing (counter) account is chosen in three passes:

1. **Your own rules** — one `pattern = Account:Name` per line in *Account rules*.
   A case-insensitive substring match wins first, so `starbucks = Expenses:Coffee`
   overrides everything below it.
2. **A built-in keyword table** — common payees map to sensible categories
   (groceries, food, transport, utilities, subscriptions, rent, salary, refunds …).
3. **A sign-based fallback** — anything unmatched goes to your *default expense
   account* (money out) or *default income account* (money in).

Money out decreases your asset account and increases an expense; money in is the
mirror. If a statement exports spending as **positive** numbers, tick *Invert
amounts* so the signs land on the correct side.

### Worked example

Input (a two-row statement with a single signed amount column):

```
Date,Description,Amount
2024-01-15,Starbucks Coffee,-4.50
2024-01-16,ACME Payroll,2000.00
```

Output (ledger format, `$` currency):

```
2024-01-15 Starbucks Coffee
    Expenses:Food           $4.50
    Assets:Bank:Checking    $-4.50

2024-01-16 ACME Payroll
    Income:Salary           $-2000.00
    Assets:Bank:Checking    $2000.00
```

The coffee row is money out, so `Assets:Bank:Checking` is debited `$-4.50` and the
keyword table sends the balancing leg to `Expenses:Food`. The payroll row is money
in, so the asset gains `$2000.00` and `Income:Salary` carries the balancing
`$-2000.00`. Choose **hledger** output to omit the amount on the asset posting and
let hledger infer it.

### Limits

Up to 10,000 rows per conversion (paste larger exports in batches). Category
guessing is a heuristic starting point, not tax advice — review the counter-accounts
and refine them with your own rules. The tool categorizes and formats; it does not
reconcile balances, detect duplicates, or split a single row across multiple
categories.

## FAQ

<details>
<summary>What ledger format does it produce — ledger-cli or hledger?</summary>

Both — the syntax overlaps. In **ledger** mode each transaction prints an explicit
amount on *both* postings, so it is fully balanced and unambiguous. In **hledger**
mode the amount on the asset (bank) posting is omitted and left for the tool to
infer, which is more compact. Either output is valid input to both `ledger` and
`hledger`; pick whichever you prefer to read.

</details>

<details>
<summary>How are the category accounts decided, and can I fix wrong ones?</summary>

Each transaction's balancing account is guessed from the description. Your own
**Account rules** are checked first — write one `pattern = Account:Name` per line
(for example `whole foods = Expenses:Groceries`), and a case-insensitive substring
match overrides everything else. If no rule matches, a built-in keyword table maps
common payees to categories; if that misses too, the row falls back to your default
expense or income account by sign. Add rules to correct any guess — they always win.

</details>

<details>
<summary>My bank splits amounts into separate Debit and Credit columns — does that work?</summary>

Yes. When there is no single signed amount column, fill in the **Debit** (money out)
and **Credit** (money in) column names, or let the tool auto-detect headers like
`Withdrawal`/`Deposit` or `Paid out`/`Paid in`. Each row's net amount is money in
minus money out, so a debit becomes a negative (expense) posting and a credit a
positive (income) one, exactly as a single signed column would.

</details>

<details>
<summary>My dates or numbers come out wrong — how do I fix them?</summary>

Set the **Date format** explicitly: `mdy` for US `01/02/2024` (Jan 2), `dmy` for
European `01/02/2024` (Feb 1), `ymd` for ISO, or leave it on `auto`. The amount
parser reads both `1,234.56` and `1.234,56`, parentheses like `(20.00)` as
negatives, and trailing `DR`/`CR` markers. If the whole file mis-parses, the
**delimiter** sniff probably picked the wrong separator — choose comma, semicolon,
tab, or pipe by hand.

</details>

<details>
<summary>Is my bank data uploaded anywhere?</summary>

No. The conversion runs entirely in your browser via WebAssembly — the CSV you paste
is processed on your own machine and never sent to a server, so nothing is stored or
transmitted. That also makes the output deterministic and available offline once the
page has loaded.

</details>
