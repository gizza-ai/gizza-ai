# csv-to-sql — competitor analysis (2026-07-23)

Tool: `csv-to-sql` — "Generate SQL CREATE TABLE and INSERT statements from a
CSV/JSON table with inferred column types." Pure-compute (runs in the WASM
sandbox / browser; no upload).

## Distinctness vs existing gizza blocks

- **json-to-sql-insert** — takes only pre-typed JSON. It cannot ingest raw CSV
  text and does no string→type sniffing (it trusts JSON's native types), so it
  never infers DATE / TIMESTAMP. `csv-to-sql` accepts CSV *text* (delimiter
  sniffing, header detection) as its primary input and infers types from string
  cells, including date/datetime → SQL `DATE`/`TIMESTAMP`. JSON is accepted as a
  secondary convenience input.
- **spreadsheet-to-sql** — reads *binary* workbook files (`.xlsx`/`.ods` via
  calamine); it does not parse plain CSV text pasted into a field.
- **csv-type-inferrer** — infers column types but emits a JSON schema/records
  report, never SQL. `csv-to-sql` reuses the same proven CSV parser + type
  sniffer and turns the result into DDL/DML.

Conclusion: distinct superset (CSV-text → SQL DDL+DML with date-aware type
inference). Buildable.

## Competitors surveyed (paraphrased; no copied copy/branding)

1. **Chat2DB CSV→SQL** — dialect select (MySQL/Postgres/SQL Server), automatic
   per-column data-type detection, first row = headers, batch (multi-row) insert,
   client-side.
2. **CodeShack CSV→SQL** — choose statement kind (INSERT / UPDATE / CREATE
   TABLE), dialect select, "generate CREATE TABLE from the header row and guess
   the best type per column", table-name field.
3. **TableConvert CSV→SQL** — auto-recognizes the delimiter (comma/tab/semicolon/
   pipe), auto type + encoding detection, handles large files, table name.
4. **CodeTidy / Tools.beer / AI2SQL** (secondary) — type inference toggle
   (INTEGER/REAL/BOOLEAN/TEXT), multi-dialect incl. SQLite/Oracle, CREATE TABLE +
   INSERT together, all browser-local.

## Table-stakes matrix (→ fit decision)

| Capability | Competitors | Decision |
|---|---|---|
| SQL dialect select | all | **In** — `dialect` enum mysql/postgres/sqlite/mssql/ansi |
| Emit CREATE TABLE | all | **In** — `create_table` (on by default; this tool's headline) |
| Emit INSERTs | all | **In** — always emitted |
| Per-column type inference | all | **In** — int/float/bool/date/datetime/text → dialect SQL types |
| First row = header | Chat2DB, CodeShack | **In** — `has_header` (default true) |
| Delimiter auto/select | TableConvert | **In** — `delimiter` enum auto/comma/tab/semicolon/pipe |
| Table name | all | **In** — `table` |
| Batch (multi-row) vs per-row INSERT | Chat2DB, AI2SQL | **In** — `multi_row` (default true) |
| Accept JSON as well as CSV | (gizza superset) | **In** — `format` enum auto/csv/json |
| NULL handling for blanks | AI2SQL | **In** — `null_handling` null/default/empty-string |
| Prepared-statement/placeholder output | (advanced) | **In** — `values` literal/placeholder |
| Quote identifiers | (advanced) | **In** — `quote_identifiers` |
| DROP TABLE IF EXISTS | CodeShack | **In** — `drop_table` |
| Primary key column | CodeShack | **In** — `primary_key` |
| Oracle dialect | Tools.beer/AI2SQL | **Out (deferred)** — ANSI covers generic; Oracle's `NUMBER`/`VARCHAR2`/date literals differ enough to defer; ansi is the portable fallback |
| UPDATE statement generation | CodeShack | **Out of scope** — this tool focuses on CREATE+INSERT (matches json-to-sql-insert scope) |
| Column constraints / indexes / FKs | some | **Out of scope** — types only, no lengths/indexes |
| Large-file streaming upload | TableConvert | **Out** — paste/text input; bounded by device memory (stated in page limits) |

## UX controls competitors ship (→ page)

- Dialect / statement-kind / delimiter as selects → `Param::enumv` selects.
- Header + batch + create-table as checkboxes → `Param::boolean`.
- Preset one-click examples → `[[example]]` chips (CSV→MySQL, Postgres+dates,
  JSON input, per-row placeholders).
- All client-side/no-upload messaging → page hero + FAQ.

No competitor copy, branding, or trademarks reproduced.
