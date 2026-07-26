# regex-column-validate — competitor analysis (2026-07-26)

**Tool:** flag CSV column values that do not match a supplied regex pattern. Report-only
data-quality checker. Pure-Rust (`csv` + `regex`), chat + CLI + page.

## Competitors scanned

1. **CSV Tools Online — Regex Tester** (csvtoolsonline.com/tools/regex-tester) — upload/paste
   CSV, pick a target column, enter a regex + flags (`g i m s u y`), test across all rows,
   extract capture groups, export matched rows to CSV. Match/extract oriented (not a
   pass/fail column validator); no explicit "show only non-matching" view.
2. **CSV Blueprint** (GitHub Action / PHP) — schema-driven column rules, incl. a `regex` rule
   (PCRE) plus `is_lowercase/uppercase/capitalize`. Reports failures with row number, column
   name, and rule name; renders in PR diffs. Batch/CI oriented, YAML schema.
3. **Teleport CSV Validator** (goteleport.com) — browser CSV validator; regex used to validate
   formats (email/phone). General validation, syntax-first.
4. **Free Tools Corner — Test Regex (Line-by-Line mode)** — paste a column, line-by-line mode
   highlights which rows fail a required format. Single-column, no CSV structure/header logic.
5. **convertcsv CSV Validator** (convertcsv.com/csv-validator.html) — validate ZIP/phone/NPI and
   custom patterns before import, with a live count of invalid cells and a filter to show only
   invalid rows.

## Table-stakes → decisions

| Capability | In model? | Decision |
|---|---|---|
| Pick target column (header name or index) | yes | `column` param (name when `has_header`, else 0-based index; a numeric value is always an index) |
| Supply a regex pattern | yes | `pattern` param (required) |
| Case-insensitive matching | yes | `ignore_case` boolean (maps to regex `(?i)`) |
| Other flags (multiline/dotall/etc.) | yes | Documented: embed inline flags in the pattern itself (`(?i)`, `(?s)`, `(?m)`) — Rust `regex` supports them, so all flags are available without extra params |
| Full-cell match vs match-anywhere | yes | `full_match` boolean (default true): anchor the pattern to the whole cell (`\A(?:…)\z`); off = match if the pattern is found anywhere |
| Show non-matching vs matching rows (invert) | yes | `report` enum `non-matching` (default) / `matching` (flag values that DO match — find forbidden values) |
| Per-row failure report (row, line, value, reason) | yes | `invalid_rows` with `line`, `row`, `value`, `message`; capped by `max_issues` |
| Live invalid count | yes | summary reports full `invalid` count even when the listed rows are truncated |
| Header / headerless CSV | yes | `has_header` boolean |
| Delimiter (comma/tab/semicolon/pipe) + auto-detect | yes | `delimiter` enum incl. `auto` |
| Blank-cell handling | yes | `allow_blank` boolean (default true) |
| Text + machine-readable output | yes | `output` enum `text` / `json` |

## Out of model (listed, not built)

- **Capture-group extraction / export matched substrings to CSV** — that is extraction, not
  validation; the existing `regex-extract` tool covers it. This tool is report-only.
- **Auto-fix / transform non-conforming cells** (UPPER/lower/trim) — mutation is out of scope
  for a report-only validator; other CSV tools (`csv-cleaner`) handle cleanup.
- **Multi-column schema / YAML rule files** (CSV Blueprint) — this tool validates one column
  against one pattern per run; general multi-rule validation is covered by `data-validator`.
- **File upload > a few MB / gigabyte streaming** — pages run in-browser wasm on pasted text.

## Not a duplicate

`data-validator` supports a generic `field:regex=…` rule among many, and `date-column-validate` /
`csv-column-type-validator` are focused single-column checkers that intentionally coexist with it.
`regex-column-validate` is the focused single-regex column checker in that same family (full-match
anchoring, invert/report mode, per-row reasons) — a distinct, discoverable tool, not a redundant one.
