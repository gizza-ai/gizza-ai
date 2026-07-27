## Round the numbers in a CSV, your way

Paste a CSV and this tool rounds its **numeric cells** to the number of decimal
places you choose — locally in your browser, **nothing is uploaded**. Pick which
rounding mode you want and, optionally, which columns to touch:

- **Decimal places** — keep 0 to 10 places.
- **Rounding mode** — how ties (a trailing `5`) and every other digit are handled:
  - **Half up** — round halves away from zero (classical / spreadsheet `ROUND`): `2.5 → 3`, `-2.5 → -3`.
  - **Half down** — round halves toward zero: `2.5 → 2`.
  - **Half to even** — banker's rounding, which reduces bias: `0.5 → 0`, `1.5 → 2`, `2.5 → 2`.
  - **Ceil** — always round up (toward +∞).
  - **Floor** — always round down (toward −∞).
  - **Truncate** — just drop the extra digits (toward zero).
- **Columns** — leave blank to round every numeric column, or list columns by
  header **name** and/or **1-based number** (e.g. `price, 3`).
- **Fixed formatting** — turn on *pad to a fixed number of decimals* to always show
  the full width, so `3` becomes `3.00` at 2 places.

A first-row **header** is kept exactly as-is, and any **non-numeric** cell (text, a
`$` price, a `%` value) passes through unchanged.

### Worked example

With **2 decimal places**, **half up**, and columns left blank, this input:

```
name,price,qty
Apple,1.005,3.14159
Pear,2.5,10
```

becomes:

```
name,price,qty
Apple,1.01,3.14
Pear,2.5,10
```

`1.005` rounds to `1.01`, `3.14159` to `3.14`, and `2.5` / `10` already fit in two
places so they're unchanged. The `name` column is text, so it's left alone.

### Why `1.005` rounds to `1.01` here (and not in a spreadsheet)

Most tools multiply by `100` and round, but `1.005` can't be stored exactly in
binary — it becomes `1.00499999…`, so the naive result is `1.00`. This tool rounds
on the **decimal digits you typed**, so `1.005` at two places gives the `1.01` you
expect.

### FAQ

<details>
<summary>What's the difference between "half up" and "banker's" rounding?</summary>

**Half up** (the default) rounds a trailing `5` away from zero, so `0.5 → 1`,
`1.5 → 2`, `2.5 → 3`. **Half to even** (banker's) rounds a trailing `5` to the
nearest *even* digit, so `0.5 → 0`, `1.5 → 2`, `2.5 → 2`. Banker's rounding is
common in finance and statistics because always rounding halves upward biases a
running total; sending them to the nearest even value cancels out over many rows.

</details>

<details>
<summary>How do I round only some columns?</summary>

Put the columns you want in the **Columns** box, separated by commas. You can mix
header **names** and **1-based numbers** — `price, 3` rounds the column named
`price` and the third column. Leave the box blank to round every numeric column.
Columns you don't list are copied through untouched.

</details>

<details>
<summary>What happens to text, blank, or currency cells?</summary>

Anything that isn't a plain number is left exactly as-is. A header row (when *First
row is a header* is on), a name like `Apple`, a blank cell, and a formatted value
like `$12.99` or `4.7%` all pass through unchanged — only clean numeric cells get
rounded.

</details>

<details>
<summary>Can I keep trailing zeros so every value has the same width?</summary>

Yes. Turn on **Pad to a fixed number of decimals** and every rounded cell shows
exactly the number of places you chose — `3` becomes `3.00` and `2.5` becomes
`2.50` at two places. With it off, results are shown naturally, dropping trailing
zeros (`2.50 → 2.5`).

</details>

<details>
<summary>Can it read tab- or semicolon-separated files?</summary>

Yes. Set the **Delimiter** to match your file — a single character, or one of
`comma` / `tab` / `semicolon` / `pipe`. The output uses the same delimiter, and
quoted fields that contain the delimiter are parsed correctly.

</details>

### Limits & edge cases

- Decimal places range from **0 to 10**.
- Rounding operates on the digits you paste for plain decimals; values written in
  **scientific notation** (`1.5e3`) are rounded via floating point, which can differ
  in the last place for extreme inputs.
- Numbers with **thousands separators** or symbols (`1,234.50`, `$9.99`, `50%`) are
  treated as non-numeric and left unchanged — the comma is also a CSV delimiter, so
  strip separators before rounding.
- `-0` never appears: a value that rounds to zero is written as a plain `0`.
- Everything runs in memory in your browser, so very large files are bounded by the
  memory available to the page.
