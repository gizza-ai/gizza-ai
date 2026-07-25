# schema-inferrer — competitor analysis (2026-07-25)

**Tool:** Infers a JSON Schema **and** a SQL `CREATE TABLE` statement from a sample dataset
(CSV/delimited text or JSON records), with a single type-inference pass and an output selector.

## Why this is not a duplicate

No existing block produces BOTH a standard JSON Schema and a SQL `CREATE TABLE` from one dataset:

- `blocks/json-to-json-schema` — emits a JSON Schema only (no SQL); infers over arbitrary nested
  JSON using JSON's native types, not SQL-aligned types.
- `blocks/csv-to-sql` — emits `CREATE TABLE` **+ INSERT** rows (data migration); no JSON Schema.
  Its output is INSERT-focused (`create_table` is just a prefix toggle) and it never emits a schema.
- `blocks/csv-type-inferrer` — emits a *custom* per-column report ("dialect + types/counts") and/or
  typed JSON records; not a standard JSON Schema and no SQL DDL.
- `blocks/spreadsheet-to-sql` / `blocks/json-to-sql-insert` — INSERT/DDL emitters, no JSON Schema.

`schema-inferrer` is DDL/schema-only (no INSERT rows — that stays csv-to-sql's job), emits a
**standard** JSON Schema (Draft 2020-12 / Draft-07) alongside the `CREATE TABLE`, and lets the user
select which artifact(s) to get. The distinctive capability is the two canonical schema
representations from one inference pass — a data-modeling/scaffolding workflow neither parent serves.

## Competitors scanned

- JSON Utils — JSON → SQL `CREATE TABLE`, auto type inference, multiple dialects.
- CoderTools "Schema & SQL DDL Generator (CSV/JSON to Tables)" — CSV/JSON → SQL DDL + inferred
  types, multiple DBs (closest to the combined capability, but SQL-only, no JSON Schema output).
- JSONLint / DevFlow / DataFormatterPro / JsonToTable — JSON → `CREATE TABLE` (+ INSERT),
  PostgreSQL/MySQL/SQLite/SQL Server, auto-inferred types (VARCHAR/INT/DECIMAL/BOOLEAN).
- jsonschema2sql (GitHub) — the inverse direction (JSON Schema → `CREATE TABLE`).

## Table-stakes → decision (every one lands in the descriptor or is listed out-of-model)

| Capability | Decision |
| --- | --- |
| Accept CSV/delimited **or** JSON (object / array of objects), auto-detect | in-model — `data` + `format` (auto/csv/json) |
| Auto delimiter + header detection for CSV | in-model — `delimiter`, `has_header` |
| Per-column type inference (int/float/bool/date/datetime/text) | in-model — core inference, SQL-aligned |
| Multiple SQL dialects (mysql/postgres/sqlite/mssql/ansi) | in-model — `dialect` |
| Table name | in-model — `table` |
| `NOT NULL` for columns with no nulls | in-model — `not_null` (also drives JSON Schema `required`) |
| PRIMARY KEY column | in-model — `primary_key` |
| Standard JSON Schema output (Draft 2020-12 / Draft-07) | in-model — `output`, `draft` |
| String `format` hints (email/uri/date-time/date/uuid/ipv4) in JSON Schema | in-model — `detect_formats` |
| Choose which artifact to emit (both / schema / SQL) | in-model — `output` (the differentiator) |
| INSERT-statement generation | **out-of-model here** — already `blocks/csv-to-sql`; kept schema-only by design |
| FOREIGN KEY / relationship detection, nested-object → multiple normalized tables | out-of-model — needs multi-table normalization / a relational model beyond one flat table |
| VARCHAR length sized to longest value | out-of-model here — toolkit maps text → the dialect's TEXT/VARCHAR(255) consistently (see csv-to-sql) |

## UX patterns adopted

- `<select>` controls for `format`, `delimiter`, `output`, `dialect`, `draft` (enums).
- Checkboxes for `has_header`, `not_null`, `detect_formats` (booleans with sensible defaults).
- `[[example]]` preset chips (JSON-array records; CSV; SQL-only Postgres) — competitors ship presets.
- Placeholders on `data`, `table`, `primary_key`.

No competitor copy, branding, or trademarks were reproduced.
