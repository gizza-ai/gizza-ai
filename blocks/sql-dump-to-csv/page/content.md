## What this tool does

Paste a SQL dump — a `.sql` file from `mysqldump`, `pg_dump`, SQLite `.dump`, or
any migration/export — and this tool pulls the actual row data out of its
`INSERT` statements and hands it back as CSV. When the dump touches more than one
table, each gets its own labelled CSV section. Everything runs locally in your
browser: nothing is uploaded, it works offline, and there is no sign-up.

## How columns are named

The header row (when enabled) is resolved in this order:

1. **The INSERT column list** — `INSERT INTO users (id, name) VALUES …` → `id,name`.
2. **A matching `CREATE TABLE`** in the same dump, if the INSERT has no column list.
3. **Generated `col1..colN`** as a last resort, when neither is available.

Comments (`-- …`, `# …`, `/* … */`) and every non-INSERT statement are used for
context or skipped. Multiple INSERTs into the same table are concatenated in
order.

## Options

| Option | What it does |
| --- | --- |
| **Only this table** | Export just one table by name (case-insensitive). Leave blank to export them all. Selecting one table also drops the `### TABLE:` section marker, giving you clean single-table CSV. |
| **Delimiter** | `comma` (CSV, default), `tab` (TSV), `semicolon`, or `pipe`. |
| **Header row** | On by default — turn it off to emit rows only. |
| **Text for SQL NULL** | What to write for a `NULL` cell. Empty by default (an empty field); set it to `NULL` or `\N` to make nulls explicit. |
| **Quoting** | `minimal` (default) quotes a field only when it contains the delimiter, a `"`, or a newline; `all` wraps every field in double quotes. |
| **UTF-8 BOM** | Prepend a byte-order mark so Excel opens the file as UTF-8. |

## Example

Input:

```sql
INSERT INTO users (id, name, email) VALUES
  (1, 'Alice', 'alice@example.com'),
  (2, 'Bob', NULL);
```

Output (default settings):

```csv
id,name,email
1,Alice,alice@example.com
2,Bob,
```

With **multiple tables**, each is prefixed with a `### TABLE: <name>` line and
separated by a blank line, so you can split the result or copy one section:

```csv
### TABLE: authors
id,name
1,Ada

### TABLE: books
id,title,author_id
10,On Computation,1
```

## Limits & edge cases

- Values are extracted **verbatim** — numbers, `TRUE`/`FALSE`, and hex/blob
  literals (`0x…`, `X'…'`) are written as they appear; no type conversion.
- String literals honour SQL-standard doubled quotes (`''`) **and** MySQL
  backslash escapes (`\'`, `\n`, `\t`). A PostgreSQL dump that stores a literal
  backslash before a quote is the one ambiguous case.
- Only the `INSERT … VALUES (…)` form carries rows. `INSERT … SET …` and
  `INSERT … SELECT …` have no literal tuples and are skipped.
- Schema-qualified names (`db.users`) are grouped by the final component (`users`).
- Line endings are LF. If a field spans lines it is quoted per RFC 4180.
- Processing is in-memory in your browser; very large dumps (hundreds of MB) may
  be slow or hit the tab's memory limit.

## FAQ

<details>
<summary>Do I need the <code>CREATE TABLE</code> statements too?</summary>

No. If your `INSERT` statements already list their columns
(`INSERT INTO t (a, b) VALUES …`), that is enough for the header row. Including
the `CREATE TABLE` only helps when the INSERTs omit the column list — then the
column names come from the schema instead of generic `col1, col2, …`.

</details>

<details>
<summary>How are multiple tables returned?</summary>

Each table is emitted as its own CSV block, preceded by a `### TABLE: name`
marker and separated by a blank line. To get one table on its own with no
marker, put its name in the **Only this table** box.

</details>

<details>
<summary>How are SQL <code>NULL</code> values handled?</summary>

By default a `NULL` becomes an empty field. If you would rather see it spelled
out — for round-tripping into another database, say — set **Text for SQL NULL**
to `NULL`, `\N`, or whatever your target expects.

</details>

<details>
<summary>Which SQL dialects work?</summary>

The row extractor is dialect-agnostic for the common cases: MySQL/MariaDB,
PostgreSQL, SQLite, and SQL Server dumps all use the same `INSERT INTO … VALUES`
shape. It understands backtick, double-quote, and `[bracket]` quoted identifiers,
`N'…'` national strings, and multi-row VALUES lists.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The parsing happens entirely in your browser via WebAssembly. Your dump
never leaves your device, and the page keeps working offline once loaded.

</details>
