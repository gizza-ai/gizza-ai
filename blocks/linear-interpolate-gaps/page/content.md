## About this tool

`linear-interpolate-gaps` fills the holes in an ordered numeric series. Every missing cell is replaced by the value on the straight line drawn between the nearest known value before it and the nearest known value after it — the same thing a spreadsheet's fill-series does between two anchors, and the same thing `interpolate(method="linear")` does in a dataframe.

Paste a column of readings with blanks, `NA`, `null` or `-` in it. What comes back is the same column with the gaps filled. The numbers you typed are echoed **verbatim**: a value entered as `1.00` comes back as `1.00`, not `1`, so nothing you did not ask for gets reformatted.

Two-column data is the part most quick converters get wrong. If every row has exactly two fields, they are read as `x,y` pairs and the interpolation uses the **real x spacing**. A sensor that logged at minute 0, 5, 10 and 20 is not the same series as four evenly spaced rows, and the answers differ.

### Worked example

Two readings, four missing months between them, defaults everywhere:

```
10




20
```

Result:

```
10
12.5
15
17.5
20
```

Now the same shape with real timestamps, as `x,y` rows:

```
minute,reading
0,20
5,
10,
20,50
```

Result — the step from 10 to 20 is twice as wide as the others, so the values are not evenly spaced:

```
0,20
5,27.5
10,35
20,50
```

### Choosing the options

- **How to read the data** — `auto` (default) reads `x,y` pairs when there are two or more rows and every row has exactly two fields, otherwise a plain list. Force it with `values` (positions 1, 2, 3 … act as the x axis) or `xy`.
- **Max gap to fill** — `0` (default) fills every hole. Set `3` and a run of four or more blanks is left alone, which is how you stop a two-month outage being invented from the readings either side of it. Under **both** directions an over-long run is still filled `max_gap` cells in from each end and left blank in the middle.
- **Fill direction** — `both` (default), `forward` (only after a known value, the ffill direction) or `backward`. On interior gaps with no max gap all three give the identical straight line; the setting starts to matter once a run is clipped, and it decides whether the leading and trailing gaps are eligible at all.
- **Gaps outside the known range** — the blanks before the first known value and after the last have only one anchor, so there is no line to draw. `leave` (default) keeps them empty, `hold` repeats the nearest known value, `extrapolate` extends the slope through the two nearest known points.
- **Extra missing markers** — add your own sentinels, such as `-999` or `missing`, so they become gaps instead of failing the parse as non-numbers.
- **Decimals** — rounds only the values this tool computes, 0 to 12, trailing zeros trimmed.
- **Output** — the filled series, a CSV with a `known` / `filled` / `missing` status per row, or JSON with counts and a per-gap report.

Everything runs locally in WebAssembly in your browser. Nothing is uploaded, there is no account, and there is no daily quota.

### Limits and edge cases

- The cap is 100,000 values per run. Longer series are rejected with a count — split them and run the parts.
- At least one known value is required. An all-blank series is an error, not an empty result: linear interpolation has nothing to anchor on.
- In `x,y` layout the x column must be numeric and **strictly increasing**. A repeated or out-of-order x is rejected with the row number, because the line between two points at the same x is undefined. Sort first.
- Only y may be missing. A blank x is an error, again with the row number.
- `extrapolate` with exactly one known value degrades to `hold` — a single point has no slope.
- `extrapolate` can produce negative or physically impossible values (a falling series extended past zero). It is off by default for that reason.
- Recognised as missing out of the box, case-insensitively: an empty field, `na`, `n/a`, `nan`, `null`, `none`, `nil`, `-`, `--`, `?`. Anything else non-numeric is an error naming the position and the token.
- A leading header row or a single label before the numbers is skipped automatically.
- Fields split on commas, semicolons and tabs — which preserves empty cells — otherwise on runs of whitespace. A row of `1 2 3` is three values; a row of `1,,3` is three values with the middle one missing.
- This is straight-line interpolation only. It will not fit a spline, a polynomial or a seasonal model, and it does not smooth the values you already have.

## FAQ

<details>
<summary>How is this different from a forward fill?</summary>

A forward fill copies the last known value forward, so a gap between 10 and 20 becomes `10, 10, 10, 20` — a staircase. Linear interpolation draws the line, so it becomes `10, 12.5, 15, 17.5, 20` — a ramp. If you want the staircase behaviour at the ends of the series, set **Gaps outside the known range** to `hold`; that is exactly a forward/backward fill for the cells that have no second anchor.

</details>

<details>
<summary>Why did my long gap only fill partway?</summary>

Because **Max gap to fill** clipped it. With a limit of 2 and direction `both`, a run of seven blanks is filled two cells in from the left and two in from the right, and the three in the middle stay empty — the ends are close enough to a real reading to trust, the middle is not. Set the limit to `0` to fill everything, or switch the direction to `forward` or `backward` to fill from one side only. The `json` output names every run it refused and why.

</details>

<details>
<summary>My timestamps are not evenly spaced. Does that matter?</summary>

It matters a lot, and it is handled. Paste the data as two columns — `x,y`, one pair per row — and the interpolation uses the real distance along the x axis instead of the row number. Between `(0, 20)` and `(20, 50)`, the value at x = 5 is `27.5`, not the `35` you would get by treating the rows as equal steps. Dates need converting to numbers first (a day index or an epoch value); the x column has to be numeric.

</details>

<details>
<summary>What happens to blanks at the very start or end of the series?</summary>

By default they are left blank. Those cells sit outside the known range, so there is no pair of anchors to draw a line between, and filling them means extrapolating — inventing data past your last measurement. Choose `hold` to repeat the nearest known value, or `extrapolate` to extend the slope through the two nearest known points. Note that `extrapolate` can run negative, so check the result against what the quantity can physically be.

</details>

<details>
<summary>Can I use my own missing marker, like -999?</summary>

Yes. Put it in **Extra missing markers** — comma-separated, case-insensitive — and it is treated as a gap on top of the built-in list (blank, `na`, `n/a`, `nan`, `null`, `none`, `nil`, `-`, `--`, `?`). This is worth doing for logger sentinels: without it, `-999` parses as a perfectly valid number and gets used as an anchor, which drags the interpolated values around it wildly out of range.

</details>

<details>
<summary>How do I tell which numbers were filled and which were mine?</summary>

Switch **Output** to `csv` and every row comes back with a status of `known`, `filled` or `missing`, so the generated values are labelled in the data itself. The `json` output goes further: counts of each status, the numeric array with unfilled gaps as `null`, and a report of every gap run giving its start, end, length, kind (`leading`, `interior` or `trailing`) and how many cells were filled.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The whole calculation is a WebAssembly module running in your browser tab, so the series never leaves your machine. The same code ships in the `gizza` CLI if you would rather fill gaps from a terminal or a script.

</details>
