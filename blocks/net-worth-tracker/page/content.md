## What this tool does

Your net worth is what you own minus what you owe: **total assets − total liabilities**. Paste a list of accounts, property, and debts, and this tool sorts each row onto the asset or liability side, groups the values by category, and returns a plain-text balance sheet — net worth, per-category totals with proportional bars, item counts, and your debt-to-asset ratio.

It measures wealth, not income: a high earner with large debts can have a low or negative net worth, and a modest earner with few debts can have a high one. Everything runs locally in WebAssembly — nothing is uploaded, and there is no account or sign-up.

## Input format

Use CSV or tab-separated rows. A header row is allowed and skipped automatically:

```text
Item, Amount, Type, Category
Checking, 8000, asset, Cash
Brokerage, 42000, asset, Investments
Mortgage, 240000, liability, Real Estate
Credit Card, 3500, liability, Credit Card
```

- **Item** — the account, property, or debt name.
- **Amount** — the value. Accepts plain numbers, currency amounts (`$1,234.50`), thousands separators (tab-separated rows work best for these), accounting negatives (`(500)`), and a quick `shares @ price` form (`10 @ 150` = 1500).
- **Type** — `asset` or `liability` (optional). If you leave it out, a **positive** amount is treated as an asset and a **negative** amount (`-3500` or `(3500)`) as a liability.
- **Category** — the grouping label (optional). Rows with no category fall into `Uncategorized`.

The type and category columns can appear in either order after the amount.

Common asset categories: **Cash, Investments, Retirement, Real Estate, Vehicles, Personal Property**. Common liability categories: **Mortgage, Auto Loan, Student Loan, Credit Card, Other Debt**. You are free to use any labels you like.

## Options

| Option | Choices | What it does |
| --- | --- | --- |
| **Sort categories** | `value` (default), `label` | Orders the category rows within each side largest-first or alphabetically. |
| **Currency prefix** | `$`, `€`, `£`, blank | Prefix shown before amounts in the output. |

## Example

With this input:

```text
Item, Amount, Type, Category
Home, 320000, asset, Real Estate
Brokerage, 80000, asset, Investments
Mortgage, 240000, liability, Real Estate
Credit Card, 4000, liability, Credit Card
```

the output is:

```text
Net worth: $156,000.00   (Assets $400,000.00 − Liabilities $244,000.00)

Assets — $400,000.00 total across 2 items
  Real Estate  $320,000.00   80.00%  ███████████████████·····  (1 item)
  Investments   $80,000.00   20.00%  █████···················  (1 item)

Liabilities — $244,000.00 total across 2 items
  Real Estate  $240,000.00   98.36%  ████████████████████████  (1 item)
  Credit Card    $4,000.00    1.64%  ························  (1 item)

Debt-to-asset ratio: 61.00%   (you own 39.00% of your assets)
```

## FAQ

<details>
<summary>How is net worth calculated?</summary>

Net worth is total assets minus total liabilities — everything you own minus everything you owe. This tool sums each side from your pasted rows and subtracts liabilities from assets. A negative result means your debts exceed your assets.

</details>

<details>
<summary>What counts as an asset versus a liability?</summary>

An asset is something you own that has value: cash, investments, retirement accounts, your home, vehicles, and personal property. A liability is something you owe: a mortgage, auto loan, student loan, or credit-card balance. Mark each row with `asset` or `liability`, or leave the type out and enter liabilities as negative amounts.

</details>

<details>
<summary>What is the debt-to-asset ratio?</summary>

It is total liabilities divided by total assets, shown as a percentage. Lower is stronger: a ratio of 60% means your debts equal 60% of your assets and you own the remaining 40% outright. It is a snapshot of financial health, not advice.

</details>

<details>
<summary>Is this financial advice?</summary>

No. It only calculates totals and percentages from numbers you provide. It does not fetch live prices, value your home, suggest targets, or recommend any action.

</details>

<details>
<summary>Is my data private?</summary>

Yes. The calculation runs entirely in your browser using WebAssembly. Your figures are never uploaded, and there is no account or sign-up.

</details>

<details>
<summary>What are the limits?</summary>

The tool accepts up to 5 MB of pasted text and 100,000 entries. It does not track net worth over time, project future values, convert currencies, or connect to financial accounts.

</details>
