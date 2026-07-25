## Round every number in a CSV to a multiple you choose

Paste a CSV and this tool rounds its **numeric cells** to the nearest multiple of
any step — `0.25`, `0.05`, `5`, `100`, `1000`, whatever you need — locally in your
browser, **nothing is uploaded**. It's the batch, spreadsheet-`MROUND` version of
"round to the nearest 5 / quarter / thousand", over a whole column at once.

- **Multiple / step** — the value every result must be a multiple of. `0.25` snaps
  to quarters, `0.05` to nickels, `1000` to thousands. Must be greater than 0.
- **Rounding mode** — how a value between two multiples is resolved:
  - **Nearest, ties away from zero** — the classical / spreadsheet `MROUND` rule: an exact halfway rounds away from zero. `1.375 → 1.5` at step `0.25`, `-2.5 → -3` at step `5`.
  - **Nearest, ties toward zero** — an exact halfway rounds toward zero instead.
  - **Nearest, banker's ties** — an exact halfway rounds to the *even* multiple, which reduces bias over many rows.
  - **Up** — always to the next multiple (toward +∞): `41 → 45` at step `5`.
  - **Down** — always to the previous multiple (toward −∞): `49 → 45` at step `5`.
  - **Toward zero** — drop to the lower-magnitude multiple: `7.9 → 5`, `-7.9 → -5`.
- **Columns** — leave blank to round every numeric column, or list columns by
  header **name** and/or **1-based number** (e.g. `price, 3`).
- **Fixed formatting** — turn on *pad to the step's decimal places* so a step of
  `0.25` always shows two places: `1.00`, `1.25`, `1.50`.

A first-row **header** is kept exactly as-is, and any **non-numeric** cell (text, a
`$` price, a `1,234` with a thousands separator) passes through unchanged.

### Worked example

With step **0.05**, mode **nearest (ties away)**, and columns left blank, this input:

```
name,price
Apple,1.23
Pear,2.42
```

becomes:

```
name,price
Apple,1.25
Pear,2.4
```

`1.23` is closer to `1.25` than to `1.20`, so it rounds up; `2.42` is closer to
`2.40`, so it rounds down (shown as `2.4` with trailing zeros off). The `name`
column is text, so it's left alone.

### Why the ties are exact here (and not a floating-point artifact)

The formula is `result = step × round(value ÷ step)`, but `value ÷ step` in binary
floating point can land at `2.4999999…` when it should be exactly `2.5`, flipping a
tie the wrong way. This tool computes the quotient with **exact integer arithmetic
on the digits you typed**, so `0.125` to the nearest `0.05` is a true halfway and
follows the tie rule you picked — `0.15` with ties-away, `0.10` with ties-toward.

### FAQ

<details>
<summary>What's the difference between this and just rounding decimals?</summary>

Rounding **decimals** snaps a number to a number of places — `1.237 → 1.24` at two
places. Rounding to a **multiple** snaps it to a step that need not be a power of
ten — `1.237 → 1.25` at step `0.05`, or `137 → 135` at step `5`. Use this tool when
you want prices to the nearest nickel, weights to the nearest `0.25`, or totals to
the nearest `1000`, rather than to a fixed number of decimal places.

</details>

<details>
<summary>How do "up", "down", and the three "nearest" modes differ?</summary>

**Up** and **Down** ignore distance: every value goes to the next multiple up
(`41 → 45` at step `5`) or the previous multiple down (`49 → 45`), regardless of how
close it is. The three **nearest** modes go to whichever multiple is closer and only
differ on an *exact halfway*: **ties away from zero** sends it away (`2.5 → 3`),
**ties toward zero** brings it back (`2.5 → 2`), and **banker's** sends it to the
even multiple (`2.5 → 2`, `3.5 → 4`) to avoid bias.

</details>

<details>
<summary>How do I round only some columns?</summary>

Put the columns you want in the **Columns** box, separated by commas. You can mix
header **names** and **1-based numbers** — `price, 3` rounds the column named
`price` and the third column. Leave the box blank to round every numeric column;
columns you don't list are copied through untouched.

</details>

<details>
<summary>Can I keep a fixed width like 1.00, 1.25, 1.50?</summary>

Yes. Turn on **Pad to the step's decimal places** and every rounded cell shows
exactly as many places as the step — a step of `0.25` gives `1.00`, `1.25`, `1.50`,
and a step of `0.05` gives `1.20`, `1.25`. With it off, results are shown naturally,
dropping trailing zeros (`1.50 → 1.5`). An integer step like `5` has no decimals, so
padding leaves whole numbers unchanged.

</details>

<details>
<summary>Can it read tab- or semicolon-separated files?</summary>

Yes. Set the **Delimiter** to match your file — a single character, or one of
`comma` / `tab` / `semicolon` / `pipe`. The output uses the same delimiter, and
quoted fields that contain the delimiter are parsed correctly.

</details>

### Limits & edge cases

- The **step** must be greater than 0; it can be a decimal (`0.25`), a whole number
  (`5`), or large (`1000`).
- Rounding operates on the digits you paste for plain decimals; values written in
  **scientific notation** (`1.5e3`) are rounded via floating point, which can differ
  in the last place for extreme inputs.
- Numbers with **thousands separators** or symbols (`1,234.50`, `$9.99`, `50%`) are
  treated as non-numeric and left unchanged — the comma is also a CSV delimiter, so
  strip separators before rounding.
- `-0` never appears: a value that rounds to zero is written as a plain `0`.
- Everything runs in memory in your browser, so very large files are bounded by the
  memory available to the page.
