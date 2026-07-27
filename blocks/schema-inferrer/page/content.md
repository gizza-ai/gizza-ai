## Infer a JSON Schema and a SQL CREATE TABLE from sample data

Paste a **sample dataset** — CSV/delimited text with a header row, or JSON (one record object
or an array of record objects) — and this tool infers a type and nullability for every column,
then emits two schema artifacts at once:

- a standard **JSON Schema** (Draft 2020-12 or Draft-07), and
- a **SQL `CREATE TABLE`** statement for your chosen dialect.

It reads schemas *only* — no `INSERT` rows — so the output is exactly the structure definition you
paste into a validator, a migration, or an ORM. Everything runs locally in your browser.

### Worked example

Input (JSON records):

```json
[{"id":1,"email":"a@example.com","active":true},
 {"id":2,"email":"b@example.com","active":false}]
```

With **table** `users` and **primary key** `id`, the `both` output is:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "users",
  "type": "array",
  "items": {
    "type": "object",
    "properties": {
      "id": {
        "type": "integer"
      },
      "email": {
        "type": "string",
        "format": "email"
      },
      "active": {
        "type": "boolean"
      }
    },
    "required": [
      "id",
      "email",
      "active"
    ],
    "additionalProperties": false
  }
}

CREATE TABLE "users" (
  "id" INTEGER NOT NULL,
  "email" TEXT NOT NULL,
  "active" BOOLEAN NOT NULL,
  PRIMARY KEY ("id")
);
```

### How inference works

- **Types**: each column is checked most-specific first — boolean → integer → float → date →
  datetime → text. Zero-padded codes (`007`, ZIP codes) stay text so they keep their leading zero.
- **Nullability**: a column that has a value in *every* record becomes `NOT NULL` (SQL) and appears
  under `required` (JSON Schema). A blank CSV cell, a JSON `null`, or a key missing from some record
  makes the column nullable — its JSON Schema type becomes `["<type>", "null"]`. Turn this off with
  **NOT NULL for full columns**.
- **String formats**: text columns whose values all match `email`, `uri`, `uuid`, or `ipv4` get that
  JSON Schema `format`. Date and datetime columns always carry `date` / `date-time`. (Draft-07 has no
  `uuid` format, so it is omitted there.)
- **Primary key**: the named column is marked `PRIMARY KEY` in the SQL and treated as required /
  `NOT NULL` in both outputs.

### Limits and edge cases

- One flat table only — nested JSON objects/arrays are stringified into a single `text`/string column
  rather than split into related tables (no `FOREIGN KEY` / normalization).
- No `INSERT` statements — this tool emits schemas/DDL only. For data-loading SQL, use a CSV-to-SQL
  converter instead.
- `VARCHAR` lengths are not sized to the data: text maps to `TEXT` (MySQL/PostgreSQL/SQLite),
  `NVARCHAR(255)` (SQL Server), or `VARCHAR(255)` (ANSI).
- With no data rows, types cannot be inferred and every column falls back to a nullable string / `TEXT`.

<details>
<summary>What is the difference between this and a CSV-to-SQL or JSON-to-JSON-Schema tool?</summary>

Those tools produce a single artifact. A CSV-to-SQL converter emits `CREATE TABLE` **plus `INSERT`**
rows for loading data; a JSON-to-JSON-Schema tool emits only a JSON Schema. Schema Inferrer runs one
type-inference pass over your sample and returns **both** a JSON Schema and a `CREATE TABLE` (schemas
only, no `INSERT` rows), so you get a validation schema and a database definition side by side.

</details>

<details>
<summary>Which SQL dialects and JSON Schema drafts are supported?</summary>

SQL: MySQL, PostgreSQL, SQLite, SQL Server (`mssql`), and ANSI. The dialect sets identifier quoting
(backticks, double quotes, or brackets) and column types — for example a float becomes
`DOUBLE` / `DOUBLE PRECISION` / `REAL` / `FLOAT`, and a datetime becomes `TIMESTAMP` / `DATETIME` /
`DATETIME2`. JSON Schema: Draft 2020-12 (default) or Draft-07, which changes the `$schema` URL and
whether the `uuid` string format is emitted.

</details>

<details>
<summary>How are column types decided?</summary>

A column takes the most specific type that fits **all** of its non-empty values: boolean
(`true`/`false`/`yes`/`no`/`t`/`f`), integer, float, date (`YYYY-MM-DD`, `MM/DD/YYYY`, and slash/dash
variants), datetime (`YYYY-MM-DD HH:MM:SS` or ISO 8601 with `T`/`Z`), otherwise text. A single value
that does not fit demotes the whole column — e.g. one non-numeric cell keeps the column as text. Empty
cells and JSON nulls are ignored for type inference but still count toward nullability.

</details>

<details>
<summary>Does my data leave the browser?</summary>

No. Inference runs entirely in WebAssembly on this page — your sample dataset is never uploaded to a
server. You can also run it from the command line with the `gizza` CLI, or from chat.

</details>

<details>
<summary>Can I get just the JSON Schema or just the SQL?</summary>

Yes. Set **Output** to `JSON Schema only` or `SQL CREATE TABLE only`; the default `both` returns the
JSON Schema first, then the `CREATE TABLE`.

</details>
