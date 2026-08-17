# csv-quote-normalizer — competitor analysis (2026-08-17)

Scan run before implementation. Findings are paraphrased from public documentation and tool pages; no competitor copy, branding, or trademarks are reused.

## Duplicate / viability check

| Existing block | Overlap | Verdict |
| --- | --- | --- |
| `csv-change-delimiter` | Re-emits parsed CSV with another delimiter | Not a duplicate: it uses a strict RFC 4180 parser and cannot read backslash-escaped quotes, single-quoted fields, curly quotes, padding around quotes, or unclosed quotes. |
| `csv-cleaner` | General trim/drop/fill/dedupe cleanup | Not a duplicate: it changes rows/cells, not quoting dialects or malformed quote repair. |
| `csv-whitespace-normalizer` | Cell whitespace repair | Not a duplicate: intentionally leaves quoting/escaping alone. |
| `csv-structure-validator` | Reports malformed CSV | Not a duplicate: diagnostic surface only; it does not rewrite a repaired CSV. |
| `csv-null-standardizer` | Rewrites token values and quote style | Not a duplicate: handles null tokens, not tolerant quote parsing. |

The distinct capability is tolerant quote/dialect parsing followed by strict re-emission: repair the rows a strict reader mis-splits, then produce one consistent dialect.

## Competitor profiles

### 1. Python `csv` module / dialect options
- Table stakes: delimiter, quote character, quote policy, escape character, doublequote flag, lineterminator, QUOTE_MINIMAL / QUOTE_ALL / QUOTE_NONNUMERIC / QUOTE_NONE.
- Constraint: strict parsing once the dialect is chosen; it does not auto-repair smart quotes or mixed conventions.
- Decision: ship the same core knobs as first-class params (`quote_style`, `escape`, `output_quote`, `line_ending`, `delimiter`) but add tolerant reading.

### 2. pandas `read_csv` / `to_csv`
- Table stakes: read and write quoting, doublequote, escapechar, sep, lineterminator, quotechar, malformed-line policy.
- Constraint: users need Python installed and must know the dialect flags; malformed quotes are often skipped or errored rather than repaired.
- Decision: expose browser-local presets and an audit report instead of requiring code.

### 3. OpenCSV / Java CSVParser + CSVWriter
- Table stakes: separator, quote char, escape char, strict vs non-strict quotes, ignore leading whitespace, writer quote modes.
- Useful signal: leading whitespace before an opening quote is common enough to be configurable.
- Decision: the parser treats padding before/after quotes as a repair, while the writer emits a clean dialect.

### 4. CSVLint / online CSV validators
- Table stakes: detect ragged rows, bad quote state, unexpected delimiter/quote use, and report line numbers.
- Constraint: validation tells you where the file is broken but does not produce a fixed file.
- Decision: `output=report` lists detected dialect, row/field counts, ragged rows and every repair with line numbers; `output=csv` returns the fixed file.

### 5. Spreadsheet import/export dialogs
- Table stakes: delimiter selection, quote handling, line endings and compatibility with Excel/Sheets-style CSV.
- Constraint: they hide repairs, can retype values, and do not explain what was fixed.
- Decision: no type inference or cell rewriting; quote normalization only.

## Table stakes → decisions

| Table stake | Decision |
| --- | --- |
| Input delimiter detection and override | `delimiter=auto` plus comma/tab/semicolon/pipe/space/single-character support. |
| Output delimiter conversion | `output_delimiter=same` or explicit delimiter. |
| Input quote character | `input_quote=auto|double|single|none`; auto avoids treating apostrophes in text as field quotes. |
| Quote policy | `quote_style=minimal|always|non_numeric|never`. |
| Quote escaping | `escape=doubled|backslash`. |
| Output quote character | `output_quote=double|single`. |
| Backslash-escaped input | `backslash_escapes` boolean, default on. |
| Smart/curly quote input | `smart_quotes` boolean, default on. |
| Line ending normalization | `line_ending=lf|crlf`, including embedded newlines. |
| Auditability | `output=report` with repairs and line numbers. |

## Out of model / deliberately not built

- Encoding detection and byte transcoding: owned by text-encoding tools.
- Header renaming, row padding, trimming values, null-token cleanup, duplicate removal: existing CSV-cleaning blocks own those transformations.
- Streaming very large files: this browser-local pure tool uses a 5 MB pasted-text cap.
- Uploading, accounts, hosted storage, paste-service export: not part of the local gizza model.

## Preset chips shipped

1. Repair mixed quoting with defaults.
2. See what changed as a report.
3. Quote every field with CRLF line endings.
4. Backslash escaping for MySQL-style destinations.
5. Single-quoted CSV to TSV.
