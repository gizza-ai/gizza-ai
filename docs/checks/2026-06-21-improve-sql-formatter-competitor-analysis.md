# sql-formatter — competitor analysis (2026-06-21)

Tool: `blocks/sql-formatter` — pretty-prints / standardizes a SQL query. Pure-Rust,
dependency-free tokenizer + layout engine. Three surfaces verified: chat block (wafer
build + drift-guard schema test), CLI (`gizza tool sql-formatter`), and the standalone
page (Playwright, 2 specs green).

## Competitors surveyed (top 5)

1. **Devart SQL Formatter & Beautifier Online** (devart.com/dbforge) — profile-based
   styling, SQL-Server-focused, handles queries of any complexity.
2. **Aiven SQL Formatter** (aiven.io/tools/sql-formatter) — advanced options: keyword
   case, function case, datatype case, indentation width.
3. **Redgate SQL Formatter** (red-gate.com/website/sql-formatter) — free online
   beautifier/prettifier, dialect profiles.
4. **CodeBeautify SQL Formatter** (codebeautify.org/sqlformatter) — multi-DB (SQL Server,
   Oracle, DB2, MySQL, MariaDB, Sybase, Access, MDX), `.sql` file upload, also
   minify/compress.
5. **Poor SQL / PoorSQL** (poorsql.com) — open-source T-SQL formatter, tolerant of
   invalid/incomplete SQL, available as a library + command line.

(Sources: dpriver.com, red-gate.com, codebeautify.org, devart.com, poorsql.com,
aiven.io, fasttool.app, w3resource.com.)

## Feature gap matrix

| Capability                              | Competitors | gizza sql-formatter | Status |
|-----------------------------------------|:-----------:|:-------------------:|--------|
| Clause-per-line layout (SELECT/FROM/…)  | yes         | yes                 | parity |
| Configurable indent width               | yes         | yes (0–8)           | parity |
| Keyword case (upper/lower)              | yes         | yes (+preserve)     | parity / ahead (preserve) |
| Select-list / column item per line      | yes         | yes                 | parity |
| AND/OR broken onto indented lines       | yes         | yes                 | parity |
| JOIN clauses on their own line          | yes         | yes                 | parity |
| Preserve string literals & comments     | yes         | yes (`--`, `/* */`, `''` escapes) | parity |
| Forgiving on invalid/incomplete SQL     | partial     | yes (reformats token stream, never rejects) | parity / ahead |
| Dialect-agnostic                        | per-tool    | yes (token-based)   | parity |
| Command-line interface                  | some        | yes (`gizza` CLI)   | parity |
| API / programmatic                      | some        | yes (chat block)    | parity |
| Runs locally / privacy                  | mostly server | yes (in-browser wasm, nothing uploaded) | **ahead** |

## In-model gaps closed this pass

- Added `keyword_case = preserve` beyond the usual upper/lower, so users can keep their
  original casing — a small edge over the two-option competitors.
- Verified function calls stay tight (`COUNT(*)`, no space before `(`) and quoted
  identifiers / bracket identifiers (`"x"`, `` `x` ``, `[x]`) are preserved verbatim.

## Out-of-model / deliberately not built

- **Function-case / datatype-case as separate axes** (Aiven). gizza re-cases all
  recognized SQL keywords — which includes the common aggregate functions
  (COUNT/SUM/AVG/MIN/MAX/COALESCE/CAST) — under one `keyword_case` control. Splitting
  function vs. datatype vs. keyword into independent toggles is a low-value UX expansion
  and was left out to keep the schema small; the single control covers the common need.
- **Minify / compress SQL** (CodeBeautify, FastTool). This is the inverse operation and
  belongs in its own tool, not as a mode here.
- **`.sql` file upload** (CodeBeautify). The page already accepts pasted multi-line SQL
  via a textarea; a binary/file-input surface for plain text is redundant.
- **Syntax highlighting** (FastTool). The gizza page output is plain text; highlighting
  is a presentation concern outside the formatter's compute model.
- **Per-dialect profiles** (Devart/Redgate). gizza is intentionally dialect-agnostic; a
  token-stream reformatter avoids the maintenance cost of N grammars while still handling
  PostgreSQL/MySQL/SQLite/SQL-Server/etc.

## Verification

- `cargo test --workspace` — 12 core + 1 drift schema test pass.
- `wafer build` — chat block.wasm builds and instantiates.
- `wasm-pack build … web` + generator — page renders with 3 inputs (sql textarea, indent
  field, keyword_case select).
- CLI: upper (default), lower+indent=4, preserve, and the empty-input error path all
  verified.
- Playwright: 2 specs (clause layout + uppercasing; lower-case + indent=4) pass.
