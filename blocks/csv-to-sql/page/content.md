## What this tool does

Paste a **CSV, delimited text, or JSON table** and generate runnable SQL: an
optional `DROP TABLE IF EXISTS`, a `CREATE TABLE` with inferred column types, and
`INSERT` statements for every row. It runs entirely in your browser, so the table
data is not uploaded.

Use it when you need a quick import script for seed data, a database migration
snippet, a fixture file, or a handoff from spreadsheet-style data to SQL.

## Input formats

- **CSV / delimited text** — header row by default, with comma, semicolon, tab,
  or pipe delimiters. Leave delimiter on **auto** for the most common cases.
- **JSON object** — one object becomes one row.
- **JSON array of objects** — each object becomes a row; the column list is the
  union of keys in first-seen order.

Blank CSV cells, JSON `null`, and missing JSON keys become `NULL` by default.
Set **Blank/null cells** to `default` or `empty-string` when you need a different
literal.

## Dialects and value modes

Choose **MySQL**, **PostgreSQL**, **SQLite**, **SQL Server**, or **ANSI SQL**.
The dialect controls identifier quoting, boolean literals, placeholder syntax,
and the `CREATE TABLE` type names. Inferred categories include integer, float,
boolean, date, datetime, and text. Zero-padded codes such as ZIP/postal codes
stay as text so leading zeroes are not lost.

The default **literal** mode escapes values and writes complete SQL. Choose
**placeholder** to produce a prepared-statement shape (`?`, `$1`, `@p1`, etc.)
with a trailing `-- params:` comment showing the bound values.

## Example

Input CSV:

```csv
id,name,score,active,joined
1,Alice,9.5,true,2024-01-02
2,Bob,7,false,2024-02-10
```

With table `users`, dialect `mysql`, and primary key `id`, the output starts:

```sql
CREATE TABLE `users` (
  `id` INT,
  `name` TEXT,
  `score` DOUBLE,
  `active` TINYINT(1),
  `joined` DATE,
  PRIMARY KEY (`id`)
);
INSERT INTO `users` (`id`, `name`, `score`, `active`, `joined`) VALUES
(1, 'Alice', 9.5, 1, '2024-01-02'),
(2, 'Bob', 7, 0, '2024-02-10');
```

## Limits and edge cases

- This tool generates SQL text; it does not connect to a database or validate
  against an existing schema.
- Type inference is conservative. If mixed values would be unsafe as numeric or
  date types, the column falls back to text.
- CSV parsing supports quoted fields and doubled quotes, but does not execute
  spreadsheet formulas.
- Unquoted identifiers are allowed only when they are simple ASCII names. Leave
  **Quote identifiers** on for spaces, keywords, schema-qualified table names, or
  mixed-case columns.

## FAQ

<details>
<summary>Can it generate both CREATE TABLE and INSERT statements?</summary>

Yes. **Include CREATE TABLE** is on by default, and it uses inferred column types.
You can turn it off when you only need `INSERT` statements for an existing table.

</details>

<details>
<summary>How are column types inferred?</summary>

Each column is scanned across non-blank values. Integers, floats, booleans,
dates, and datetimes are recognized when every populated value fits that type.
Otherwise the column becomes text. Zero-padded numeric-looking values stay text.

</details>

<details>
<summary>Which SQL dialect should I choose?</summary>

Pick the database that will run the SQL. The dialect changes identifier quotes,
boolean values, prepared-statement placeholders, and data types such as
`TIMESTAMP`, `DATETIME`, `REAL`, and `BIT`.

</details>

<details>
<summary>What is placeholder mode for?</summary>

Placeholder mode emits a prepared statement shape instead of inline literals.
For example PostgreSQL uses `$1`, `$2`, and SQL Server uses `@p1`, `@p2`. The
bound values are shown in a trailing `-- params:` comment for easy copying into
application code or tests.

</details>

<details>
<summary>Will blank cells become empty strings?</summary>

Not by default. Blank CSV cells, JSON `null`, and missing JSON fields become
`NULL`. Change **Blank/null cells** to `empty-string` for `''`, or `default` for
the SQL `DEFAULT` keyword.

</details>
