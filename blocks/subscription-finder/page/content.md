## What this tool does

Paste a list of your bank or card transactions and this tool finds the **recurring
charges** hiding in them — the subscriptions and memberships that quietly renew
every month or year. It groups repeat charges from the same merchant, works out how
often each one bills, estimates the next charge date, and projects what each is
costing you per year. Everything runs **locally in your browser** — nothing is
uploaded, there's no bank linking, and no sign-up.

Give it one transaction per line as `date, description, amount`:

```
2026-01-15, Netflix, 15.99
```

Most bank and card exports (CSV) already look like this, or can be pasted and lightly
tidied to match. A header row or any line that doesn't parse is simply ignored.

## Worked example

Paste this:

```
2026-01-01, Netflix, 15.99
2026-02-01, Netflix, 15.99
2026-03-01, Netflix, 15.99
2026-01-05, Spotify, 9.99
2026-02-05, Spotify, 9.99
2026-01-10, Corner Cafe, 4.50
```

and you get:

```
Found 2 recurring charges · $25.98/mo · $311.76/yr projected

1. Netflix — $15.99 monthly ×3 · next ~2026-03-31 · $191.88/yr
2. Spotify — $9.99 monthly ×2 · next ~2026-03-08 · $119.88/yr

Total: $25.98/mo · $311.76/yr
```

Netflix and Spotify each repeat, so they're flagged; the one-off `Corner Cafe` charge
is not. Charges are ranked by projected **annual** cost so the expensive habits sit at
the top.

## How it reads each charge

| Column | What it does |
| --- | --- |
| **Merchant** | The first spelling seen for the group. Repeat charges are matched on a normalized merchant name, so `NETFLIX #1234` and `Netflix` group together. |
| **Amount** | The typical (median) charge. Small variations group too — `$15.99` and `$16.20` from one merchant count as the same subscription. |
| **Cadence** | The median gap between charges → `weekly`, `biweekly`, `monthly`, `quarterly`, `semiannual`, `annual`, or `every ~N days` for anything irregular. |
| **×N** | How many matching charges were found. |
| **Next ~date** | An estimate: the last charge date plus the median interval. |
| **Per year** | The amount × charges per year — what it projects to annually. |

## Options

- **Minimum occurrences (2–24)** — how many times a charge must repeat before it's
  called recurring. The default is `2`. Raise it if you have lots of history and only
  want well-established subscriptions.
- **Currency symbol** — the symbol shown in front of amounts (`$`, `£`, `€`, …). It's
  cosmetic; the tool never converts between currencies.
- **Date format** — `auto` handles ISO (`2026-01-15`) and slash dates, guessing US vs
  EU from the day numbers. Force `iso`, `us` (MM/DD/YYYY) or `eu` (DD/MM/YYYY) if your
  dates are ambiguous (e.g. every day is ≤ 12).

## Limits and edge cases

- It reads only the columns you paste — **date, description, amount**. It can't open a
  PDF statement, link to your bank, or auto-sync; export or copy your transactions as
  text first.
- Detection needs at least the **minimum-occurrences** number of matching charges, so a
  brand-new or annual-only subscription with a single visible charge won't be flagged
  until it repeats.
- Amounts are read as magnitudes — a `-9.99` debit and a `(9.99)` are both a `9.99`
  charge. It doesn't tell debits from credits, so a repeated refund could show up.
- Cadence and the next-charge date are **estimates** from the spacing of past charges,
  not a guarantee of when a merchant will actually bill.
- It never cancels anything or contacts a merchant — it just shows you what's recurring
  so you can decide.

## FAQ

<details>
<summary>Is my statement data private?</summary>

Yes. The whole tool runs in your browser — your transactions are never uploaded to a
server, there's no bank linking, and nothing is stored. You can even run it offline once
the page has loaded.

</details>

<details>
<summary>What format should each line be in?</summary>

One transaction per line as `date, description, amount`, for example
`2026-01-15, Netflix, 15.99`. Only the **first** field is treated as the date and the
**last** as the amount, so a description containing commas still works. Amounts can
include a currency symbol or grouping commas (`$1,299.00`). Header rows and any line
that doesn't parse are skipped.

</details>

<details>
<summary>Why isn't one of my subscriptions showing up?</summary>

A charge only counts as recurring once it appears at least the **minimum-occurrences**
number of times (default `2`). If a subscription bills annually, or you only pasted one
month, there may be just a single charge to see — paste more history or lower the
minimum. Charges whose amounts differ a lot between months may also land in separate
groups.

</details>

<details>
<summary>How is the yearly cost worked out?</summary>

From the detected cadence: a monthly charge is multiplied by 12, weekly by 52, annual by
1, and so on. The amount used is the **median** of the matching charges, so an occasional
price blip doesn't skew the projection. The next-charge date is the last charge plus the
median gap between charges.

</details>

<details>
<summary>My dates are day/month — how do I read them correctly?</summary>

Set **Date format** to `eu` for `DD/MM/YYYY` (or `us` for `MM/DD/YYYY`). `auto` guesses
from the numbers — it can tell `13/01/2026` is day-first — but if every date has a day of
`12` or lower it's genuinely ambiguous, so pick the format explicitly.

</details>
