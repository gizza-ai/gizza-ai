# dbml-to-sql — competitor analysis (scan run 2026-09-24, filed under the loop's 2026-09-23 check date)

Scan done **before** implementing, per `/create-next-tool` step 4. Everything below is
**paraphrased** — no competitor copy, branding, or trademarks are reproduced anywhere in this
repo. Out-of-model items are listed, not built.

## What the tool does

DBML (Database Markup Language) is the plain-text schema language used by dbdiagram.io /
dbdocs.io. `dbml-to-sql` compiles a `.dbml` document into `CREATE TABLE` DDL.

## Competitors reviewed

### 1. `@dbml/cli` — `dbml2sql` (the reference implementation, Holistics)

- **Shape:** Node CLI, `dbml2sql <file> [--postgres|--mysql|--mssql|--oracle] [-o out.sql]`.
- **Default dialect:** PostgreSQL when no flag is given.
- **Dialects:** PostgreSQL, MySQL, MSSQL, Oracle. (No SQLite.)
- **Other:** resolves multi-file schemas through DBML's import/module system by being handed the
  entry-point file. Ships the inverse `sql2dbml` as a separate command.
- **Requires:** Node ≥ 18 install — i.e. not a paste-and-go surface.

### 2. Chat2DB — online DBML ↔ SQL converter

- **Params:** direction toggle (DBML→SQL and SQL→DBML) plus a dialect picker limited to
  PostgreSQL and MySQL.
- **Covers:** primary keys, auto-increment, defaults, unique constraints, indexes, foreign keys;
  backtick-wrapped defaults are passed through as raw SQL expressions; type names are remapped per
  dialect (jsonb/json, timestamptz/datetime, bytea/blob).
- **UX:** an example schema is pre-loaded so output shows immediately; live re-render on edit;
  copy-to-clipboard on the output; a collapsible explainer section about DBML syntax and how the
  refs map to foreign keys.
- **Positioning:** states the parse happens in the browser; funnels toward their desktop product
  for live-database schema export.

### 3. ChartDB — free online DBML to SQL / DDL generator

- **Params:** dialect picker with PostgreSQL (default), MySQL, SQL Server.
- **UX:** two-pane editor (DBML left, SQL right), live conversion as you type, a "load example"
  button, and a clear/reset button.
- **Positioning:** client-side only, schemas not uploaded. Cross-links sibling tools (SQL→DBML,
  ERD visualization, SQL formatting/validation).

## Table stakes → decision

| Table stake | Seen in | Verdict |
| --- | --- | --- |
| Paste DBML → CREATE TABLE DDL | all 3 | **in-model** — core of the tool |
| Dialect picker, PostgreSQL default | all 3 | **in-model** — `dialect` enum |
| MySQL target | all 3 | **in-model** |
| SQL Server / MSSQL target | dbml2sql, ChartDB | **in-model** |
| Oracle target | dbml2sql | **in-model** — superset of the online tools |
| SQLite target | neither online tool; sibling gizza tools have it | **in-model** — added |
| Auto-detect dialect from the schema | none (dbml2sql needs a flag) | **in-model differentiator** — read `Project { database_type: '…' }` |
| `[pk]` / `[primary key]` / composite `indexes { (a,b) [pk] }` | all 3 | **in-model** |
| `[increment]` → SERIAL / AUTO_INCREMENT / AUTOINCREMENT / IDENTITY / GENERATED AS IDENTITY | all 3 | **in-model** |
| `[not null]`, `[null]`, `[unique]`, `[default: …]` incl. backtick expressions | all 3 | **in-model** |
| `indexes { }` block: composite, `[unique]`, `[name: '…']`, `[type: btree\|hash]`, expression indexes | dbml2sql, Chat2DB | **in-model** |
| `Ref` in all three forms (inline `[ref: > t.c]`, short `Ref: a.b > c.d`, long `Ref name { … }`) | dbml2sql, Chat2DB | **in-model** |
| Composite refs `(a, b) > (c, d)` | dbml2sql | **in-model** |
| `[delete: cascade]` / `[update: …]` referential actions | dbml2sql | **in-model** |
| `Enum` blocks | dbml2sql, Chat2DB | **in-model** — CREATE TYPE (pg), inline ENUM (mysql), CHECK (sqlite/mssql/oracle) |
| Schema-qualified names `Table core.users` | dbml2sql | **in-model** |
| `Note:` on tables/columns → SQL comments | dbml2sql | **in-model** — `comments` toggle |
| Column `check:` constraints | DBML 3 spec | **in-model** |
| `TablePartial` + `~partial` expansion | DBML 3 spec | **in-model** |
| Many-to-many `<>` → join table | dbdiagram semantics | **in-model differentiator** — emitted as a real join table |
| Live re-render, copy button, load-example, reset | Chat2DB, ChartDB | **already platform** — the generator gives every page live run, Copy result, Download, Reset; `[[example]]` chips cover load-example (3 shipped) |
| Explainer / syntax help beside the tool | Chat2DB, ChartDB | **in-model** — `content.md` mapping tables + FAQs |

## In-model gaps added beyond every competitor

- **`auto` dialect** from `Project { database_type }` — no competitor infers the target.
- **Six dialects** (postgresql, mysql, sqlite, sqlserver, oracle + auto) vs two on Chat2DB, three
  on ChartDB, four on the reference CLI.
- **Re-runnable / rebuild toggles:** `if_not_exists` and `drop_if_exists`. No competitor ships
  these; they make the output pasteable into a scratch-database bootstrap.
- **`quote_identifiers` toggle** — lets you drop the quoting the reference implementation always
  applies.
- **`foreign_keys` / `indexes` / `comments` toggles** — load data first, constrain after.
- **Foreign keys emitted as trailing `ALTER TABLE`**, so table creation order never matters
  (a circular ref set breaks an inline-FK generator).
- **CLI + chat surfaces** in addition to the page, with no Node install.

## Out-of-model — considered, NOT built

- **SQL → DBML (reverse direction).** Chat2DB and ChartDB ship it. Out of scope for this slug,
  not out of model: it belongs in its own tool, and `sql-schema-extractor` / `er-diagram-from-sql`
  already occupy nearby ground here.
- **Rendered ERD diagram / visual schema editor** (ChartDB, dbdiagram) — a canvas app, not a
  paste-in/paste-out compute tool.
- **Multi-file DBML module imports** (`dbml2sql` resolves imported files from disk). The page and
  chat surfaces take one pasted document and have no filesystem; a single-file paste is the model.
- **Live database introspection / connect-and-export** (Chat2DB desktop) — needs a server and
  credentials.
- **`records` seed-data blocks → INSERT statements.** Parsed and skipped; `json-to-sql-insert`
  and `csv-to-sql` already cover INSERT generation.

## Copy / SEO angles worth covering (our own words)

Worked example with input **and** output; a Prisma-style DBML→SQL type mapping table per dialect;
what each DBML construct compiles to; how the three `Ref` forms differ; why FKs are emitted last;
stated limits (max input size, no execution, single-file only).
