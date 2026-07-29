## About this tool

**Prisma Schema from SQL** turns raw SQL DDL into a `schema.prisma` you can drop
straight into a Prisma project. Paste one or more `CREATE TABLE` statements (plus
any `ALTER TABLE ... ADD` and `CREATE INDEX` lines) and it produces a model for
each table with:

- **Fields** mapped to Prisma scalar types — `Int`, `BigInt`, `Boolean`,
  `Decimal`, `Float`, `DateTime`, `Json`, `Bytes` and `String` — with `?` for
  nullable columns.
- **Attributes** — `@id`, `@unique`, and `@default(...)` (`autoincrement()`,
  `now()`, `uuid()`, literals, or `dbgenerated("…")` for anything it can't
  simplify).
- **Native types** — `@db.VarChar(n)`, `@db.Char(n)`, `@db.Decimal(p,s)` — so a
  round-trip back to the database keeps the original column sizes.
- **Keys & indexes** — composite `@@id([...])`, `@@unique([...])` and `@@index([...])`.
- **Relations** — `@relation(fields: [...], references: [...])` inferred from
  foreign keys, with `onDelete`/`onUpdate` referential actions carried across.

It supports **PostgreSQL, MySQL, SQLite and SQL Server**, and it's a *lenient
parser, not a database*: comments and non-DDL statements (`INSERT`, `SELECT`,
`DROP`, …) are ignored and nothing is ever executed. Everything runs locally in
your browser — your schema never leaves the page.

## Worked example

Given this PostgreSQL DDL:

```sql
CREATE TABLE users (id SERIAL PRIMARY KEY);
CREATE TABLE orders (
  id SERIAL PRIMARY KEY,
  user_id INT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  total DECIMAL(10,2) NOT NULL
);
```

you get:

```prisma
model users {
  id Int @id @default(autoincrement())
}

model orders {
  id Int @id @default(autoincrement())
  user_id Int
  total Decimal @db.Decimal(10, 2)
  user users @relation(fields: [user_id], references: [id], onDelete: Cascade)
}
```

Turn **Map names to Prisma conventions** on and the models become `User`/`Order`
with camelCase fields and `@@map`/`@map` back to the original database names.

## FAQ

<details>
<summary>Does this connect to my database or run my SQL?</summary>

No. It only *reads* the DDL text you paste — `CREATE TABLE`, `ALTER TABLE ... ADD`
and `CREATE INDEX`. It never opens a connection and never executes anything.
`INSERT`, `SELECT`, `DROP` and comments are skipped, and the whole conversion
happens in your browser, so nothing is uploaded.

</details>

<details>
<summary>How are foreign keys turned into relations?</summary>

Each foreign key becomes a Prisma `@relation` field on the owning model, with
`fields: [...]` (the local columns) and `references: [...]` (the referenced
columns), plus `onDelete`/`onUpdate` actions mapped to Prisma's `Cascade`,
`Restrict`, `SetNull`, `SetDefault` or `NoAction`. A `user_id` column referencing
`users` produces a `user` field; when two foreign keys point at the same table,
each gets an explicit relation name so Prisma can tell them apart. Prisma also
expects the *back-relation* on the other model — add those opposite fields (e.g.
`orders Order[]`) yourself, or let `prisma format` prompt for them.

</details>

<details>
<summary>Why does a column come out as <code>String</code> or <code>dbgenerated(...)</code>?</summary>

`String` is Prisma's fallback for any textual or unrecognized type, so an exotic
or vendor-specific type you didn't expect will land there — change it by hand if
needed. `@default(dbgenerated("…"))` is used when a column default is an
expression the tool can't reduce to a Prisma helper (`now()`, `uuid()`, a literal),
which keeps the exact SQL default without guessing at its meaning.

</details>

<details>
<summary>What does "Map names to Prisma conventions" do?</summary>

Off (the default), table and column names are used verbatim — only sanitized to
valid Prisma identifiers. On, model names become PascalCase and singularized
(`blog_posts` → `BlogPost`) and fields become camelCase (`full_title` →
`fullTitle`), while `@@map`/`@map` preserve the original database names so the
schema still points at your real tables and columns.

</details>

<details>
<summary>Which providers are supported, and does the provider matter?</summary>

PostgreSQL, MySQL, SQLite and SQL Server. The provider sets the `datasource`
block, tunes parsing (for example MySQL's `TINYINT(1)` becomes `Boolean`), and
decides which native `@db.*` types are valid — SQLite has none, so native types
are omitted there even when the toggle is on.

</details>

## Limits

- It's a **best-effort DDL parser**, not a full SQL grammar — unusual syntax,
  vendor extensions, or partial statements may be skipped or fall back to
  `String`. Always review the output and run `prisma format` / `prisma validate`.
- **Back-relations aren't added.** Prisma requires a field on both sides of a
  relation; only the foreign-key side is generated.
- **Enums, views, stored procedures, triggers and partitions** are not modeled.
- Name mapping uses a **crude English singularizer**; odd plurals may need a
  manual tweak (the `@@map` still keeps the DB name correct).
