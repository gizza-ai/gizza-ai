## About this tool

This tool reads a **Prisma** `schema.prisma` and writes the `CREATE TABLE` DDL
that would build the same database. Paste your models, pick a dialect, and you
get a script you can hand to `psql`, `mysql`, `sqlite3` or `sqlcmd` — or paste
into a migration file, a docker-entrypoint seed script, or a code review to show
exactly what a schema change does at the SQL level.

Nothing is executed and no database is contacted: the schema is parsed and
mapped inside your browser, and the result is text.

### What each part of the schema becomes

- **`model` blocks → `CREATE TABLE`.** Every scalar field becomes a column,
  typed the way Prisma's own migrations type it for the chosen provider.
  Required fields get `NOT NULL`; relation fields get no column of their own.
- **`@id` / `@@id` → `PRIMARY KEY`.** A single-field key and a composite
  `@@id([a, b])` both work; on PostgreSQL and SQL Server the constraint is named
  `Table_pkey`, matching Prisma's naming.
- **`@default(...)` → `DEFAULT`.** Literals, `now()`, and `dbgenerated("…")`
  become SQL defaults. `autoincrement()` becomes `SERIAL`/`BIGSERIAL`,
  `AUTO_INCREMENT`, `PRIMARY KEY AUTOINCREMENT` or `IDENTITY(1,1)` depending on
  the dialect.
- **`@unique` / `@@unique` / `@@index` → index statements**, named the way
  Prisma names them (`Table_column_key`, `Table_column_idx`) unless the
  attribute carries its own `map:` name.
- **`enum` blocks →** a `CREATE TYPE … AS ENUM` on PostgreSQL, an inline
  `ENUM('…')` column on MySQL, and a `CHECK (col IN ('…'))` constraint on SQLite
  and SQL Server, which have no enum type of their own.
- **`@relation(fields: […], references: […])` → foreign keys**, emitted as
  `ALTER TABLE … ADD CONSTRAINT … FOREIGN KEY` after all the tables exist, so
  creation order never matters. `onDelete`/`onUpdate` map to `ON DELETE`/
  `ON UPDATE` actions; when they are absent, Prisma's own defaults apply —
  `RESTRICT` for a required relation, `SET NULL` for an optional one, and
  `CASCADE` on update.
- **Implicit many-to-many relations** (a list on both sides, no `fields:`) get
  their `_AToB` join table, with the `A`/`B` columns typed from each side's
  primary key, the composite primary key, the `B` index and both cascading
  foreign keys.
- **`@map` / `@@map` → renamed columns and tables**, and the new names are used
  everywhere downstream — indexes, primary keys and foreign keys all follow.
- **`@db.*` native types are used verbatim**, so `@db.VarChar(200)` becomes
  `VARCHAR(200)` and `@db.Timestamptz(6)` becomes `TIMESTAMPTZ(6)`.

### Type mapping

| Prisma | PostgreSQL | MySQL | SQLite | SQL Server |
| --- | --- | --- | --- | --- |
| `String` | `TEXT` | `VARCHAR(191)` | `TEXT` | `NVARCHAR(1000)` |
| `Boolean` | `BOOLEAN` | `TINYINT(1)` | `BOOLEAN` | `BIT` |
| `Int` | `INTEGER` | `INT` | `INTEGER` | `INT` |
| `BigInt` | `BIGINT` | `BIGINT` | `BIGINT` | `BIGINT` |
| `Float` | `DOUBLE PRECISION` | `DOUBLE` | `REAL` | `FLOAT(53)` |
| `Decimal` | `DECIMAL(65,30)` | `DECIMAL(65,30)` | `DECIMAL` | `DECIMAL(32,16)` |
| `DateTime` | `TIMESTAMP(3)` | `DATETIME(3)` | `DATETIME` | `DATETIME2` |
| `Json` | `JSONB` | `JSON` | `JSONB` | *not supported* |
| `Bytes` | `BYTEA` | `LONGBLOB` | `BLOB` | `VARBINARY(MAX)` |

A `@db.*` attribute always wins over the row above.

### The toggles

**SQL dialect** defaults to *Auto*, which reads the `datasource` block's
`provider` and falls back to PostgreSQL when the schema has no datasource —
handy when you paste only the models. **Emit foreign-key constraints** and
**Emit index statements** let you keep just the bare tables, which is what you
usually want when seeding a scratch database and loading data before the
constraints go on. **Add IF NOT EXISTS guards** makes the script re-runnable,
and **Prepend DROP TABLE IF EXISTS** makes it rebuild from zero — that one is
destructive, so keep it for throwaway databases. **Quote identifiers** is on by
default because Prisma's camelCase table and column names need quoting to
survive; turn it off for a lowercase, folded-identifier script.

### Worked example

Schema:

```prisma
model User {
  id    Int     @id @default(autoincrement())
  email String  @unique
  name  String?
}
```

PostgreSQL output:

```sql
CREATE TABLE "User" (
    "id" SERIAL NOT NULL,
    "email" TEXT NOT NULL,
    "name" TEXT,
    CONSTRAINT "User_pkey" PRIMARY KEY ("id")
);

CREATE UNIQUE INDEX "User_email_key" ON "User"("email");
```

