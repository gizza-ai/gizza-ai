# sql-playground — competitor analysis (2026-06-21)

New tool. Built end-to-end (chat block + CLI + page) and then sized against the
top online SQL-playground tools. All competitor notes are **paraphrased** — no
copy, branding, or trademarks reproduced.

## What we shipped

A browser-local SQL playground backed by **GlueSQL** (an SQL engine written
entirely in Rust → instantiates in the wafer wasm runtime and in the page's
wasm32-unknown-unknown target, no C/SQLite/native code). Each run creates a
fresh in-memory database, executes one or more `;`-separated statements, and
renders the **last** statement's result.

- **Params (single-sourced descriptor):** `sql` (required), `format` ∈
  `table | csv | json` (default `table`).
- **SQL coverage (via GlueSQL):** CREATE/DROP/ALTER TABLE, CREATE INDEX,
  INSERT/UPDATE/DELETE, SELECT with WHERE/ORDER BY/LIMIT/OFFSET/GROUP BY/HAVING/
  JOIN, aggregates (COUNT/SUM/AVG/MIN/MAX) and common functions, NULL handling.
- **Output:** aligned ASCII table, CSV (header + rows), or JSON (array of row
  objects). Non-SELECT statements report rows affected.
- **Surfaces verified:** chat schema + drift-guard test, wafer fixture, CLI
  (`gizza tool sql-playground`), page run, and query-param deep-link
  (`?sql=...&format=...`) — all green.

## Competitors surveyed (paraphrased)

1. **SQLite Online** — multi-engine WASM (SQLite/DuckDB/PGLite) + server engines;
   schema panel, charts, history, CSV/XLSX/JSON export, themes, P2P sharing.
   Local-first for the WASM engines.
2. **DB Fiddle** — server-provisioned MySQL/MariaDB/Postgres/SQLite with version
   pinning; two-pane schema+query; Markdown export; share-by-link. Server-side.
3. **SQL Fiddle** — learning sandbox, schema+query panels, execution-plan view,
   AI assistant; mostly server-side (transaction rollback per run).
4. **OneCompiler SQL** — cloud containers per engine (selected by URL), boilerplate
   sample data, short shareable URLs, saved history. Server-side, no WASM.
5. **ExtendsClass SQLite Online** — genuine in-browser SQLite via sql.js; open/
   drop `.db` files, CSV import/export, editable cells, structure panel,
   save-and-share. Fully client-side.

Honorable mention: **Programiz** — beginner SQLite sandbox with a fixed seeded DB
and an "available tables" panel.

## Gap analysis (fit-to-model)

**In-model and already covered (our strengths):**
- Pure-WASM, no account, no server, no upload — only SQLite Online and
  ExtendsClass are genuinely local-first; the server majority can't match the
  privacy/zero-storage story.
- **Multi-format output (table / CSV / JSON)** in one tool — across the field
  output is mostly a single grid; CSV/JSON breadth is a real differentiator.
  ✅ shipped.
- **Multiple statements per run** (DDL + seed + SELECT in one script). ✅ shipped.
- **Deep-linkable queries** via query params (`?sql=...&format=...`), encoded
  purely client-side, no server storage. ✅ shipped (verified by Playwright).

**In-model but deferred (would need page-driver/framework work beyond a single
field+select page; listed, not forced in):**
- Schema/structure panel auto-listing tables/columns — the gizza page driver
  renders a fixed input→output form, not an interactive multi-pane IDE.
- Multiple query tabs with localStorage autosave — same page-driver limit.
- CSV/file import into a table, editable result cells — needs a richer page UI
  than the shared tool chrome provides.
- Per-statement result blocks (we show the last statement's result, the common
  playground convention) — multi-output rendering isn't in the page driver.
- EXPLAIN / execution-plan view — GlueSQL has no plan output to surface.

**Out-of-model (cannot run browser-local / pure-Rust):**
- Multiple real DB engines (MySQL/Postgres/Oracle/SQL Server) and version
  pinning — those are server-backed; gizza is browser-local pure-Rust.
- Cloud-saved shareable snippets / accounts / history — no backend by design
  (our share mechanism is the stateless deep-link).
- AI query assistant, P2P collaboration — out of a single tool's scope.

## Decisions

- Markdown-table output was considered as a 4th format; kept the format set at
  table/CSV/JSON to match the descriptor + page select cleanly (CSV/JSON already
  cover the spreadsheet/programmatic copy paths). Easy to add later if desired.
- Not a duplicate of `blocks/sql-formatter` (which pretty-prints SQL text) — this
  tool *executes* SQL against a database. Distinct function.
