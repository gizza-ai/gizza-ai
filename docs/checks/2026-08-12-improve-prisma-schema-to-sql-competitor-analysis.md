# Prisma schema to SQL competitor analysis (2026-08-12)

Backlog tool: `prisma-schema-to-sql` — translate a Prisma `schema.prisma` into `CREATE TABLE` DDL
for a chosen SQL dialect.

All observations below are paraphrased from public product surfaces and documentation. No
competitor copy, wording, branding or trademark is reused anywhere in the tool.

## Competitor scan

| Competitor | Observed surface | Table-stakes controls and UX | In-model decisions for this tool | Out-of-model / not built |
| --- | --- | --- | --- | --- |
| Prisma's own `migrate diff` CLI (`--from-empty --to-schema-datamodel … --script`) | Official CLI that renders the SQL a schema would produce, either from empty or as a diff between two schema states. | Exact provider-correct type mapping, Prisma's own constraint/index naming, referential actions, implicit m-n join tables, script-only output. | The type map, `Table_column_key` / `Table_column_idx` / `Table_pkey` naming, `ON DELETE`/`ON UPDATE` defaults (RESTRICT for required, SET NULL for optional, CASCADE on update) and `_AToB` join tables all match this behaviour, so the output is recognisable to anyone who has run the official command. | Diffing two schema states, reading a live database URL, migration-history bookkeeping and shadow databases all need a database connection and the real migration engine — explicitly out of scope, and the page says so. |
| FileLabs Prisma-to-SQL converter | Free web page that pastes a Prisma schema and returns `CREATE TABLE` statements. | Single paste box, instant conversion, no signup, copyable SQL. Its own notes concede that complex relations may need manual fixing. | Same one-paste flow with instant browser-local output, but relations are a first-class feature rather than a caveat: `@relation`, composite keys, implicit m-n and referential actions are all implemented and unit-tested. | Nothing observed there that is missing here. |
| FWDTools database schema designer | Visual schema designer that exports PostgreSQL/MySQL/SQLite DDL as well as a `schema.prisma`. | Multi-dialect DDL export covering `CREATE TABLE`, primary keys, unique constraints, indexes and foreign keys; no login. | Multi-dialect export is the central control here (`dialect`, with `auto` reading the schema's own `datasource`), and the same four statement families are emitted. SQL Server is added on top of their three. | A drag-and-drop visual designer / ER canvas is a UI product, not a pure-compute block; not built. |
| CodeTidy SQL-to-ORM converter | Converts SQL `CREATE TABLE` into Prisma, Sequelize, TypeORM, SQLAlchemy and Django models. | Fast paste-in/paste-out conversion, several ORM targets, error feedback on unparseable input. | Confirms the *reverse* direction is well served, which is why this tool goes Prisma → SQL only and states that plainly rather than half-implementing both. | SQL → Prisma and the non-Prisma ORMs are separate tools; not built here. |
| Devzstudio SQL-to-Prisma generator (open source) | Small open-source web app generating a Prisma schema from a SQL query. | Minimal single-purpose UI, deployed as a static site, transparent behaviour. | Reinforces the browser-local, no-backend, no-upload posture: this block compiles to WebAssembly and never contacts a server or executes SQL. | Same reverse direction as above; not built. |

## Table-stakes matrix

| Capability | Decision | Notes |
| --- | --- | --- |
| Models → `CREATE TABLE` with Prisma's provider type map | In model | String/Boolean/Int/BigInt/Float/Decimal/DateTime/Json/Bytes across four dialects. |
| `@db.*` native types | In model | Passed through verbatim, including arguments (`@db.VarChar(200)` → `VARCHAR(200)`). |
| `@id` and composite `@@id` | In model | Named `Table_pkey` on PostgreSQL/SQL Server; inline `PRIMARY KEY` elsewhere. |
| `@unique` / `@@unique` / `@@index` | In model | `indexes` toggle; Prisma's index naming, or the attribute's `map:` name. |
| `@relation` foreign keys with referential actions | In model | `foreign_keys` toggle; emitted as trailing `ALTER TABLE` so creation order is irrelevant. |
| Implicit many-to-many join tables | In model | `_AToB` table, composite PK, `B` index, both cascading FKs, PK-typed columns. |
| `enum` blocks | In model | `CREATE TYPE` (PostgreSQL), inline `ENUM(...)` (MySQL), `CHECK (… IN …)` (SQLite/SQL Server). |
| `@map` / `@@map` renames | In model | Applied to columns, tables, indexes, primary keys and foreign keys alike. |
| Auto-increment per dialect | In model | `SERIAL`/`BIGSERIAL`, `AUTO_INCREMENT`, `PRIMARY KEY AUTOINCREMENT`, `IDENTITY(1,1)`. |
| Defaults | In model | Literals, `now()`, `dbgenerated("…")`; client-side `uuid()`/`cuid()`/`ulid()`/`nanoid()`/`auto()` deliberately emit none. |
| Dialect auto-detection from `datasource` | In model | `dialect = auto`, falling back to PostgreSQL when the paste has no datasource. |
| Re-runnable / rebuild scripts | In model | `if_not_exists` (incl. the SQL Server `IF OBJECT_ID` guard) and `drop_if_exists`. |
| Identifier quoting | In model | `quote_identifiers`, per-dialect delimiter; off gives a bare-identifier script. |
| `Unsupported("…")` fields | In model | The raw database type is emitted as written. |
| Diff against a live database or a previous schema | Out of model | Needs a connection and the migration engine; documented as out of scope. |
| MongoDB provider | Out of model | No `CREATE TABLE` equivalent for a document store. |
| Schema validation / error recovery like the Prisma compiler | Out of model | This is a lenient translator; it rejects unbalanced braces and unknown field types, not every invalid schema. |
| SQL → Prisma (reverse direction) | Out of model | Covered by other tools; a separate block if it is ever wanted. |
| Visual ER diagram / designer canvas | Out of model | UI product, not a pure-compute block. |

## Defaults and UX choices

- `dialect = auto` is the default because the pasted schema usually already declares its provider;
  the explicit choices exist for the common "just the models" paste and for retargeting.
- `foreign_keys` and `indexes` default to **on** — a faithful translation is the expected output —
  while `if_not_exists` and `drop_if_exists` default to **off** so nothing destructive or unusual is
  produced unless it is asked for.
- `quote_identifiers` defaults to **on**: Prisma's camelCase names do not survive unquoted on
  PostgreSQL, so the safe rendering is the default and the bare-identifier script is opt-in.
- Foreign keys are always emitted after all tables, which makes the script order-independent — the
  one place where a naive per-table rendering breaks on real schemas.
- Example chips cover the three shapes people actually paste: a blog schema with a relation and an
  index, an enum plus composite key on MySQL, and a re-runnable SQLite script.
- Competitor capabilities informed the control set and the limits section; no competitor wording or
  branding was reused.
