## About this tool

The **SQL Dialect Converter** rewrites SQL so it runs on a different database
engine. Pick the **From** and **To** dialects (PostgreSQL, MySQL or SQLite),
paste your SQL, and get it back rewritten for the target — no signup, no upload.

It focuses on the three things that actually differ between these engines and
break a copy-paste migration:

- **Identifier quoting.** Delimited identifiers are re-quoted to the target
  style everywhere they appear — Postgres/SQLite `"name"`, MySQL `` `name` ``,
  and SQL-Server-style `[name]` brackets are all understood on input.
- **Auto-increment columns.** `SERIAL` (Postgres), `AUTO_INCREMENT` (MySQL) and
  `INTEGER PRIMARY KEY AUTOINCREMENT` (SQLite) are reconciled inside
  `CREATE TABLE` — `BIGINT`/`BIGSERIAL` keep their width.
- **Column data types.** Inside `CREATE TABLE`, common types are mapped through
  a canonical table: `BOOLEAN` ⇄ `TINYINT(1)` ⇄ `INTEGER`, `VARCHAR(n)` →
  `TEXT` for SQLite, `TIMESTAMP` ⇄ `DATETIME`, `BYTEA` ⇄ `BLOB`,
  `DOUBLE PRECISION` ⇄ `DOUBLE` ⇄ `REAL`, `JSON`/`JSONB`, `UUID`, and more.

It also **strips MySQL table options** (`ENGINE=…`, `DEFAULT CHARSET=…`) when
the target isn't MySQL, and leaves **string literals and comments untouched**.

Everything runs **locally in your browser** via WebAssembly — your schema is
never uploaded.

### Worked example

Input (from **PostgreSQL**, to **MySQL**):

```sql
CREATE TABLE "users" (
  id SERIAL PRIMARY KEY,
  email VARCHAR(255),
  active BOOLEAN
);
```

Output:

```sql
CREATE TABLE `users` (
  id INT AUTO_INCREMENT PRIMARY KEY,
  email VARCHAR(255),
  active TINYINT(1)
);
```

### Scope & limits

This is a **forgiving tokenizer**, not a full SQL parser, so it is fast and
predictable but deliberately narrow:

- Data **types are only mapped inside `CREATE TABLE` column definitions** —
  identifiers still re-quote everywhere, but types written in queries or
  `ALTER TABLE … ADD COLUMN` are left as-is.
- **Expression and function rewriting is not done**: string concat (`||` vs
  `CONCAT()`), date functions (`NOW()` / `CURDATE()`), `x::type` casts vs
  `CAST(…)`, `IFNULL`/`COALESCE`, and `GROUP_CONCAT` vs `STRING_AGG` are left
  unchanged.
- **Stored procedures, triggers, views and functions** are out of scope.
- Only **PostgreSQL, MySQL and SQLite** are supported (no SQL Server, Oracle,
  BigQuery or Snowflake).
- Converting a dialect **to itself** returns the input unchanged.

Always review and test converted DDL against your target database before
running it in production.

## FAQ

<details>
<summary>Which dialects can it convert between?</summary>

PostgreSQL, MySQL and SQLite — in any of the six directions. Pick a **From**
and a **To** dialect. Other engines (SQL Server, Oracle, BigQuery, Snowflake)
are not supported. If **From** and **To** are the same, the SQL is returned
unchanged.

</details>

<details>
<summary>Does it convert data types in my queries too?</summary>

Type mapping runs only inside `CREATE TABLE` **column definitions** — that's
where a type must be spelled in the target dialect. In `SELECT`/`INSERT`
statements and in `ALTER TABLE … ADD COLUMN`, **identifiers are still
re-quoted**, but any type names are left exactly as written, because rewriting
them safely needs full semantic analysis.

</details>

<details>
<summary>What happens to auto-increment primary keys?</summary>

They're reconciled to the target's idiom: Postgres `SERIAL` / `BIGSERIAL`,
MySQL `INT`/`BIGINT … AUTO_INCREMENT`, and SQLite's required
`INTEGER PRIMARY KEY AUTOINCREMENT`. `BIGINT`-width columns stay 64-bit. The
`PRIMARY KEY` marker is carried across (and forced for SQLite, which needs it
for `AUTOINCREMENT`).

</details>

<details>
<summary>Will it rewrite functions like NOW(), CONCAT() or ::casts?</summary>

No. Expression- and function-level differences (`||` vs `CONCAT()`, `NOW()` vs
`CURDATE()`, `x::int` vs `CAST(x AS INT)`, `IFNULL` vs `COALESCE`,
`GROUP_CONCAT` vs `STRING_AGG`) are **not** rewritten — that needs a full
per-dialect parser. This tool handles identifiers, auto-increment and
`CREATE TABLE` types. Review the output and adjust expressions by hand.

</details>

<details>
<summary>Is my SQL uploaded anywhere?</summary>

No. The conversion runs entirely in your browser via WebAssembly. Your schema,
table names and any embedded values never leave your device — nothing is sent
to a server or logged.

</details>
