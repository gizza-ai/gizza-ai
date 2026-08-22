# line-protocol-csv-converter — competitor analysis (2026-08-22)

Scan run BEFORE implementation. Goal: convert between InfluxDB line protocol and CSV in both
directions, for inspection (LP → spreadsheet) and bulk import (CSV → LP).

## Competitors reviewed

| # | Tool | What it is | Reachable |
|---|------|-----------|-----------|
| 1 | InfluxData `influx write` / "Write CSV data to InfluxDB" docs | The official, canonical CSV → line protocol path (annotated CSV) | yes |
| 2 | `github.com/influxdata/influxdb/v2/pkg/csv2lp` (Go package, also wrapped as the `csv2lp` CLI) | The library behind `influx write --format csv` | yes |
| 3 | `qn7o/csv2influx` | Small OSS CLI: flag-driven column mapping, no annotations | yes |
| 4 | InfluxData line protocol syntax reference | The normative grammar/escaping/type rules both directions must obey | yes |

Supporting context from the search: `influxify` (PyPI) writes a `.lp` file from a CSV;
`nhancv/nc-csv2influxdb` does the same with pandas. Both are thin re-implementations of the
same flag-driven mapping as #3, so they added no new table stakes.

## Table-stakes parameters (union of the four)

| Capability | Source | In model? |
|---|---|---|
| Bidirectional conversion (LP → CSV **and** CSV → LP) | none of them do both — #1/#2/#3 are CSV → LP only; the docs explicitly note that exporting LP back to CSV "needs an additional step" | **yes — this is the gap we close** |
| Annotated-CSV input: `#datatype`, `#constant`, `#default` rows | #1, #2 | yes |
| Column datatypes: `measurement`, `tag`, `field`, `string`, `double`, `long`, `unsignedLong`, `boolean`, `dateTime`, `ignored` | #1, #2 | yes |
| Inline column-name datatype syntax `name\|datatype\|default` | #2 | yes |
| Flag-driven mapping when there are no annotations: measurement, tag columns, field columns, time column | #1 (`--header`), #3 (`--measurement/--tag-columns/--field-columns`) | yes |
| Custom delimiter (`sep=;` first line, or a flag) | #2, #3 | yes |
| Timestamp formats: RFC3339, RFC3339Nano, numeric Unix | #1, #2 | yes |
| Timestamp precision (ns/µs/ms/s) for numeric timestamps | #1 (`--precision`) | yes |
| Field typing: `1i` integer suffix, `u` unsigned, quoted strings, bare booleans | #4, #3 | yes |
| Correct escaping per position (measurement: `,` and space; tag/field keys and tag values: `,` `=` space; string field values: `"` and `\`) | #4 | yes |
| Skipping `#` comment lines and blank lines in LP input | #4 | yes |
| Sorting tag keys (InfluxDB write-performance best practice; #3 explicitly calls out that it does NOT do this) | #3 (as a stated gap) | yes — we default it ON |
| Error handling: stop on first error with line number, or skip bad rows | #2 (`SkipRowOnError`) | yes |
| Emitting annotation rows so the CSV round-trips back through `influx write` | not offered by any of them | **yes — differentiator** |
| `#timezone -0500` annotation, custom Go time layouts (`dateTime:2006-01-02`) | #2 | **no — out of model** (needs a tz database + Go layout parser; documented as a limit) |
| Locale-aware numeric separators (`double:,.` → `3.494,123`) | #2 | **no — out of model** (documented) |
| Writing directly to an InfluxDB endpoint over HTTP | #1, #3 | **no — out of model** (this is a pure, offline block; no network) |
| Reading from a file path / directory of files | #2, #3 | **no — out of model** (paste/pipe text instead) |

## UX patterns worth copying (behaviour, never copy)

- **#1** leads with a single worked example (annotated CSV in, one LP line out). We do the same
  on the page and add the reverse direction.
- **#2** reports errors as `line N: <problem>` — actionable. We match that shape and add a
  `on_error = skip` mode for dirty exports.
- **#3** shows the whole invocation in one copy-pasteable block. Our generated CLI example plus
  the `[[example]]` chips cover that.
- None of the four can be used without installing Go/Python; all four are CLI-only with no
  browser surface. A paste-in, no-upload, both-directions page is the differentiator.

## Decisions taken into the build

- Both directions, with `direction = auto` detecting annotated CSV / a CSV header row vs. LP.
- Two CSV shapes for LP → CSV: `wide` (one row per point, union of tag/field columns — the
  spreadsheet-inspection shape) and `long` (one row per field value — the tidy/normalized shape).
- `emit_annotations` writes the `#datatype …` row with per-column types inferred from the data,
  so LP → CSV → `influx write` round-trips.
- Tag keys sorted by default (the best practice #3 flags as missing), with an opt-out.
- Every out-of-model row above is stated on the page under limits, not silently dropped.

No competitor copy, wording, branding or trademark was reproduced; only format/grammar facts
from the normative InfluxData syntax reference were used.

## Sources

- [Write CSV data to InfluxDB | InfluxDB OSS v2](https://docs.influxdata.com/influxdb/v2/write-data/developer-tools/csv/)
- [Line protocol | InfluxDB OSS v2 reference](https://docs.influxdata.com/influxdb/v2/reference/syntax/line-protocol/)
- [csv2lp Go package](https://pkg.go.dev/github.com/influxdata/influxdb/v2/pkg/csv2lp)
- [qn7o/csv2influx](https://github.com/qn7o/csv2influx)
