## About the age calculator

This tool works out your **exact age** from your date of birth. Enter your
**date of birth** and, optionally, an **as-of date** (it uses today if you leave
it blank). It then reports your age several ways:

- **Calendar breakdown** — your age as years, months and days, where each unit
  is the remainder after the larger ones. This is the natural "26 years, 5 months
  and 7 days" form, and it accounts for months having different lengths and for
  leap years.
- **Flat totals** — your whole age expressed in a single unit: total months,
  total weeks, total days and total hours lived.
- **The weekday you were born on** — e.g. Thursday.
- **Your next birthday** — the date and a countdown of days until it.
- **Your zodiac sign** — the Western (tropical) sun sign for your birth date.

Everything runs locally in your browser. Nothing is uploaded to a server, and it
works offline.

### Accepted formats

You can use any of these for either field:

- `2000-06-22` (date only)
- `2000-06-22T08:30:00` or `2000-06-22 08:30:00` (the time is ignored)
- `2000-06-22T08:30:00Z` or `...+02:00` (RFC-3339 — the offset is dropped)
- `2000/06/22`, `06/22/2000` (US month/day/year), and `22.06.2000`

### How age is computed

Age is counted by stepping the calendar forward, not by dividing a number of
seconds. So if you were born on the 31st, a month later (in a shorter month) is
still counted as a full month, and a span crossing **Feb 29** in a leap year is
handled correctly.

### FAQ

<details>
<summary>How is age in months and days calculated?</summary>
<p>First whole years are counted (you only gain a year once your birthday has passed), then whole months past that, then the leftover days — so the breakdown always sums back to your birthdate.</p>
</details>

<details>
<summary>What happens with a Feb 29 birthday?</summary>
<p>In years that are not leap years your birthday is treated as Feb 28 for counting purposes, and the next-birthday date falls back to Feb 28 too.</p>
</details>

<details>
<summary>Does it handle time zones?</summary>
<p>Dates are compared as wall-clock values. If you paste a timestamp with an offset (`Z` or `+02:00`), only the date part is used.</p>
</details>

<details>
<summary>Is my date of birth sent anywhere?</summary>
<p>No. The calculation happens entirely in your browser; nothing is uploaded.</p>
</details>
