## About this tool

SQL Schema Extractor turns raw DDL from a dump into a clean schema model without connecting to a database or executing anything. Paste `CREATE TABLE`, `ALTER TABLE`, and `CREATE INDEX` statements and it extracts tables, columns, types, nullability, defaults, primary keys, foreign keys, unique constraints, checks, and indexes.

Use JSON when you want structured data for automation, audits, migrations, or documentation pipelines. Use Markdown when you want a readable schema reference with one section per table. Comments and data rows are ignored, so mixed dumps that include `INSERT` statements still produce a schema-only result.

The parser is intentionally lenient and local-first. It normalizes common identifier quoting styles from MySQL, PostgreSQL, SQLite, SQL Server, and generic SQL, then folds supported `ALTER TABLE ... ADD` statements onto their target table by default so the output represents the final schema.

## FAQ

<details>
<summary>Does this execute my SQL or connect to a database?</summary>

No. The tool is a text parser that runs locally in the browser/CLI. It reads DDL text and emits a model; it does not connect to a database, run queries, create tables, or inspect live data.

</details>

<details>
<summary>Which SQL statements are parsed?</summary>

It focuses on schema-defining statements: `CREATE TABLE`, supported `ALTER TABLE ... ADD` forms, and `CREATE INDEX` / `CREATE UNIQUE INDEX`. Non-DDL statements such as `INSERT`, `UPDATE`, `SELECT`, `DROP`, and comments are skipped so dump files remain usable.

</details>

<details>
<summary>What output format should I choose?</summary>

Choose `json` for automation: it includes table counts, columns, constraints, foreign keys, checks, and indexes in a structured model. Choose `markdown` when you want a human-readable schema document with table sections and constraint lists.

</details>

<details>
<summary>Does it support every SQL dialect feature?</summary>

No. It covers common DDL patterns across MySQL, PostgreSQL, SQLite, SQL Server, and generic SQL, including quoted identifiers and common column/constraint syntax. It does not try to be a full SQL engine, parse stored procedures, infer relationships without explicit foreign keys, or render ER diagrams.

</details>
