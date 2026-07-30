# sql-dialect-converter — competitor analysis (2026-07-30)

Scan of the top real SQL dialect-converter tools before implementing. All observations are
paraphrased; no competitor copy, branding, or trademarks are reproduced. "Out-of-model" items
are listed for transparency, not built.

## Competitors skimmed

1. **sqlformat.io — free online SQL dialect converter.** Converts between MySQL, PostgreSQL and
   SQLite. Advertises "40+ syntax differences handled automatically", private/client-side, no
   signup. Source + target dialect pickers; paste-in text area; converted output pane.
2. **ChartDB SQL dialect converter.** MySQL / PostgreSQL / SQL Server / SQLite. AI-assisted;
   claims to handle syntax, data types and functions. Two dialect dropdowns + editor panes.
3. **FeuTex online SQL converter.** MySQL / PostgreSQL / SQL Server / SQLite, client-side
   transforms. Explicitly lists identifier quoting, LIMIT/OFFSET, and common DDL patterns as what
   it rewrites.

(Also seen: SQLines online, SQLAgnostic (SQLGlot-based, 32+ dialects, AI refinement + visual
diff), SQLAI.ai (LLM-based, 8+ dialects).)

## Table-stakes params / behaviours

| Feature | In competitors | Our decision | In/out of model |
|---|---|---|---|
| Source dialect selector | yes (all) | `from` enum: postgres/mysql/sqlite | in-model |
| Target dialect selector | yes (all) | `to` enum: postgres/mysql/sqlite | in-model |
| SQL input text area | yes (all) | `sql` string, multiline field | in-model |
| Identifier quote conversion (`"x"` ↔ `` `x` `` ↔ `[x]`) | yes | requote all delimited identifiers to target style, everywhere (queries + DDL) | in-model |
| Auto-increment (`SERIAL` ↔ `AUTO_INCREMENT` ↔ `INTEGER PRIMARY KEY AUTOINCREMENT`) | yes | full reconciliation inside `CREATE TABLE` columns | in-model |
| Data-type mapping (bool, text/varchar, timestamp/datetime, blob/bytea, double, json/jsonb, uuid, …) | yes | canonical type table, mapped in `CREATE TABLE` columns | in-model |
| MySQL table options stripped when target ≠ MySQL (`ENGINE=…`, `DEFAULT CHARSET=…`) | partial | drop trailing MySQL table-option tail on CREATE TABLE when target ≠ mysql | in-model |
| LIMIT / OFFSET | yes | identical across all three dialects → pass through unchanged | n/a (no rewrite needed) |
| Preset examples / chips | some | 3 `[[example]]` preset chips (pg→mysql, mysql→pg, pg→sqlite) | in-model |
| Client-side / private | yes | runs as local wasm, nothing uploaded | in-model |

## Out-of-model (listed, not built)

- **Full AST parsing / SQLGlot-level fidelity** (SQLAgnostic, ChartDB) — we use a forgiving
  tokenizer, not a full grammar per dialect.
- **AI/LLM refinement** of ambiguous conversions (ChartDB, SQLAI.ai, SQLAgnostic).
- **Function / expression rewriting**: string concat `||` ↔ `CONCAT()`, date functions
  (`NOW()`/`CURDATE()`), `x::type` casts ↔ `CAST(x AS …)`, `IFNULL`/`COALESCE`, `GROUP_CONCAT`
  vs `STRING_AGG`. Expression-level rewriting needs semantic analysis; we convert identifiers
  everywhere but only convert *types* inside `CREATE TABLE` column definitions.
- **Stored procedures, triggers, views, functions** and procedural language bodies.
- **SQL Server / Oracle / BigQuery / Snowflake** and other dialects beyond the three named.
- **Type conversion inside queries and `ALTER TABLE … ADD COLUMN`** (identifiers still convert;
  types there are left as written).
- **Side-by-side visual diff view** (a UI feature; our page shows the converted output pane).

## Design decisions baked into the descriptor from the start

- Three params: `sql` (required string, multiline), `from` (required enum), `to` (required enum),
  each `Param::enumv` for the fixed dialect choices, each `.describe()`d.
- The three transform pillars named in the backlog description — **identifiers, auto-increment,
  types** — are all implemented in-model; everything expression/procedure-level is documented as
  out-of-model above and in the page FAQ so the tool never over-promises.
