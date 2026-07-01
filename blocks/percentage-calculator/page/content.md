## About the percentage calculator

This free percentage calculator answers the five everyday percentage questions
from plain numbers. Pick a question, enter your numbers, and it returns the
result, the inputs echoed back, and a one-line summary.

Everything runs locally in your browser. Nothing is uploaded to a server, and it
works offline once the page has loaded.

### What it can work out

- **Percent of a number** (`percent_of`) — what is `percent`% of `base`?
  e.g. 15% of 200 = **30**.
- **What percent** (`what_percent`) — `part` is what percent of `whole`?
  e.g. 30 is **15%** of 200.
- **Percent change** (`change`) — the percent increase or decrease from `from`
  to `to`, plus the absolute change. e.g. 200 → 230 is a **15% increase**.
- **Apply a change** (`apply_change`) — increase or decrease `base` by
  `percent`% (use a negative percent to decrease). e.g. 200 increased by 15% =
  **230**.
- **Share of a total** (`percent_of_total`) — `value` is what percent of
  `total`, plus the remaining amount and remaining percent. e.g. 30 of 200 is
  **15%**, leaving 170 (**85%**).

### Examples

- **percent_of**, `percent = 15`, `base = 200` → `30`
- **what_percent**, `part = 30`, `whole = 200` → `15%`
- **change**, `from = 200`, `to = 230` → `15%` increase (absolute change `30`)
- **apply_change**, `base = 80`, `percent = -25` → `60`
- **percent_of_total**, `value = 30`, `total = 200` → `15%` (remaining `85%`)

### How the values are computed

Standard arithmetic is used throughout — `percent_of` is `percent/100 · base`,
`what_percent` is `part/whole · 100`, `change` is `(to − from)/from · 100`,
`apply_change` is `base · (1 + percent/100)`, and `percent_of_total` is
`value/total · 100`. Inputs may be negative; only divisors (`whole`, `from`,
`total`) must be non-zero. Results are rounded to six decimal places.

### FAQ

<details>
<summary>Which numbers do I enter?</summary>

Only the ones the chosen question uses — the rest
are ignored. The field labels list which question each number belongs to.

</details>

<details>
<summary>Can I use negative numbers?</summary>

Yes. A negative `percent` in `apply_change`
decreases the base, and negative values are allowed anywhere except as a
divisor.

</details>

<details>
<summary>Is it free and private?</summary>

Yes. Your input never leaves your device, and it
works offline.

</details>
