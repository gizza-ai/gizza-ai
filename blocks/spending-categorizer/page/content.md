## About this tool

Export your transactions from your bank or credit-card site as CSV, paste them above, and get
an instant picture of where the money went. Every row is matched against a built-in table of
merchant keywords — Groceries, Dining & Takeaway, Transport, Fuel, Shopping, Subscriptions &
Streaming, Utilities & Phone, Housing & Rent, Insurance, Health & Fitness, Entertainment,
Travel, Cash & ATM, Transfers, Fees & Interest — and money coming in is recognized as Income.
Anything unmatched lands in `Other`, and you can add your own `keyword = Category` rules
(checked before the built-ins) to route it wherever you want.

The summary shows each category's total, its share of overall spending, the transaction count
and a proportional bar, followed by total spending, income and net cash flow. Switch the output
to *Categorized CSV only* to download the original rows with a `Category` column appended —
ready for a spreadsheet or budgeting app. Everything runs locally in your browser; your
statement never leaves your machine.

### Worked example

Pasting this statement:

```
Date,Description,Amount
2024-01-05,WALMART SUPERCENTER,-52.30
2024-01-06,STARBUCKS #1234,-4.50
2024-01-07,SHELL GAS STATION,-48.90
2024-01-08,NETFLIX.COM,-15.99
2024-01-09,WALMART SUPERCENTER,-34.80
2024-01-10,ACME PAYROLL,2000.00
2024-01-11,CITY PARKING,-12.00
```

produces this summary (plus the categorized rows):

```
Spending by category
====================

Category                       Total   Share  Txns
--------------------------------------------------
Groceries                     $87.10   51.7%     2  ████████████████████
Fuel                          $48.90   29.0%     1  ███████████
Subscriptions & Streaming     $15.99    9.5%     1  ████
Transport                     $12.00    7.1%     1  ███
Dining & Takeaway              $4.50    2.7%     1  █
--------------------------------------------------
Total spending               $168.49  100.0%     6
Income                      $2000.00             1
Net cash flow              +$1831.51
```

### Formats and limits

- The first row must be the column headers. Description/merchant, amount and date columns are
  auto-detected from common header names — including European ones like `Beschreibung`,
  `Omschrijving`, `Libellé`, `Betrag`, `Bedrag`, `Montant`, `Datum` — and you can name them
  explicitly when detection guesses wrong.
- Works with one signed amount column (money out negative) **or** separate debit/credit
  columns. If your bank exports spending as positive numbers, tick *Invert amounts*.
- Amounts may use US (`1,234.56`) or European (`1.234,56`) separators, parentheses negatives
  (`(42.00)`), or trailing `DR`/`CR` markers.
- Comma, semicolon, tab and pipe delimiters are supported (auto-sniffed by default).
- At most **10 000 rows** per run; larger files are rejected with a clear error.
- Categorization is deterministic keyword matching, not machine learning: single-word keywords
  match whole words by prefix (so `rent` matches RENT and RENTAL but not PARENT), and rules
  you write match anywhere in the description.

## FAQ

<details>
<summary>Which bank exports does this work with?</summary>

Any bank or card provider that exports CSV. The tool auto-detects the usual header names —
`Description`/`Payee`/`Details`/`Memo` for the merchant, `Amount` (or `Debit`/`Credit`) for
the money, `Date`/`Posted` for the date — so most exports work as-is. If your bank uses
unusual headers, type them into the column fields; if it uses semicolons (common in Europe),
the delimiter is sniffed automatically.

</details>

<details>
<summary>How do I fix a transaction that lands in the wrong category (or in Other)?</summary>

Add a rule line like `costco = Wholesale` or `my landlord llc = Housing & Rent` in the rules
box. Rules are case-insensitive substring matches, checked before the built-in keyword table,
so they always win. One rule per line; `#` starts a comment. The category name is free text —
you can invent your own categories.

</details>

<details>
<summary>Why are my expenses showing up as income?</summary>

The tool expects money out to be negative and money in positive, which is what most banks
export. Some card statements export spending as positive numbers instead — tick *Invert
amounts* and the signs are flipped before categorizing. Statements with separate debit and
credit columns are handled automatically (debits are money out).

</details>

<details>
<summary>Is my bank data uploaded anywhere?</summary>

No. The categorizer is WebAssembly running entirely in your browser — the CSV is parsed,
categorized and summarized locally, and nothing is sent to a server. You can disconnect from
the network after the page loads and it keeps working.

</details>

<details>
<summary>Can I get the result into Excel or Google Sheets?</summary>

Yes — set *Output* to *Categorized CSV only* and use the download link under the result. You
get your original rows with a `Category` column appended, quoted as standard CSV, which
imports cleanly into any spreadsheet or budgeting app. The date column is passed through
exactly as your bank wrote it.

</details>