## Limits & edge cases

- The schema must be at most **200,000 bytes**. Larger input is rejected rather
  than silently truncated.
- The parser is **lenient and structural**, not the real Prisma compiler: it
  reads `model`, `view`, `enum`, `datasource` and `generator` blocks and ignores
  everything it does not model. It will not tell you your schema is invalid — it
  is a translator, not a validator. Unbalanced braces and unknown field types
  are the two things it does reject.
- A field whose type is neither a Prisma scalar, an `enum` in the same schema,
  nor a `model` in the same schema is an error, because there is nothing to map
  it to. Paste the whole schema, not one model out of it.
- `Json` has no SQL Server equivalent here and is reported as an error; store it
  as `String @db.NVarChar(Max)` if you need that dialect.
- **Scalar lists** (`String[]`) exist only on PostgreSQL, where they become
  array columns. On the other dialects the field is skipped and a `--` comment
  marks the spot.
- **Client-side ID generators produce no database default.** `uuid()`, `cuid()`,
  `ulid()`, `nanoid()` and `auto()` are filled in by Prisma Client, so the
  column is emitted without a `DEFAULT`. Use `dbgenerated("gen_random_uuid()")`
  when you want the database to do it.
- **`@updatedAt` is application behaviour**, not a database trigger, so no
  `ON UPDATE CURRENT_TIMESTAMP` is generated — Prisma's own migrations behave
  the same way.
- Explicit many-to-many models (a join model you wrote yourself) are ordinary
  tables and need no special handling; only *implicit* m-n relations synthesise
  a `_AToB` table.
- MongoDB schemas are out of scope: there is no `CREATE TABLE` for a document
  database, and `@db.ObjectId`/`auto()` have no SQL meaning.
- The output is a **from-empty** script — it builds a schema, it does not diff
  one against a live database. For an incremental migration between two schema
  versions, use Prisma's own migration tooling.

## FAQ

<details>
<summary>Is this the same SQL that Prisma Migrate would generate?</summary>

It is very close, and it follows the same type map, the same constraint and
index naming (`Table_column_key`, `Table_column_idx`, `Table_pkey`) and the same
referential-action defaults, so the two line up on ordinary schemas. It is not a
byte-for-byte reimplementation of the migration engine, though: statement
ordering, some provider-specific edge cases and any feature-preview behaviour
can differ. Treat the result as a very good starting script and review it before
running it against anything you care about.

</details>

<details>
<summary>Why does my <code>@default(uuid())</code> column have no DEFAULT?</summary>

Because `uuid()`, `cuid()`, `ulid()`, `nanoid()` and `auto()` are generated by
Prisma Client in your application, not by the database — the column genuinely
has no server-side default in a Prisma-managed schema either. If you want the
database to generate the value, express that explicitly with
`@default(dbgenerated("gen_random_uuid()"))`, and the expression is passed
straight through into the DDL.

</details>

<details>
<summary>What happens to my enums on SQLite and SQL Server?</summary>

Neither dialect has a native enum type, so the column is emitted as `TEXT`
(SQLite) or `NVARCHAR(1000)` (SQL Server) with a `CHECK (col IN ('A', 'B'))`
constraint that enforces the same set of values. On PostgreSQL you get a real
`CREATE TYPE … AS ENUM` and the column references it; on MySQL you get an inline
`ENUM('A', 'B')` column. `@map` on an enum value is respected everywhere, so the
database sees the mapped string, not the Prisma identifier.

</details>

<details>
<summary>Do I get foreign keys for both sides of a relation?</summary>

No — and that is correct. A Prisma relation is written on both models, but only
one side carries `fields:` and `references:`, and that is the side that owns the
column and therefore the foreign key. The back-relation field (`posts Post[]` on
`User`, say) produces neither a column nor a constraint. All the foreign keys
are emitted as `ALTER TABLE` statements after every `CREATE TABLE`, so you can
run the script top to bottom regardless of the order your models are in.

</details>

<details>
<summary>Can I run the script twice, or rebuild a scratch database from zero?</summary>

Yes. **Add IF NOT EXISTS guards** makes every `CREATE TABLE` (and, on PostgreSQL
and SQLite, every index) skip quietly if the object is already there; on SQL
Server it becomes an `IF OBJECT_ID(…) IS NULL` guard, since that dialect has no
`IF NOT EXISTS` on `CREATE TABLE`. **Prepend DROP TABLE IF EXISTS** goes further
and drops everything first, in reverse creation order, plus the PostgreSQL enum
types. That second one destroys data, so point it only at a throwaway database.

</details>

<details>
<summary>What if I only paste the models, with no datasource block?</summary>

That works. The **SQL dialect** control defaults to *Auto*, which looks for a
`datasource` block and uses its `provider`; with no datasource to read, it falls
back to PostgreSQL. Pick a dialect explicitly from the dropdown whenever you
want a specific target, and the datasource — if there is one — is ignored.

</details>
