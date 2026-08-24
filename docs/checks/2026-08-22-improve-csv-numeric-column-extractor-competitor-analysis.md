# csv-numeric-column-extractor — competitor analysis (2026-08-22)

## Scope

The tool scans pasted CSV/TSV-style tabular text, detects columns whose values are numeric, and returns those columns as typed arrays, row objects, CSV, or a names-only list. It is not a full spreadsheet, charting package, SQL engine, or schema inference service; the goal is a small local extraction step that explains both included and rejected columns.

## Sources reviewed

- DataFrame workflows in Python/pandas (`select_dtypes`, `to_numeric`, `read_csv`) — the common programmable answer for numeric-column extraction.
- R/tidyverse workflows (`readr` type guessing and `dplyr::select(where(is.numeric))`) — similar capability but requires a local runtime.
- CSVKit / command-line recipes (`csvcut`, `csvstat`, `xsv select`, shell filters) — strong delimiter handling, but numeric detection usually takes multiple commands.
- Browser CSV viewers/converters that infer types for preview/export — useful UX patterns: delimiter choice, header-row switch, JSON/CSV output, and visible rejected columns.
- Spreadsheet import wizards — table-stakes controls around delimiter, header row, blanks/nulls, and accounting-style number formatting.

## Table-stakes capabilities

| Capability | In model? | Decision |
| --- | --- | --- |
| Parse CSV and TSV text | yes | Minimal RFC-4180 reader covers comma plus tab/semicolon/pipe. |
| Auto-detect delimiter | yes | `delimiter=auto` scores comma/tab/semicolon/pipe by consistent column counts. |
| Header detection and forced header modes | yes | `header=auto|present|absent`; absent names columns `column_1`, etc. |
| Return typed numeric arrays by column | yes | Default `columns` JSON includes name, original index, integer/float type, counts, missing count, numeric ratio, and values. |
| Explain non-numeric columns | yes | `skipped` includes the rejection reason, first bad example, index, and ratio. |
| Output records / CSV / names list | yes | `output=records|csv|names` covers scripting, copy/paste, and quick column-list use cases. |
| Blanks and null tokens | yes | `allow_blanks` default-on and `null_tokens` default to common markers. |
| Numeric-ratio threshold | yes | `min_numeric_ratio` from 0.1 to 1.0 supports mostly-numeric sensor/export columns. |
| Accounting-style normalisation | yes | `normalize` accepts currency symbols, grouped thousands, percent signs, parentheses negatives, and trailing-minus values. |
| Avoid corrupting identifiers | yes | Zero-padded codes such as `007` and `01234` are deliberately skipped. |
| RFC-4180 quoted delimiters/newlines | yes | Quoted fields with delimiters, doubled quotes, and newlines are parsed. |
| Type inference for dates/booleans/categories | out-of-model | Adjacent schema inference; this tool extracts numeric columns only. |
| Statistical summaries / charts | out-of-model | Use downstream stats/chart tools after extraction. |
| File upload / multi-file batch | out-of-model | Current gizza pure page model uses pasted text; no filesystem traversal. |
| Locale-specific decimal comma | listed, not built | Ambiguous with comma-delimited CSV; users can pre-normalize or force semicolon+plain decimals. |

## UX / parameter decisions

- Selects for delimiter, header, and output mirror spreadsheet import controls and CLI flags.
- A slider is used for `min_numeric_ratio` because competitors frame this as a tolerance threshold.
- Checkbox defaults match practical data work: blanks are allowed, and accounting normalization is on.
- Preset chips cover typed arrays, accounting figures, headerless grids, and a 75% numeric tolerance.
- The page copy explicitly states that identifiers with leading zeroes are skipped to prevent lossy conversions.

## Verification implications

Advertised matrix to cover: all delimiters, all header modes, all output modes, both default-on booleans toggled off, a non-default numeric ratio, default and custom null tokens, quoted fields, zero-padded identifiers, accounting normalization, exact CLI output, page deep links, and the 1 MB cap boundary.
