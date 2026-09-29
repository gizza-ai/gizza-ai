## Compile DBML into SQL DDL

DBML is a compact way to describe tables, columns, indexes and relationships.
This tool turns that markup into executable SQL DDL for PostgreSQL, MySQL,
SQLite, SQL Server or Oracle. It runs locally in your browser, so schema drafts
never have to be uploaded to a server.

### Worked example

Input DBML:

```dbml
Table users {
  id int [pk, increment]
  email varchar [unique, not null]
}

Table orders {
  id int [pk]
  user_id int [not null]
}

Ref: orders.user_id > users.id
```

PostgreSQL output includes quoted `CREATE TABLE` statements, a unique index for
`users.email`, and an `ALTER TABLE ... ADD CONSTRAINT ... FOREIGN KEY` statement
for `orders.user_id`.

### What is supported

- `Project { database_type: '...' }` auto-detection, or an explicit dialect.
- `Table`, schema-qualified table names, `TablePartial` injection and `Enum`.
- Column flags such as `pk`, `increment`, `not null`, `unique`, `default`, `check`
  and `note`.
- `indexes { ... }` blocks and inline unique columns.
- Standalone `Ref:` relationships and inline `ref:` settings, including
  many-to-many join-table generation.
- Optional foreign keys, indexes, comments, `IF NOT EXISTS`, `DROP IF EXISTS` and
  quoted identifiers.

### Limits

Input is capped at 200,000 bytes. The parser is designed for practical DBML
schemas, not for executing arbitrary SQL; unsupported DBML sections such as
records or sticky notes are ignored, and unknown native column types pass through
so you can still use dialect-specific types.

## FAQ

<details>
<summary>Which SQL dialect should I pick?</summary>

Use **auto** when your DBML has a `Project` block with `database_type`. If there
is no project setting, auto emits PostgreSQL. Pick a dialect explicitly when you
want to preview how types, identifiers and indexes will look for a specific
database.

</details>

<details>
<summary>Does it preserve foreign keys and indexes?</summary>

Yes. DBML `Ref` statements become trailing `ALTER TABLE` foreign-key statements,
so table order and circular references are safe. Index blocks and inline unique
columns become separate index statements when the index option is enabled.

</details>

<details>
<summary>What happens to DBML enums?</summary>

PostgreSQL gets `CREATE TYPE ... AS ENUM`. Other dialects inline the enum values
as a string-like type with a `CHECK` constraint where practical, because they do
not all share PostgreSQL's enum syntax.

</details>

<details>
<summary>Can I use this as a migration tool?</summary>

It generates DDL from the current DBML shape; it does not diff an existing
database or produce reversible migrations. Use the `DROP IF EXISTS` option for a
rebuild script, or feed the output into your migration workflow for review.

</details>
