## About this tool

Ledger Balance runs the **balance report** — the most-used command in plain-text
accounting — on a journal you paste in. It reads the plain-text format shared by
[ledger-cli](https://ledger-cli.org/) and [hledger](https://hledger.org/), sums
every posting into its account, and rolls those totals up the `:` hierarchy, so
`Expenses:Food:Coffee` also feeds `Expenses:Food` and `Expenses`.

Everything runs locally in your browser through WebAssembly. Your books never
leave the page — nothing is uploaded to a server — and the output is fully
deterministic: the same journal and settings always produce the same report.

### What the parser understands

- **Transactions and postings.** A dated header (with an optional `*` cleared or
  `!` pending flag, a `(code)`, and a payee) followed by postings indented under
  it. The account name is separated from its amount by **two or more spaces** or
  a tab — one space is part of the account name, exactly as in ledger.
- **One amount-less posting per transaction is inferred**, so the common
  `Assets:Bank:Checking` line with no number balances the entry for you.
- **Multiple commodities.** `$`, `€`, `USD`, `AAPL`, `"my fund"` — prefixed or
  suffixed, quoted or bare. Accounts holding several commodities print one line
  per commodity. Both `1,234.56` and `1.234,56` are read correctly.
- **Prices.** `10 AAPL @ $50.00` (unit price) and `4 VTI @@ $800.00` (total
  price) are parsed; tick *Convert @ / @@ priced postings to their cost* to
  report them in the price's commodity instead of the original one.
- **Virtual postings** in `(…)` or `[…]`, and **balance assertions** (`= $100.00`),
  which are parsed so they can never be mistaken for an amount, then ignored.
- **Directives**: `account`, `alias`, `commodity`, `D`, `Y`, `P`, `apply
  account` / `end apply account`, and `comment` / `end comment` blocks.

Amounts are summed as fixed-point integers, so thousands of postings add up with
no floating-point drift, and each commodity is printed back with the number of
decimals it was written with.

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

Output (tree layout, grand total on):

```
 $1950.00  Assets
 $1950.00    Bank
 $1950.00      Checking
   $50.00  Expenses
   $50.00    Food
    $4.80      Coffee
   $45.20      Groceries
$-2000.00  Income
$-2000.00    Salary
---------
    $0.00
```

The first transaction leaves `Assets:Bank:Checking` blank, so it is inferred as
`$-45.20`. Checking then nets `-45.20 + 2000.00 - 4.80 = $1950.00`, and the
grand total is `$0.00` because the journal is balanced — that zero is the
quickest check that nothing was mistyped.

Switch **Layout** to *flat* for full account names with no roll-up, set
**Depth** to `1` for a top-level summary, or pick a machine-readable **Output
format**: `csv` (`account,commodity,amount`), `json` (accounts, totals and the
transaction/posting counts), or `markdown` (a table to paste into notes).

### Limits

Up to 5,000 transactions per run — this is a paste tool, not a repository-scale
reporter. The depth fold accepts 0–10 levels. `include FILE` lines are skipped
(there is no filesystem in the browser) and reported in the notes under the
report. Market-value conversion (`-V` / `-X`), multi-period column reports
(`-M`, `-Y`), budget reports, and checking that balance assertions actually hold
are out of scope here. Beancount-dialect journals parse only where their posting
syntax overlaps; `open`/`close`/`balance` directives are not supported.

## FAQ

<details>
<summary>Does it read ledger-cli or hledger journals?</summary>

Both. The two tools share the same core journal syntax — dated transactions,
indented postings, `:`-separated account names, `*`/`!` status flags, an
inferred blank posting — and this parser targets that shared surface, so a file
written for either one works. The output matches the `ledger balance` /
`hledger balance` shape: amounts right-aligned in a column, sub-accounts
indented under their parent, and a dashed rule above the grand total.

</details>

<details>
<summary>Why is my grand total zero, and how do I turn it off?</summary>

In double-entry bookkeeping every transaction sums to zero, so totalling *all*
accounts also gives zero — a balanced-books check rather than a bug. Untick
**Show the grand-total row** to hide it, or narrow the report with the account
filter (for example `assets`) so the total covers only the accounts you care
about. The **% of top-level account** column is measured against each row's own
top-level account for the same reason, so the percentages stay meaningful on a
balanced journal.

</details>

<details>
<summary>How do the account filter and date range work?</summary>

The filter takes comma-separated, case-insensitive **substrings**: `expenses`
keeps every account containing that text. Prefix a pattern with `not:` or `-` to
exclude instead, and mix the two — `expenses, not:coffee` reports expenses
without the coffee sub-account. Dates are `YYYY-MM-DD`; **Start date** is
inclusive and **End date** is *exclusive*, matching `-b`/`-e` in both CLIs, so
January alone is `2024-01-01` to `2024-02-01`.

</details>

<details>
<summary>What does the cost-basis option do with @ and @@ prices?</summary>

A posting like `Assets:Broker:Stocks  10 AAPL @ $50.00` records ten shares
bought at fifty dollars each. By default the account is reported in its own
commodity (`10 AAPL`). With cost basis on, the posting is reported at what it
cost — `$500.00` — so share and cash accounts total in one currency. `@@` states
the *total* rather than the unit price, so `4 VTI @@ $800.00` costs `$800.00`.
Prices declared separately with `P` directives are parsed but not applied;
converting to a market value at a date is out of scope.

</details>

<details>
<summary>Are my books uploaded anywhere?</summary>

No. The parsing and totalling run entirely in your browser via WebAssembly — the
journal you paste is processed on your own machine and never sent to a server,
so nothing is stored or transmitted. That also makes the report deterministic
and available offline once the page has loaded.

</details>

<details>
<summary>Something failed to parse — what does the error mean?</summary>

Errors name the line number, so `line 12: '$12x.00' is not a number` points
straight at the typo. The most common causes are an account and its amount
separated by only **one** space (two or more are required, or a tab), more than
one posting in a transaction left without an amount (only one can be inferred),
and a `comment` block that was never closed with `end comment`. If the report
comes back empty, the account filter, date range or status filter excluded
everything — loosen one of them.

</details>
