## About this tool

Spreadsheets make you build a helper column and drag a formula down just to strip a minus sign.
This tool does the same job on a whole pasted column in one pass: choose an operation, paste the
numbers, and copy the transformed column back out.

Four operations cover the usual sign chores:

- **Absolute value** — `|x|`, the distance from zero. `-3` becomes `3`, `3` stays `3`, `0` stays `0`.
- **Sign** — the signum function: `-1` for any negative value, `0` for zero, `1` for any positive
  value. Useful for turning a column of amounts into a direction flag.
- **Negate** — multiply every value by `-1`, so signs swap in both directions.
- **Force negative** — `-|x|`, which turns a mixed column into all-negative values (the usual fix
  when an export gives you credits and debits with inconsistent signs).

### Worked example

With **Operation** set to *Absolute value* and this input:

```
-3
4
-2.5
0
```

the output is:

```
3
4
2.5
0
```

Switch **Operation** to *Sign* on the same input and you get `-1`, `1`, `-1`, `0`. Switch it to
*Flip sign* and you get `3`, `-4`, `2.5`, `0`.

### Options

**Input separator** defaults to auto-detect, which splits on newlines, commas, semicolons, pipes,
or spaces — so a pasted spreadsheet column and a comma list both just work. **Output separator**
defaults to mirroring the input, so a column comes back as a column.

**Round decimals** applies fixed rounding (0–6 places) to the results; leave it on *Keep full
precision* to get whole numbers without a trailing `.0`. **Append summary stats** adds a
count/sum/min/max/mean block computed over the transformed values.

**Output format** switches between the plain column, a tab-separated audit table with each original
value next to its result, and a JSON report with counts of transformed and invalid values.

### Limits

Up to **20,000 values** per run. Values must be plain decimals or scientific notation (`-3`, `4.5`,
`-1.2e3`, `+2`). Currency symbols, thousands separators, percent signs, and fractions like `1/4` are
rejected rather than guessed at — by default the run stops and names the offending value, and the
*Values that are not numbers* option lets you skip, keep, or blank them instead.

## FAQ

<details>
<summary>What is the difference between absolute value and force negative?</summary>

Absolute value returns `|x|`, so everything comes out zero or positive. Force negative returns
`-|x|`, so everything comes out zero or negative. Both ignore the input sign entirely — unlike
*Flip sign*, which preserves the distinction and simply swaps each value's direction.

</details>

<details>
<summary>How does the sign operation handle zero?</summary>

Zero has no direction, so `sign(0)` is `0`. Only strictly negative values return `-1` and only
strictly positive values return `1`. A very small value like `0.001` is still positive, so it
returns `1`.

</details>

<details>
<summary>My column has currency symbols and commas — why does it fail?</summary>

This tool only changes signs; it deliberately does not guess at formatting, because `1,234` is
ambiguous once commas are also a separator. Clean the column first with the numeric string
sanitizer tool, then run it through here. If you would rather push a messy column through as-is,
set *Values that are not numbers* to skip, keep, or blank.

</details>

<details>
<summary>Can I keep the original values next to the results?</summary>

Yes — set **Output format** to *Audit table*. You get a tab-separated table with an `original`
column and a `result` column, which pastes straight back into a spreadsheet as two columns. The
JSON output carries the same pairing plus counts of how many values were transformed and how many
were invalid.

</details>

<details>
<summary>Does the output stay in the same order as the input?</summary>

Yes. Values are transformed in place and emitted in input order, so row *n* of the output matches
row *n* of the input. That holds for the blank and keep error modes too, which is why they exist;
the skip mode is the one option that changes the row count.

</details>

<details>
<summary>Why do rounded zeros not print as -0.00?</summary>

Rounding a tiny negative value like `-0.004` to two places gives negative zero, which is
mathematically equal to zero but reads as a bug in a report. Any result that rounds to zero is
normalised to a plain `0`, so you never see `-0` or `-0.00` in the output.

</details>
