## About the date difference calculator

This tool measures the exact span between two points in time. Enter a **start**
and an **end** date (or datetime) and it returns the duration two ways:

- **Calendar breakdown** — years, months, days, hours, minutes and seconds,
  with each unit being the remainder after the larger ones. This is the natural
  "2 years, 3 months and 5 days" form, and it accounts for the fact that months
  have different lengths and some years are leap years.
- **Flat totals** — the whole span expressed in a single unit: total weeks,
  total days, total hours, total minutes and total seconds.

Everything runs locally in your browser. Nothing is uploaded to a server, and it
works offline.

### Accepted formats

You can mix and match these on either field:

- `2024-01-31` (date only — treated as midnight)
- `2024-01-31T08:30:00` or `2024-01-31 08:30:00` (date and time)
- `2024-01-31T08:30:00Z` or `...+02:00` (RFC-3339 — the offset is dropped and
  the wall-clock value is used)
- `2024/01/31`, `01/31/2024` (US month/day/year), and `31.01.2024`

### How the breakdown is computed

Calendar units are counted by stepping the calendar forward, not by dividing a
number of seconds. That means going from **Jan 31** to **Mar 31** is exactly
**2 months** (not "2 months and a few days"), and a span crossing **Feb 29** in a
leap year counts that extra day correctly.

### FAQ

<details>
<summary>Does it handle time zones?</summary>

Dates are compared as wall-clock values. If you
paste a timestamp with an offset (`Z` or `+02:00`), the offset is dropped and the
local clock value is kept — so compare two timestamps in the same zone.

</details>

<details>
<summary>Why is the order-independent?</summary>

If you put the later date first, the magnitude
is still reported as a positive duration, and the result is flagged so you know
the end was before the start.

</details>

<details>
<summary>What's the difference between the breakdown and the totals?</summary>

The breakdown
mixes units (years + months + days …); the totals each express the entire span in
one unit. For example 8 days is "1 week and 1 day" in the breakdown, but "8" total
days and "1" total week in the totals.

</details>
