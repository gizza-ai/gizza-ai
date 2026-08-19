## About this tool

NDJSON (also called JSON Lines or JSONL) is great for logs and streams because each line is a complete JSON record. It is less convenient when you need a rectangular table: one record may have `user.tier`, the next may omit it, and nested payloads can hide the numeric columns you actually want.

This tool parses each non-blank line independently, flattens nested objects into column paths such as `user.geo.lat`, takes the union of every path it sees, and writes one aligned row per record. Missing paths and JSON `null` become your chosen fill value, so ragged event streams become CSV, TSV, a whitespace-aligned matrix, or a JSON array of rows without writing a script.

### Worked example

Input:

```json
{"id":1,"latency_ms":12,"user":{"tier":"pro"}}
{"id":2,"latency_ms":940}
{"id":3,"latency_ms":31,"user":{"tier":"free"}}
```

With the defaults (`format=csv`, headers on, first-seen columns, blank fill) the output is:

```csv
id,latency_ms,user.tier
1,12,pro
2,940,
3,31,free
```

Turn on **Numeric columns only**, set **Fill for missing cells** to `0`, and turn headers off when you need a bare numeric matrix for `numpy.loadtxt`, R, Octave, or a plotting package. Use **Columns to keep** when you want a stable schema in a specific order.

### Controls that matter

- **Output format** — CSV, TSV, aligned `matrix`, or `json` array-of-arrays.
- **Nested arrays** — index arrays into columns (`reading.0`, `reading.1`), keep the full array as JSON in one cell, or skip array-valued columns.
- **Column order** — first seen in the stream, alphabetical for stable diffs, or coverage-first so the most-populated columns appear first.
- **Max depth** — cap flattening and keep deeper objects as compact JSON when a payload would explode into too many columns.
- **Invalid lines** — stop at the first bad line with its line number, or skip malformed lines and convert the rest.

### Limits and edge cases

- Input is capped at **5,000,000 bytes**, **50,000 non-blank lines**, and **2,000 distinct column paths**.
- JSON objects become rows keyed by flattened paths. Bare JSON arrays become positional columns `0`, `1`, `2` and bare scalars go into a single `value` column.
- `numeric_only` keeps columns whose present values are all finite numbers; numeric-looking JSON strings count, but booleans, labels and non-finite tokens do not.
- Duplicate paths inside one record keep the last value written, which can happen when path separators collide with literal key names.
- CSV output follows RFC 4180 quoting for delimiters, quotes and newlines inside cells.

## FAQ

<details>
<summary>What is the difference between NDJSON and a JSON array?</summary>

NDJSON has one complete JSON value per line: `{...}\n{...}\n{...}`. A JSON array wraps all records in one value: `[{...},{...}]`. This tool expects NDJSON because that is how logs, export streams and append-only data files are usually stored. If you have one big JSON array, split it into one element per line first.

</details>

<details>
<summary>How are nested objects and arrays turned into columns?</summary>

Object keys are joined with the path separator. With the default separator, `{"user":{"id":7}}` becomes column `user.id`. Arrays default to indexed columns, so `{"v":[10,20]}` becomes `v.0` and `v.1`; switch **Nested arrays** to JSON to keep `[10,20]` in one cell, or to skip to drop array-valued data entirely.

</details>

<details>
<summary>How do I make a numeric matrix with no labels?</summary>

Set **Numeric columns only** on, choose a fill such as `0` or `NaN`, and turn **Include the header row** off. The `matrix` format produces a whitespace-aligned grid, while CSV/TSV produce delimiter-separated rows that common numeric tools can load.

</details>

<details>
<summary>What happens when one line is not valid JSON?</summary>

With **Unparsable lines: error**, conversion stops and the error names the line number and parser location. With **skip**, malformed lines are ignored and the remaining records are converted. If every non-blank line is invalid, the tool still returns an error instead of emitting an empty table.

</details>

<details>
<summary>Can I force a schema instead of using every discovered column?</summary>

Yes. Put a comma-separated list in **Columns to keep**, such as `timestamp, latency_ms, user.tier`. The output uses exactly that order and fails if a requested path is absent, so typos do not silently produce empty columns.

</details>
