## About this tool

Charts choke on raw time-series data: plotting 100,000 samples into an
800-pixel-wide graph wastes bandwidth and rendering time without adding any
visible detail. This tool reduces a large series to a target number of points
while keeping the curve's visual shape — peaks, dips, and trends stay where
they are. Paste CSV (from a spreadsheet, a sensor log, or a database export) or
a JSON array, choose how many points you want, and copy the result.

The default algorithm is **LTTB (Largest-Triangle-Three-Buckets)**: it splits
the series into equal buckets and keeps, from each bucket, the point that forms
the largest triangle with its neighbours — the point that contributes most to
the line's shape. The first and last points are always kept. Three alternatives
cover other needs: **Min/Max** keeps the minimum and maximum of each bucket
(best when you must never miss a spike), **M4** keeps first/min/max/last per
bucket (the classic error-free line-rendering aggregation), and **Every n-th**
takes a plain uniform stride.

All four algorithms *select* original points — nothing is averaged or
interpolated. That means CSV rows come back verbatim: every column, the exact
original formatting, and the header row are preserved. JSON arrays come back as
the selected original elements.

## Worked example

Input — 20 readings with two spikes (45 at t=5, 80 at t=17), downsampled to
**8 points** with LTTB:

```
t,v
1,10
2,12
3,11
4,13
5,45
6,12
7,10
8,11
9,13
10,12
11,14
12,13
13,12
14,15
15,13
16,12
17,80
18,13
19,12
20,11
```

Output — both spikes and both endpoints survive:

```
t,v
1,10
4,13
5,45
8,11
11,14
16,12
17,80
20,11
```

With `output = indices` the same run returns `[0,3,4,7,10,15,16,19]` — the
0-based data-row numbers that were kept, handy for selecting rows
programmatically from the original array.

## Input formats

- **CSV** — comma, tab, or semicolon delimited (auto-detected). Two or more
  columns: the first is x (time), the second is y, unless you set
  `x_column` / `y_column` (header name or 1-based number). One column: values
  only, row order is the time axis. A non-numeric first row is treated as a
  header and kept.
- **JSON** — `[10, 12, 45]` (values only), `[[x, y], …]` pairs, or
  `[{"time": …, "value": …}, …]` objects (x/y keys auto-detected from `x`,
  `t`, `time`, `timestamp`, `ts`, `date` and `y`, `v`, `value`, `val`).
- **Timestamps** — x values may be numbers (epoch, sample number) or ISO-8601
  dates/times (`2024-01-05`, `2024-01-05T12:30:00Z`).

## Limits and edge cases

- Input is capped at **2,000,000 bytes (~2 MB)**; the target point count at
  **100,000**. A series already at or below the target is returned unchanged.
- x values must be **sorted ascending** (equal values are fine) — the tool
  reports the first out-of-order row instead of guessing. Set `x_column` to
  `index` to ignore a column and use row order.
- Non-numeric or NaN y values are an error naming the offending line — fix or
  remove those rows first.
- Quoted CSV fields may not contain line breaks; decimal commas (`1,5`) are
  not supported — use dot decimals.
- M4 needs at least 4 target points; Min/Max and M4 round the target down to
  whole buckets, so they can return slightly fewer points than requested.

## FAQ

<details>
<summary>What is LTTB and why is it the default?</summary>

Largest-Triangle-Three-Buckets (from Sveinn Steinarsson's 2013 thesis) divides
the series into `points − 2` equal buckets and keeps, per bucket, the point
forming the largest triangle with the previously kept point and the next
bucket's average. It is the standard choice for visual downsampling because
the reduced line is hard to distinguish from the original at chart resolution,
it runs in a single O(n) pass, and it always keeps the first and last point.
This implementation matches the canonical reference implementation
bucket-for-bucket.

</details>

<details>
<summary>Which algorithm should I pick?</summary>

Use **LTTB** for charts — it gives the best overall shape at a given point
budget. Use **Min/Max** when missing a single outlier spike would be
unacceptable (alerting dashboards, sensor anomaly review): it keeps every
bucket's extremes. **M4** additionally keeps each bucket's first and last
point, which makes pixel-perfect line rendering possible when buckets align
with pixel columns. **Every n-th** is a plain uniform stride — predictable
spacing, but it can skip over narrow spikes entirely.

</details>

<details>
<summary>Does downsampling change my values?</summary>

No. Every algorithm here selects a subset of your original points — nothing is
averaged, smoothed, or interpolated. CSV rows are emitted verbatim with all
their columns and the header; JSON output contains the original elements. If
you need per-bucket averages instead, that is aggregation (resampling), a
different operation from shape-preserving downsampling.

</details>

<details>
<summary>Can I get the row numbers instead of the data?</summary>

Yes — set the output to **indices** to get a JSON array of the 0-based data
row numbers that were kept (the header does not count). Use it to slice the
original dataset in your own code, the same way library implementations return
index arrays.

</details>

<details>
<summary>My CSV has many columns — which ones are used?</summary>

By default the first column is x (time) and the second is y. Point the tool at
other columns with `x_column` / `y_column`, using either the header name
(case-insensitive) or a 1-based column number. Downsampling is driven by that
one y series, but kept rows are returned whole, with every column intact.

</details>
