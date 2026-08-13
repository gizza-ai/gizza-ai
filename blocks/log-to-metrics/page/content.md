## Logs into RED metrics

Paste structured log lines and aggregate them into the numbers you usually need for an incident review or a dashboard seed: request counts, traffic share, rate over the log's own time span, error counts, error percentage, and exact latency percentiles. The tool reads NDJSON (one JSON object per line), logfmt `key=value` records, and CSV/TSV blocks with a header row. It runs locally in the browser; nothing is uploaded.

### Worked example

Input NDJSON:

```
{"ts":"2024-05-06T07:00:00Z","route":"/api/users","status":200,"duration_ms":37}
{"ts":"2024-05-06T07:00:20Z","route":"/api/users","status":500,"duration_ms":412}
{"ts":"2024-05-06T07:00:40Z","route":"/api/users","status":200,"duration_ms":58}
{"ts":"2024-05-06T07:01:00Z","route":"/api/orders","status":200,"duration_ms":120}
```

Set **Group by fields** to `route`, **Numeric field** to `duration_ms`, and **Error field** to `status`. The output shows one row per route with `count`, `percent`, a rate column, `errors`, `error_%`, `min`, `avg`, `p50`, `p95`, `p99`, `max`, and `sum`. Sort by `p_top` when you want the slowest endpoints first; sort by `errors` when you want the noisiest groups first.

### Options that matter

- **Input format** can auto-detect JSON/NDJSON, logfmt, and CSV. Set it explicitly when a sample is mixed or ambiguous.
- **Group by fields** accepts up to five comma-separated fields. Nested JSON is flattened to dotted paths, so `http.status` works.
- **Numeric field** is optional. When present, plain numbers are used as-is and durations such as `250ms`, `1.5s`, `10us`, or `2m` are normalised to milliseconds.
- **Percentiles** are exact over the pasted batch. Choose `linear` interpolation for numpy/R-style values, or `nearest` when you want a percentile to be an observed value.
- **Timestamp field** drives the rate column. Leave it blank to auto-detect common timestamp names, or set it to `none` for count-only logs.
- **Error field** plus **Values that count as errors** handles HTTP classes (`5*`), numeric comparisons (`>=500`), and severity strings (`error`, `fatal`, `panic`).
- **Output format** can be a Markdown-style table, JSON report, CSV, or Prometheus text exposition.

### Limits and edge cases

- Maximum input is **2,000,000 characters**, **200,000 lines**, and **50,000 distinct groups** per run.
- This is a one-shot batch aggregator. It does not tail a file, store state, or build time-bucketed series.
- Missing group fields are labelled `(missing)` rather than dropped.
- Missing or non-numeric values in the numeric field are counted in the summary and not treated as zero.
- If **Roll the remainder into an (other) row** is enabled, that row recomputes percentiles from the merged raw values; it does not average the visible rows.

## FAQ

<details>
<summary>What log formats can I paste?</summary>

Use NDJSON with one JSON object per line, logfmt records such as `route=/api status=500 dur=42ms`, or a CSV/TSV table with a header row. JSON arrays are not the intended input here; split them to one object per line first.

</details>

<details>
<summary>How is the rate calculated?</summary>

The tool reads the earliest and latest parseable timestamps in the batch and divides each group's count by that span. It is a batch rate over the log window you pasted, not a moving average. Set **Timestamp field** to `none` if the input has no timestamps.

</details>

<details>
<summary>Are the percentiles approximate?</summary>

No. Every parsed numeric value is kept, sorted, and reduced exactly. The `linear` method interpolates between neighbouring values; `nearest` uses nearest-rank and always returns a value that occurred in the log.

</details>

<details>
<summary>What counts as an error?</summary>

Only rows whose **Error field** matches one of the rules. Blank rules use the built-in set: `5*`, `error`, `err`, `fatal`, `critical`, `crit`, `panic`, `emerg`, and `alert`. You can also write numeric comparisons such as `>=500` or prefixes such as `4*`.

</details>

<details>
<summary>Is my log data uploaded?</summary>

No. The parser and aggregator are compiled to WebAssembly and run in your browser tab. Logs often contain user IDs, hostnames, URLs, and tokens, so the page is designed for local-only processing.

</details>
