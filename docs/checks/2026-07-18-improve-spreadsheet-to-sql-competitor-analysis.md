# spreadsheet-to-sql — competitor analysis (2026-07-18)

Tool function: read an uploaded Excel/spreadsheet file (`.xlsx`/`.xlsm`/`.xls`/`.ods`) and
emit `CREATE TABLE` + `INSERT` SQL statements, one table per worksheet.

## Scan (WebSearch: "convert Excel xlsx to SQL CREATE TABLE INSERT statements online tool")

Skimmed the top real competitor tools (all browser/no-account, paraphrased — no copy reused):

1. **TableConvert — Excel to SQL** (tableconvert.com/excel-to-sql). Paste-or-upload `.xlsx`/`.xls`,
   live preview, custom table name, generates `CREATE TABLE` + `INSERT`. Column-type controls.
2. **AllFileTools — Excel to SQL** (allfiletools.com/excel-to-sql). `CREATE TABLE` + batch `INSERT`
   with an explicit **dialect** target: MySQL, PostgreSQL, SQL Server. In-browser.
3. **A.Tools — Excel to SQL** (a.tools/excel-to-sql). Dialects MySQL/PostgreSQL/SQLite/SQL Server,
   fully client-side ("files never uploaded"). `CREATE TABLE` + `INSERT` per table.
4. **ExcelTool.io — Excel to SQL** (exceltool.io/excel-to-sql). PostgreSQL/MySQL/SQLite,
   `CREATE TABLE` + `INSERT`, no upload.
5. **wtools.io — Excel to SQL**. Custom table name, MySQL insert variants.

## Table-stakes parameters (each tagged in-model / out-of-model)

| Capability | Competitors | Fit | Decision |
|---|---|---|---|
| SQL **dialect** (MySQL / PostgreSQL / SQLite / SQL Server) — drives identifier quoting + value escaping | 2,3,4 | in-model | **built** — `dialect` enum, default `mysql` |
| **Custom table name** | 1,5 | in-model | **built** — `table` (base name; sheet-suffixed when >1 sheet) |
| **CREATE TABLE** toggle (schema + inserts, or inserts only) | most | in-model | **built** — `create_table` bool, default true |
| **First row = column names** vs generated `col1..colN` | most | in-model | **built** — `header_row` bool, default true |
| **Column type inference** (INT / DECIMAL / BOOLEAN / text) vs all-text | 1,2,3 | in-model | **built** — `infer_types` bool, default true |
| **Batch multi-row INSERT** vs one INSERT per row | 2,5 | in-model | **built** — `batch_insert` bool, default true |
| **Which sheet(s)** — all vs one | most (multi-sheet) | in-model | **built** — `sheet` (name/index; empty = all sheets, one table each) |
| NULL for empty cells | most | in-model | **built** — empty cells emit `NULL` |
| Live preview / paste-table UI | 1 | out-of-model | listed — this is a chat + CLI block (binary file input, no standalone page, same as `xlsx-to-csv`) |
| Direct DB connection / run-against-server | (paid tiers) | out-of-model | listed — gizza is browser-local, no backend, no accounts |
| Download `.sql` file button | 3,4 | in-model (chat/UI) | **built** — envelope emits a `data:application/sql` download URL + `.sql` filename |

## Surface note

Binary spreadsheet bytes are neither a pure-text page input nor an ffmpeg media transform, so — as
with the sibling `xlsx-to-csv` — this ships as a **no-page block**: verifiable surfaces are the
descriptor/chat schema (drift-guarded unit tests) and the `gizza` CLI. No page / no Playwright.

Copy/branding: none reused. All descriptor text, FAQ-style `.describe()` copy, and SQL formatting
are original.
