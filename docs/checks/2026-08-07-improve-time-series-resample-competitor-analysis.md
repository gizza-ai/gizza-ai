# time-series-resample competitor analysis (2026-08-07)

## Scope

Tool: `time-series-resample` — resample or aggregate timestamped CSV/TSV data to a different interval, such as minute to hour, day to week, or ticks to OHLC candles.

## Sources checked

- pandas `DataFrame.resample` / `Series.resample` documentation and examples.
- DataCamp tutorial coverage of pandas `resample()` and `asfreq()`.
- Alpharithms article on aggregating minute-level crypto OHLC data with pandas resampling.
- Earth Data Science lesson on hourly-to-daily/weekly/monthly summaries.
- d2xlab browser time-series editor landing page for no-code/browser UX expectations.

## Table-stakes capabilities

| Capability | Seen in competitors | In model? | Decision |
| --- | --- | --- | --- |
| Choose the timestamp column / datetime index | pandas `on=` or index setup; tutorials set a datetime index | Yes | `time_column` accepts a name, case-insensitive name, or 1-based column number; blank means first column. |
| Common intervals from minutes through years | pandas rules such as 5T/15T/H/D/W/M/Q/Y; tutorials show hourly, daily, weekly, monthly | Yes | `interval` accepts `ms`, `s`, `m`, `h`, `d`, `w`, `mo`, `q`, `y` with numeric multipliers. |
| Multiple aggregates | pandas `.mean()`, `.sum()`, `.min()`, `.max()`, `.count()`, `.agg()`; tutorials show sum/mean/OHLC | Yes | `aggregate` enum covers mean, sum, min, max, count, median, first, last, std, var, and OHLC. |
| Aggregate several numeric columns at once | pandas resamples entire frames; tutorials aggregate multiple columns | Yes | Blank `value_columns` auto-selects every numeric non-time column; explicit column list supported. |
| OHLC candle generation for prices | Alpharithms and pandas examples use OHLC mappings | Yes | `ohlc` expands each selected value column into `_open`, `_high`, `_low`, `_close`. |
| Upsampling and gap filling | pandas `asfreq`, fill methods, interpolation examples | Yes | `fill` enum covers skip, empty, zero, previous, and linear. |
| Boundary label and closed side | pandas `label` and `closed` parameters | Yes | `label` and `closed` enums mirror the bucket-labelling and inclusivity controls. |
| Origin/offset alignment | pandas `origin` and `offset` parameters | Yes | `origin` covers epoch/start/start_day for fixed widths; `offset` shifts fixed-duration edges. |
| Timezone database / daylight-saving-aware calendar days | pandas can use timezone-aware indexes | Out of model | gizza blocks are local wasm without a timezone database. The tool converts offsets to UTC and offers fixed `offset` shifts only. |
| Arbitrary pandas-style per-column aggregate mapping | pandas `.agg({col: fn})` supports different functions per column | Out of model for first version | The descriptor keeps one aggregate for all selected columns to fit a compact CLI/page surface. Users can run the tool multiple times for different functions. |
| Visualization/editing workspace | browser editors can chart and edit series | Out of model | This repository builds deterministic transform tools, not an interactive charting app. Output stays CSV/JSON text. |

## Defaults and UX choices

- Default interval: `1h`, because hourly downsampling is the common first example in competitor material.
- Default aggregate: `mean`, matching the most common sensor-style resample demonstration.
- Default fill: `skip`, so downsampling never invents empty rows unless the user opts into upsampling/gap fill.
- Select controls are used for finite choices: aggregate, label, closed, fill, origin, time format, and output.
- Text fields are used where users need flexible strings: CSV input, interval expressions, time/value column selectors, and offset.
- Preset chips cover the main competitor examples: hourly mean, weekly sums, OHLC candles, interpolated gaps, shifted day boundary, and monthly JSON.

## Worked examples to support

1. Half-hour readings to hourly mean:

```text
time,temp
2024-05-01T10:00:00Z,10
2024-05-01T10:30:00Z,20
2024-05-01T11:15:00Z,30
```

with `interval=1h`, `aggregate=mean` should produce hourly rows of `15` and `30`.

2. Daily sales to weekly sums using date labels.

3. Trade ticks to hourly OHLC columns.

4. Sparse series upsampled with linear interpolation.

## Limits and honesty notes

- Input caps: 2,000,000 bytes, 200,000 parsed rows, and 100,000 output buckets to avoid unbounded wasm/page work.
- Calendar buckets are UTC-based. Month/quarter/year buckets always start on the first day of the UTC calendar period.
- DST and timezone-name handling are intentionally not promised; the tool accepts explicit offsets and fixed bucket shifts.
- This tool is not a semantic duplicate of existing CSV/statistics blocks: those summarize tabular columns or compute rolling/window functions, while this one is specifically timestamp-bucket resampling with boundary, origin, gap-fill, and OHLC controls.
