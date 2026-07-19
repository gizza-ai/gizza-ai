# csv-structure-validator — competitor analysis (2026-07-20)

Tool function: lint the raw text of a CSV for structural faults — ragged rows (inconsistent
column counts), unbalanced/unclosed quotes, stray quote characters, blank/empty rows, header
problems, whitespace, mixed line endings — and report every issue with a line number and a
severity, without modifying the data.

Distinctness check vs existing blocks (2026-07-20): `blocks/csv-find-incomplete-rows` operates on
PARSED records via the lenient `csv` crate (flexible mode), so it can flag wrong field counts and
blank required cells but structurally cannot report unclosed quotes, stray quotes, blank rows,
header duplicates, whitespace, or line-ending problems — the lenient parser swallows them. This
tool lints the RAW text with its own RFC 4180-aware scanner, serving the distinct "is my CSV
well-formed / why does my import fail" intent (csvlint-class tools). Not a duplicate.

## Competitors scanned (top 3 reachable, paraphrased — no copy/branding reproduced)

(martys.app/csv-validator returned 403 and thetexttool.com is JS-only with no fetchable content —
both replaced per procedure.)

1. **Online Tools CSV validator (onlinetools.com/csv/validate-csv)** — RFC 4180 validator with
   configurable delimiter char, quote char, and comment char; allowance checkboxes (allow
   comments / empty lines / empty values / incomplete data / leading spaces / trailing spaces);
   a configurable error limit (first 10 errors by default); output is a pass/fail badge plus an
   itemized list of error type + description + row number. Ships interactive examples with
   pre-configured settings. Detects unterminated quoted fields, missing quotes, empty lines,
   whitespace issues.

2. **csvlint.io** — the reference CSV linter (Open Data Institute lineage). Dialect options:
   delimiter, quote character, header present. Its published check families: structural errors
   (ragged rows, blank rows, unclosed quotes, stray quotes, whitespace, inconsistent line
   breaks) and schema/metadata warnings (empty column names, duplicate column names, title
   rows). Reports errors vs warnings separately with row locations.

3. **CSV Tools validator (csvtools.com/csv-validator)** — auto-detects the column separator
   (manual override available), configurable quote char + optional comment symbol, "first row
   contains column names" toggle. Checks ragged rows, empty or duplicate header names,
   completely empty rows, parser warnings. Report lists issues by type with row numbers and
   descriptions; sample-data "Example" button; explicitly report-only (companion tools do the
   fixing). Fully client-side.

## Table-stakes (each tagged in-model / out-of-model)

| Capability | Decision |
| --- | --- |
| Ragged-row / inconsistent-column-count detection with line numbers | IN — `ragged_row` error, expected width from header/first row |
| Unclosed/unterminated quoted field detection | IN — `unclosed_quote` error at the line where the quote opened |
| Stray quote detection (bare quote in unquoted field; text after a closing quote) | IN — `stray_quote` error |
| Blank line + all-empty-row detection | IN — `blank_row` / `empty_row` warnings |
| Duplicate header names | IN — `duplicate_header` warning (header=true) |
| Empty header names | IN — `empty_header` warning (header=true) |
| Leading/trailing whitespace around fields/quotes | IN — `whitespace` warning, aggregated per row |
| Inconsistent line endings (CRLF vs LF vs CR) | IN — `mixed_line_endings` warning, once, at first divergence |
| Delimiter auto-detection + manual override | IN — `delimiter` enum: auto (default) / comma / tab / semicolon / pipe |
| Quote character configuration | IN — `quote` enum: double (default) / single / none |
| Comment character (skip comment lines) | IN — `comment` param, single char, off by default |
| Header-row toggle | IN — `header` boolean, default true |
| Error limit / capped report | IN — `max_issues` integer (1–1000, default 50); full counts always reported, list truncation flagged |
| Errors vs warnings severity split, pass/fail verdict | IN — `valid` = zero errors; per-issue severity |
| Sample/example presets | IN — `[[example]]` chips (broken CSV, clean CSV, semicolon auto-detect) |
| File upload / fetch-by-URL input | OUT — pure page takes pasted text (the platform's pure-tool page pattern); CLI/chat take the same `data` string. No file-input page runtime for pure tools. |
| Per-column type/consistency stats (inconsistent_values) | OUT — data-quality profiling, a different tool class; `csv-stats` and `csv-type-inferrer` already cover column typing/stats. |
| Schema validation (CSVW / column types against a user schema) | OUT — csvlint's schema mode needs a schema language; distinct feature class. |
| Encoding detection (invalid UTF-8 bytes) | OUT — inputs arrive as text (already-decoded strings) on every surface; a bytes-level encoding checker would need file input. BOM is stripped and documented. |
| Auto-fixing the CSV | OUT by design (report-only, like competitors) — `csv-cleaner` / `csv-change-delimiter` are the fixing companions and are cross-linked in copy. |

## UX controls

- `data` — multiline textarea with a realistic broken-CSV placeholder.
- `delimiter` — `<select>` via enum + `[input.labels]` (Auto-detect / Comma / Tab / Semicolon / Pipe).
- `quote` — `<select>` via enum + labels (Double / Single / None).
- `header` — checkbox, checked by default.
- `comment` — short text field, placeholder `#`.
- `max_issues` — number field, default 50.
- Three `[[example]]` preset chips (competitors ship sample buttons): broken CSV, clean CSV,
  semicolon CSV with auto-detect.

## Design decisions

- Hand-rolled RFC 4180-aware scanner over the raw text (NOT the `csv` crate) — lenient parsers
  hide exactly the faults this tool exists to report. Quoted fields may span lines (line
  numbers stay physical); `""` escapes are honored; quote=none disables quote handling.
- Noise suppression: a row with an unclosed-quote error skips the ragged-row check (the swallowed
  text makes its field count meaningless); whitespace findings aggregate to one issue per row.
- `valid` is true iff there are zero ERRORS — warnings alone don't fail the file (this stands in
  for onlinetools' per-check "allow …" toggles without six extra booleans).
- Chat/CLI return the structured JSON report (summary counts + capped issue list); the page
  renders a plain-text report of the same content.
- Leading U+FEFF (BOM) is stripped before scanning and documented in the FAQ.
