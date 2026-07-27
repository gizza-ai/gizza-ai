# Competitor analysis — sql-schema-extractor (2026-07-27)

**Tool:** Parses `CREATE TABLE` / `ALTER TABLE` statements from a SQL dump and emits a clean
table / column / constraint model. Pure text parser — no diagram rendering, no DB engine.

Scan performed BEFORE implementation (paraphrased research; no competitor copy/branding reused).

## Competitor landscape

| Tool | Type | Input dialects | Extracts | Output |
|------|------|----------------|----------|--------|
| node-sql-parser | JS lib | MySQL/Postgres/MariaDB + more | Full AST for CREATE/ALTER/DROP; columns, constraints, table/column lists | AST JSON; SQL round-trip |
| sql-ddl-to-json-schema | JS lib (nearley) | MySQL/MariaDB only | Columns + types/params, nullability, defaults, AUTO_INCREMENT, PK, UNIQUE, table options, ALTER | Compact JSON + JSON Schema |
| simple-ddl-parser | Python lib | 12+ dialects | Most complete: columns/types/sizes, defaults, nullability, PK, FK, UNIQUE, CHECK, indexes, ALTER ADD CONSTRAINT | Python dict / JSON |
| @dbml/core / sql2dbml | CLI/JS | MySQL/Postgres/SQL Server/Rails | Tables, columns, PK/FK relationships | DBML → diagram |
| DrawSQL "Import from DDL" | Web SaaS | MySQL/Postgres/SQL Server/MariaDB | Tables/columns, FKs w/ cardinality | Editable diagram |

**Market gap this tool owns:** the SaaS tools lock output into diagrams; the best pure-JSON
extractor is MySQL-only; the most complete extractor is a Python library with no web UI. A
dialect-flexible, browser-based CREATE/ALTER → clean **JSON and Markdown** extractor is
underserved.

## Table-stakes features → in/out of model

**In-model (built into the descriptor / core):**
- Raw multi-statement, multi-table SQL dump input (`sql`).
- Parse `CREATE TABLE` → tables + columns; data type with params (`VARCHAR(255)`, `DECIMAL(10,2)`).
- Nullability (`NOT NULL` / `NULL`), default values, `AUTO_INCREMENT`/`AUTOINCREMENT`/`IDENTITY`.
- PRIMARY KEY (inline + table-level, composite), FOREIGN KEY (composite, `REFERENCES`,
  `ON DELETE`/`ON UPDATE`), UNIQUE, CHECK, named indexes (`INDEX`/`KEY`, uniqueness).
- Parse `ALTER TABLE … ADD [COLUMN|CONSTRAINT|FK|PK|UNIQUE|CHECK|INDEX]`, folded onto the target
  table — **`apply_alter` toggle** (on by default).
- **`include_indexes` toggle** — keep or drop index definitions.
- Dialect selection (`dialect`: auto/mysql/postgres/sqlite/mssql/generic) — controls `#` comment
  handling and is echoed in the model; identifiers are normalized (backticks/`"`/`[]` stripped) for
  all dialects.
- Output `json` **and** `markdown` (per-table column tables + constraint lists).
- Lenient dump handling: DML (`INSERT`/`UPDATE`), comments (`--`, `#`, `/* */`) and unparseable
  statements are skipped, not fatal (default behavior — no extra flag needed).

**Out-of-model (documented, not built):**
- ER / relationship **diagram** rendering (dbdiagram/DrawSQL core).
- DBML / graphical export.
- Live DB introspection / connect-to-database import (needs a DB engine).
- AI "schema review" / suggestions; cardinality inference beyond explicit FKs.

## Defaults chosen
- `output` = json (Markdown one toggle away).
- `dialect` = auto.
- `apply_alter` = true (merge ALTERs into their target table).
- `include_indexes` = true.

## Worked example (also the page's first example)

Input:
```sql
CREATE TABLE users (
  id INT PRIMARY KEY AUTO_INCREMENT,
  email VARCHAR(255) NOT NULL UNIQUE,
  age INT DEFAULT 0 CHECK (age >= 0)
);
```
→ one table `users`, columns `id` (INT, not null, PK, auto-increment), `email` (VARCHAR(255), not
null, unique), `age` (INT, nullable, default 0, check `age >= 0`), primary key `[id]`.

## UX controls competitors ship (adopted)
- Output-format toggle (JSON / Markdown) → `output` `<select>`.
- Dialect dropdown → `dialect` `<select>` with friendly labels.
- Include-ALTER / group toggles → `apply_alter`, `include_indexes` checkboxes.
- Preset example chips → 3 `[[example]]` chips (single-table CREATE, CREATE+ALTER FK, Markdown).
- Copy/download output → provided generically by the page driver.

## Positioning
Differentiate on multi-dialect input, first-class JSON **and** Markdown output, full constraint
coverage (ALTER ADD CONSTRAINT, CHECK, composite FK) in the browser, and lenient dump handling.
Stay clear of diagrams, DBML, live DB connections, AI review — all out-of-model.

Sources (paraphrased): node-sql-parser, sql-ddl-to-json-schema, simple-ddl-parser, @dbml/core /
sql2dbml, DrawSQL Import-from-DDL, ddlparse, CSVJSON sql2json.
