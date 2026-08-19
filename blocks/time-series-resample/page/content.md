## About this tool

Resampling is the time-series version of "group by": every row is dropped into a fixed
time bucket — 15 minutes, an hour, a day, a week, a month, a quarter, a year — and the
values inside each bucket are collapsed into one number. It is how minute-by-minute sensor
readings become an hourly chart, how daily orders become weekly totals, and how raw trades
become OHLC candles.

Paste the CSV, pick an interval, pick an aggregate. Everything runs locally in your
browser via WebAssembly — the series is never uploaded.

### Worked example

Input (half-hourly readings) with **interval `1h`** and **aggregate `mean`**:

```
time,temp
2024-05-01T10:00:00Z,10
2024-05-01T10:30:00Z,20
2024-05-01T11:15:00Z,30
```

Output — the 10:00 bucket averages 10 and 20, the 11:00 bucket holds only 30:

```
time,temp
2024-05-01T10:00:00Z,15
2024-05-01T11:00:00Z,30
```

Switch aggregate to `ohlc` and each value column expands into four:
`temp_open,temp_high,temp_low,temp_close`.

### What it accepts

- **Delimiters:** comma, tab, semicolon or pipe — auto-detected from the first line and
  reused in the CSV output. Quoted cells with embedded delimiters are handled.
- **Headers:** optional. A first row that is entirely timestamps/numbers is treated as
  data, and the output columns are named `time`, `column2`, `column3`…
- **Timestamps:** ISO-8601 / RFC-3339 (`2024-05-01T13:20:00Z`, with or without an offset —
  offsets are converted to UTC), `2024-05-01 13:20`, plain `2024-05-01`, and bare epoch
  numbers (values of 1e11 or more are read as milliseconds, smaller ones as seconds).
- **Row order:** any. Rows are sorted before bucketing, and buckets come back
  chronologically.
- **Value columns:** by default every column beside the timestamp whose cells are all
  numeric or blank. Name specific ones with `value_columns` (header names or 1-based
  numbers). Blank cells are ignored rather than counted as zero.

### Limits

Input is capped at **2,000,000 bytes** and **200,000 data rows**, and the result is capped
at **100,000 buckets** — a very fine interval over a long span hits that cap and asks for a
coarser one. Everything is computed in **UTC**; use `offset` to move the day boundary.

### Upsampling

An interval *finer* than the data works too. With `fill` set to anything but `skip`, the
buckets between two rows are created and then filled — `previous` holds the last value,
`linear` interpolates. Daily `10` and `30` two days apart become `10, 20, 30` hourly-style
at `1d`/`linear`.

## FAQ

<details>
<summary>What is the difference between `label` and `closed`?</summary>

`closed` decides which rows land in a bucket; `label` only decides what timestamp is
printed. With `closed = left` (the default) a bucket covers `[start, end)`, so a row exactly
on an edge opens the new bucket. With `closed = right` it covers `(start, end]`, so an
exact-edge row closes the previous one. `label = end` prints the closing edge instead of the
opening one, which is what many reporting tools expect for "week ending" columns.

</details>

<details>
<summary>How do I get days that start at midnight in my own timezone?</summary>

Set `offset` to your UTC difference. `offset = -5h` with `interval = 1d` makes each day run
from 05:00 UTC to 05:00 UTC, which is midnight-to-midnight at UTC-5. The tool has no
timezone database and does not apply daylight-saving transitions — the shift is a fixed
duration, so pick the offset that matches the period you are summarising.

</details>

<details>
<summary>What does `origin` change?</summary>

It moves the whole bucket grid. `epoch` (the default) anchors edges to the Unix epoch, so
hourly buckets start on the hour and weekly buckets start on a Monday. `start` anchors them
to the first row's exact timestamp, so a series beginning at 10:20 gets buckets at 10:20,
11:20, … `start_day` anchors to UTC midnight of the first row's day. Month, quarter and year
buckets always start on the 1st, so `origin` does not affect them.

</details>

<details>
<summary>What happens to intervals with no data?</summary>

By default (`fill = skip`) they simply do not appear — the output only contains buckets that
had rows. `empty` emits the bucket with blank values, `zero` writes 0, `previous` carries the
last known value forward, and `linear` interpolates between the values on either side of the
gap. `linear` leaves a leading or trailing gap blank, because there is nothing on one side to
interpolate from.

</details>

<details>
<summary>Which aggregate should I use for counting events?</summary>

`count` — it reports how many rows in the bucket had a number in that column, so blank cells
are not counted. If your rows are events with no numeric payload, add a column of `1`s and
use `sum`, or point `value_columns` at any always-populated numeric column and use `count`.

</details>

<details>
<summary>Are `std` and `var` sample or population statistics?</summary>

Sample — they use the `n-1` denominator, the same default spreadsheets and pandas use. A
bucket holding a single value has no sample spread, so its cell comes back blank rather than
`0`.

</details>

<details>
<summary>Can I aggregate several columns at once?</summary>

Yes. Leave `value_columns` blank and every numeric column is aggregated with the chosen
function, each keeping its own header name. With `ohlc`, each of those columns expands into
four (`<name>_open`, `<name>_high`, `<name>_low`, `<name>_close`). Applying *different*
functions to different columns in one pass is not supported — run the tool once per function.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The resampler is compiled to WebAssembly and runs inside your browser tab. The CSV you
paste never leaves the page, and the same engine is available offline through the command
line.

</details>
