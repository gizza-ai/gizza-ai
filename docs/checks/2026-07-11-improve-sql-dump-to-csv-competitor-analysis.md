# Competitor analysis — sql-dump-to-csv (2026-07-11)

Tool function: extract the rows from `INSERT` statements in a SQL dump into CSV,
one CSV per table. Scan performed before implementation to fix table-stakes
params/defaults/UX. **Paraphrased only — no competitor copy/branding reproduced.**

## Competitors scanned

1. **TableConvert (sql-to-csv)** — paste INSERT statements or upload a `.sql`
   file; parses several SQL dialects. Output config: value delimiter choice
   (comma / tab / semicolon / newline / colon / pipe / slash / hash), a
   "double-quote wrap" toggle (quote every value), a UTF-8 BOM toggle for Excel,
   plus row prefix/suffix. Live preview; copy or download; ~10 MB cap; no
   sign-up, browser-local.
2. **ConvertCSV (sql-to-csv)** — requires a `CREATE TABLE` + `INSERT` statements
   and a trailing `SELECT`; column headers derive from the CREATE TABLE.
   Delimiter: comma / semicolon / colon / pipe / tab / custom. Options: suppress
   in-field line breaks, force-quote all values, CRLF vs LF line endings.
   SQLite syntax only; multiple SELECTs → multiple result sets.
3. **CSVtoAny / DevToolLab / CodeShack (sql-to-csv)** — paste INSERT statements,
   parse them, produce a clean downloadable CSV; marketed as extracting data
   from SQL dumps / migration files / DB exports. Browser-local, no account.
4. **RebaseData (convert-sql / mysql to csv)** — upload one `.sql` file, get back
   a ZIP archive with one `.csv` per table in the dump. This is the canonical
   "one CSV per table" model (server-side, upload-based).

## Table-stakes params (→ our decision)

| Param | Competitors | Our tool | In/out of model |
| --- | --- | --- | --- |
| Delimiter (comma/tab/semicolon/pipe) | all | `delimiter` enum (comma default) | in-model ✅ |
| Header row (column names) | all | `header` bool (default true) | in-model ✅ |
| Quote-all vs minimal | TableConvert, ConvertCSV | `quote` enum minimal/all (minimal default) | in-model ✅ |
| UTF-8 BOM for Excel | TableConvert | `bom` bool (default false) | in-model ✅ |
| NULL rendering | (implicit) | `null_value` string (default empty) | in-model ✅ |
| One CSV per table | RebaseData | multi-table output with `### TABLE:` sections; `table` filter for a single table | in-model ✅ |
| Column names from INSERT list or CREATE TABLE | ConvertCSV (CREATE), CodeShack (INSERT list) | both: explicit INSERT column list wins, else CREATE TABLE, else `col1..colN` | in-model ✅ |
| Upload `.sql` file | most | paste on page / `sql` arg on CLI+chat | in-model (paste is the browser-local equivalent) |
| Live preview / copy / download | all | page auto-runs + Copy result + Download link (shared chrome) | in-model ✅ |

## Out-of-model / considered-not-built

- **ZIP of one-file-per-table** (RebaseData) — the page renders a single text
  output; we emit labeled `### TABLE:` sections instead, and the `table` filter
  yields one clean CSV. A real multi-file ZIP download isn't part of the text
  page model. Considered, not built.
- **Row prefix/suffix, custom row delimiter, colon/slash/hash delimiters,
  transpose/dedupe/case/regex post-processing** (TableConvert) — schema bloat for
  a niche; gizza has dedicated csv-* tools (csv-change-delimiter, csv-dedupe,
  csv-transpose…) that chain after this one. Rejected to keep the schema lean.
- **CRLF line endings** — we emit LF; downstream tools/Excel accept it. Minor;
  not added.

## UX patterns adopted

- Multi-line paste box for the dump (`multiline = true`).
- `delimiter` and `quote` render as `<select>` (enum), `header`/`bom` as
  checkboxes — the right control per data type.
- `[[example]]` preset chips seeded from a real worked example so the page shows
  output before the user types.
