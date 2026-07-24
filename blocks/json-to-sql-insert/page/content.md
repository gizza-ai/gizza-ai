## JSON to SQL INSERT generator

Paste a JSON object or an array of row objects and get SQL `INSERT` statements
you can run against your database. Everything happens locally in WebAssembly —
your data is never uploaded, so it's safe for real records.

### How it works

- **Input** — a single JSON object becomes one row; an array of objects becomes
  one row each. The columns are the **union of every row's keys**, in the order
  they first appear (turn on *Sort columns alphabetically* to reorder them). A
  key that's missing from a given row is treated as `null`, so heterogeneous
  rows still line up under one column list.
- **Dialect** — pick **MySQL** (`` `backticks` ``, `1`/`0` booleans, backslash
  escaping), **PostgreSQL** or **ANSI** (`"double quotes"`, `TRUE`/`FALSE`),
  **SQLite** (`"double quotes"`, `1`/`0`), or **SQL Server** (`[brackets]`,
  `1`/`0`). The dialect also drives placeholder syntax and inferred column types.
- **Values** — **literal** inlines properly-escaped SQL literals (a `'` becomes
  `''`; MySQL also doubles a `\`). **Placeholder** emits a parameterized /
  prepared statement (`?` for MySQL, SQLite and SQL Server as `@pN`, `$1, $2…`
  for Postgres) and lists the bound values in a trailing `-- params:` comment.
- **Batch vs per-row** — *Batch* produces one `INSERT … VALUES (…),(…)` for all
  rows; turn it off for a separate `INSERT` per row.
- **CREATE TABLE** — optionally emit a `CREATE TABLE` first, with each column's
  type inferred from the data (integer, float, boolean, or text). Name a
  **primary key** column, and optionally prepend `DROP TABLE IF EXISTS`.
- **Nulls** — choose whether a JSON `null` (or a missing key) becomes `NULL`,
  the keyword `DEFAULT` (so the column's own default applies), or an empty
  string `''`.
- **Identifiers** — table and column names are quoted for the dialect by
  default. Turn quoting off to emit bare identifiers; unsafe names (spaces,
  punctuation, a leading digit) are rejected with a clear error instead of
  producing broken or injectable SQL. A schema-qualified table like
  `public.users` is quoted part-by-part (`"public"."users"`).

### Example

Input (Postgres, with CREATE TABLE and `id` as the primary key):

```json
[{"id": 1, "email": "a@x.com", "active": true},
 {"id": 2, "email": "b@x.com", "active": false}]
```

Output:

```sql
CREATE TABLE "accounts" (
  "id" INTEGER,
  "email" TEXT,
  "active" BOOLEAN,
  PRIMARY KEY ("id")
);
INSERT INTO "accounts" ("id", "email", "active") VALUES
(1, 'a@x.com', TRUE),
(2, 'b@x.com', FALSE);
```

### Limits & edge cases

- Input must be a JSON **object** or an **array of objects**. A top-level scalar,
  an array of scalars (`[1, 2, 3]`), or an empty array is rejected with a message
  saying what was expected.
- **Nested** objects/arrays in a cell are stored as a compact JSON string literal
  (e.g. `{"k":[1,2]}` → `'{"k":[1,2]}'`) so nothing is dropped — the tool does
  not flatten them into extra columns or normalize into related tables.
- CREATE TABLE types are inferred only as integer / float / boolean / text; no
  lengths, precision, indexes, or foreign keys are generated.
- Numbers are emitted exactly as they appear in the JSON. Everything runs in your
  browser, so very large inputs are bounded by your device's memory.

### FAQ

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The generator is compiled to WebAssembly and runs entirely in your browser
tab — the JSON never leaves your device.

</details>

<details>
<summary>How are strings and quotes escaped?</summary>

A single quote is doubled (`O'Brien` → `'O''Brien'`) for every dialect, which is
the SQL-standard escape. For MySQL, a backslash is also doubled because MySQL
treats `\` as an escape character by default. Choose the matching dialect so the
escaping fits your database.

</details>

<details>
<summary>What does placeholder / prepared-statement mode produce?</summary>

It replaces each value with a positional placeholder — `?` for MySQL and SQLite,
`@p1, @p2…` for SQL Server, and `$1, $2…` for PostgreSQL — and appends the bound
values in order as a `-- params:` comment, so you can copy the statement and the
parameters separately into your client library.

</details>

<details>
<summary>What happens when rows have different keys?</summary>

The column list is the union of all keys across the rows, in first-seen order. A
row that's missing a column has that cell filled according to your *Nulls*
setting (`NULL` by default), so every row still fits one shared column list and
can be batched into a single INSERT.

</details>

<details>
<summary>Can it generate CREATE TABLE or UPDATE statements?</summary>

It can generate a `CREATE TABLE` (with inferred column types, an optional primary
key, and an optional `DROP TABLE IF EXISTS`) alongside the inserts. It focuses on
`INSERT`; `UPDATE` statements and schema normalization of nested JSON are out of
scope.

</details>
