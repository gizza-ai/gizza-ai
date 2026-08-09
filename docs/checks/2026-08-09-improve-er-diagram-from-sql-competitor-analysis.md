# er-diagram-from-sql — competitor analysis (2026-08-09)

Scan done BEFORE implementing. All notes are paraphrased observations of what each
tool exposes; no competitor copy, wording, branding or trademarks were reused.

## Scope check (why this is not a duplicate)

Existing SQL-DDL blocks were checked first:

- `blocks/sql-schema-extractor` — parses `CREATE TABLE` / `ALTER TABLE` / `CREATE INDEX`
  into a structured **JSON or Markdown model**. No diagram output, no cardinality
  inference, no Mermaid.
- `blocks/prisma-schema-from-sql` — same parser, but emits a **Prisma `schema.prisma`**.
- `blocks/compose-to-diagram` — emits Mermaid, but from a **docker-compose.yml**, not SQL.
- `blocks/sqlite-db-inspector`, `blocks/schema-inferrer`, `blocks/csv-to-sql`,
  `blocks/spreadsheet-to-sql`, `blocks/json-to-sql-insert` — different inputs/outputs.

Nothing emits a Mermaid `erDiagram` from SQL DDL. This block follows the established
`prisma-schema-from-sql` pattern: **reuse** the shared lenient DDL parser
(`gizza-ai-sql-schema-extractor-core`) and own only the mapping to the new artifact —
here, Mermaid ER syntax plus crow's-foot cardinality inference.

## Competitors reviewed

1. **mermaideditor.com — SQL to Mermaid ERD converter.** Paste DDL, get Mermaid ER code.
   Exposes a dialect dropdown (MySQL default; MySQL/PostgreSQL/SQLite/MariaDB) and a
   toggle for heuristic relationship inference — when on, a column ending in `_id` is
   linked to a table whose name matches, in addition to explicit `FOREIGN KEY`
   constraints. Entity blocks show column name + data type in the sample output.
   Client-side only; no documented size limits.

2. **xdevutilities.com — SQL to Mermaid diagram.** Paste DDL, renders the Mermaid live.
   Claims MySQL / PostgreSQL / SQL Server / SQLite plus ANSI SQL. No user-facing options
   documented. States it derives one-to-many from a foreign key pointing at a parent
   primary key, and treats a junction table holding a composite key over two entities as
   many-to-many. Openly documents that FK detection is still incomplete in the shipped
   version, that input must begin with `CREATE TABLE` with balanced parentheses, that some
   keywords break rendering, and advises limiting input to core tables to avoid clutter.
   No worked example given.

3. **sqltoerdiagram.com — SQL to ER diagram.** Broadest of the three: accepts DDL plus
   Prisma / DBML / Mermaid / SQLAlchemy / Sequelize, and lists many dialects (Postgres,
   MySQL, MariaDB, SQLite, SQL Server, Oracle, Snowflake, BigQuery). It is primarily an
   interactive canvas — drag/rename tables, notes, groups, auto-arrange, zoom, a
   layout-direction switch (horizontal/vertical), and spacing presets — with export to
   PNG/SVG and to Mermaid / DBML / PlantUML code. Ships a "load example schema" button.
   Shows tables, columns, primary keys, foreign keys, relationships; exact PK/FK/nullable
   glyphs not documented.

Also consulted: the Mermaid `erDiagram` syntax reference, for the authoritative
cardinality markers (`|o`/`o|`, `||`, `}o`/`o{`, `}|`/`|{`), identifying `--` vs
non-identifying `..` lines, the `type name PK, FK "comment"` attribute grammar, the
`type?` optional-attribute form, and entity-name quoting rules.

## Table-stakes extracted → decisions

| Table stake (seen in ≥1 competitor) | Decision | Where |
| --- | --- | --- |
| Paste SQL DDL, get Mermaid ER code | in-model | `input` (required) |
| SQL dialect selector | in-model | `dialect` = auto (also mysql/postgres/sqlite/mssql/generic) |
| Relationships from explicit FOREIGN KEY | in-model, always on | core |
| Heuristic `<name>_id` → matching table | in-model, opt-in | `infer_relations` = false |
| Column data types inside entity blocks | in-model, default on | `include_types` = true |
| PK / FK / UK markers | in-model, default on | `key_markers` = true |
| Crow's-foot cardinality | in-model | derived from NOT NULL + uniqueness (rules below) |
| Layout direction control | in-model | `direction` = auto (LR/TB/RL/BT) |
| Trim big schemas to stay readable | in-model | `attributes` = all \| keys \| none |
| Example/preset to load in one click | in-model | 3 `[[example]]` chips on the page |
| Live rendered/interactive canvas, PNG/SVG export | **out-of-model** | needs a JS Mermaid renderer + canvas; this repo emits diagram *source*, which GitHub/GitLab/Notion/Obsidian render natively |
| Drag/rename/notes/groups/auto-arrange/zoom/spacing | **out-of-model** | interactive editor features, not a pure transform |
| DBML / PlantUML / SQLAlchemy / Sequelize / Prisma input+output | **out-of-model here** | different artifacts; `prisma-schema-from-sql` already owns the Prisma direction |
| Oracle / Snowflake / BigQuery dialects | **out-of-model** | shared parser normalizes identifiers and covers mysql/postgres/sqlite/mssql/generic; those three are not modelled |
| Collapsing junction tables into a single M:N edge | **out-of-model (deliberate)** | Mermaid convention keeps the join table as its own entity with two 1:N edges; collapsing would hide its payload columns. Documented in the page FAQ. |

## Additions beyond the competitors

- `relationship_label` (column \| constraint \| none) — competitors emit a fixed label;
  choosing the FK constraint name or dropping the label is useful for wide diagrams.
- `mark_nullable` — renders nullable columns with Mermaid's documented `type?` optional
  form, which none of the three surfaced.
- `fence` — wrap output in a ```mermaid code fence for direct paste into a Markdown file
  or a GitHub comment (the dominant destination for this output).

## Cardinality rules chosen (documented on the page)

For a foreign key on `child(cols)` referencing `parent`:

- parent side — `||` (exactly one) when every FK column is `NOT NULL`, else `|o` (zero or one);
- child side — `o{` (zero or more) normally, `o|` (zero or one) when the FK columns are
  themselves unique in the child (PK, `UNIQUE` constraint, or unique index), i.e. a 1:1;
- line style — solid `--` (identifying) for a `NOT NULL` FK, dashed `..` (non-identifying)
  for a nullable one, since a nullable FK means the child can exist without a parent.

So the common case renders as `users ||--o{ orders : "user_id"`.

## Mermaid-safety notes (found while reading the syntax reference)

Mermaid's attribute grammar does not accept spaces or commas inside a type token, so
`DECIMAL(10,2)` and `TIMESTAMP WITH TIME ZONE` break rendering if passed through
verbatim. Types and identifiers are therefore sanitized to Mermaid-safe tokens
(`DECIMAL(10_2)`, `TIMESTAMP_WITH_TIME_ZONE`), which preserves the precision information
instead of dropping it. Entity names that fall outside the safe set are double-quoted.
