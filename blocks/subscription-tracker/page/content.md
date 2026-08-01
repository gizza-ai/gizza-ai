## What this tool does

Paste the subscriptions you pay for and instantly see what they really cost. The
tracker normalizes every plan to a common **monthly** and **annual** figure — no
matter whether you're billed weekly, monthly, quarterly, or yearly — then adds up
your total spend, projects it over five years, breaks it down per day, shows each
plan's share of the total, and flags the single biggest annual spend as a cancel
candidate. Nothing is uploaded: it runs locally in your browser, works offline once
loaded, and needs no account or bank linking.

## How to enter your subscriptions

Type one subscription per line as **`Name: amount [cycle]`**:

```
Netflix: 15.99 monthly
Spotify: 10.99
Amazon Prime: 139 yearly
Adobe: 59.99 quarterly
```

- The `:` (or `=`) is optional — `Netflix 15.99 monthly` works too.
- The **cycle** keyword is optional; when a line omits it, the **Default billing
  cycle** setting is used (monthly unless you change it). Above, Spotify falls back
  to monthly.
- Amounts may include a currency symbol and grouping commas (`$1,299`).
- Blank lines and lines starting with `#` are ignored, so you can add comments.

## Billing cycles and how they annualize

| Cycle | Keyword(s) | Charges per year |
| --- | --- | --- |
| Daily | `daily`, `day`, `/day` | ×365 |
| Weekly | `weekly`, `wk`, `/wk` | ×52 |
| Biweekly | `biweekly`, `fortnightly`, `2wk` | ×26 |
| Monthly | `monthly`, `mo`, `/mo`, `pm` | ×12 |
| Quarterly | `quarterly`, `qtr`, `3mo` | ×4 |
| Semiannual | `semiannual`, `biannual`, `6mo` | ×2 |
| Yearly | `yearly`, `annual`, `/yr`, `pa` | ×1 |

The monthly equivalent shown for each plan is its annual cost ÷ 12, so a `$139/yr`
plan reads as `$11.58/mo` — directly comparable to a plan billed monthly.

## Worked example

Using the four subscriptions above (default cycle = monthly):

```
Subscription    Billed          Monthly   Annual  Share
Adobe           $59.99/qtr       $20.00  $239.96  34.1%
Netflix         $15.99/mo        $15.99  $191.88  27.3%
Amazon Prime    $139.00/yr       $11.58  $139.00  19.8%
Spotify         $10.99/mo        $10.99  $131.88  18.8%

Monthly total:  $58.56
Annual total:   $702.72
5-year total:   $3,513.60
Per day:        $1.93

Biggest spend: Adobe at $239.96/yr ($20.00/mo). Cancelling it saves $239.96/year.
```

## Reading the report

- **Monthly / Annual / 5-year / Per day totals** — your combined spend across every
  plan, on each timescale, so a "$15 here and there" habit shows its real size.
- **Share** — each plan's percentage of your total annual spend, so the money sinks
  are obvious at a glance.
- **Biggest spend** — the plan with the highest annual cost, called out with what
  you'd save each year by cancelling it. This callout is always the biggest annual
  spend regardless of how you sort the rows.
- **Sort** — order rows by `cost` (biggest first, the default), `name`
  (alphabetical), or `input` (your paste order).

## Limits

- Up to **200 subscriptions** per run; each amount up to `$1,000,000,000`.
- Amounts are handled in whole cents (max two decimal places).
- The **Currency symbol** is cosmetic — the tool does not convert between currencies,
  so enter every amount in the same currency.

## FAQ

<details>
<summary>Is my data private?</summary>

Yes. The calculation runs entirely in your browser using WebAssembly — your
subscription list never leaves your device, there's no account or bank linking, and
it keeps working offline once the page has loaded.

</details>

<details>
<summary>What if a subscription doesn't list a billing cycle?</summary>

Lines that omit a cycle use the **Default billing cycle** setting (monthly unless you
change it). You can also mix cycles freely — put the cycle keyword on each line that
differs from the default.

</details>

<details>
<summary>How is the monthly cost of a yearly plan calculated?</summary>

Every plan is annualized first (a `$139/yr` plan stays `$139.00`), then the monthly
equivalent is that annual figure ÷ 12 — so `$139/yr` shows as `$11.58/mo`. This makes
a yearly plan directly comparable to one billed monthly.

</details>

<details>
<summary>Can it convert between currencies?</summary>

No. The **Currency symbol** field only changes the symbol shown in the report; it does
not apply exchange rates. Enter every amount in the same currency so the totals add up
correctly.

</details>

<details>
<summary>Does it handle free trials, taxes, or discounts?</summary>

Enter the amount you're actually charged per cycle, after any discount and including
tax if you want it counted. There's no separate trial or promo field — the tool sums
exactly the amounts you type.

</details>
