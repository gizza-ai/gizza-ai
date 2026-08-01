# sql-linter — competitor analysis (2026-07-31)

Scan run **before** implementation to fix the table-stakes feature set. All findings are
paraphrased from public docs; no competitor copy, branding, or trademarks are reproduced.

## Tool under build

`sql-linter` — "Parses SQL and flags syntax errors plus anti-patterns like SELECT \*, missing
aliases, and implicit joins." Pure-Rust, browser-local, no database connection, never executes SQL.

## Competitors inspected

### 1. SQLFluff (modular SQL linter + auto-formatter)
- Rule-code model (`AL03`, `AL10`, `AM04`, `AM05`, `AM08`, `RF02`, `CP01` …); each violation
  reports line/position + rule code + description; auto-fix for many rules.
- Relevant rules: `AM04` flags `SELECT *` (unknown/So unstable column count); `AL10` derived
  tables must have an alias; `AL03` column expressions without an alias (`allow_scalar` option);
  `AM05` a `JOIN` with no explicit type (recommends `INNER JOIN`); `AM08` implicit cross join
  (JOIN with no `ON`); `RF02` unqualified references when >1 table is referenced; `CP01` keyword
  casing.
- Dialect-flexible (30+ dialects). Online demo: dialect `<select>`, a "paste your SQL" textarea,
  a submit button, and a violations table (Rule / Description columns).

### 2. sqlcheck (SQL anti-pattern detector CLI)
- Three-tier **risk model**: HIGH / MEDIUM / LOW, with per-severity summary counts.
- 16 query anti-patterns incl. `SELECT *` (pulls unneeded columns, defeats covering indexes,
  more network traffic), implicit column usage, "spaghetti" many-join queries.
- Reports: matched SQL statement + risk + category + explanation + matched expression + summary.
- CLI flags: `-r/--risk_level` (1 = all, 2 = medium+high, 3 = high only), `-c/--color_mode`,
  `-v/--verbose_mode`, `-f/--file_name`. Input = SQL file, output = console report.

### 3. SQL anti-pattern references (boralp/sql-anti-patterns, sonra.io, MariaDB "comma vs JOIN")
- Comma / implicit join (`FROM a, b WHERE a.id = b.id`) is a widely-cited anti-pattern:
  join conditions get buried in `WHERE`, INNER↔OUTER can't be swapped by a keyword, and vendors
  are deprecating the old syntax. Explicit `JOIN … ON` is the recommended form.
- Unqualified column references / missing table aliases hurt readability and risk ambiguity once
  more than one table is in play.

## Table-stakes → gizza mapping

| Capability | Source | In-model? | Decision |
|---|---|---|---|
| Flag `SELECT *` (incl. `t.*`) | SQLFluff AM04, sqlcheck | yes | **built** — `SELECT-STAR` (warning) |
| Flag implicit / comma join | anti-pattern refs, AM08 | yes | **built** — `IMPLICIT-JOIN` (warning) |
| Flag derived table without alias | SQLFluff AL10 | yes | **built** — `SUBQUERY-NO-ALIAS` (warning) |
| Flag bare `JOIN` (no INNER/LEFT/…) | SQLFluff AM05 | yes | **built** — `BARE-JOIN` (info) |
| Real syntax errors w/ location | SQLFluff, parsers | partial | **built** — heuristic structural checks: unbalanced parens, unterminated string / block comment, leading / trailing commas (severity error) |
| Dialect selector | SQLFluff, sqlcheck | yes | **built** — `dialect` (generic/mysql/postgresql/sqlite/tsql); MySQL `#` line comments honoured in masking |
| Severity filter | sqlcheck `-r` | yes | **built** — `min_severity` (all/warning/error) |
| Suppress known-intentional categories | (linter `# noqa`) | yes | **built** — `ignore` comma-list of categories |
| Machine-readable output | SQLFluff `--format json` | yes | **built** — `format` text/json with per-severity summary counts |
| Per-finding line number | SQLFluff, sqlcheck | yes | **built** — each finding carries its own line |
| Auto-fix / rewrite | SQLFluff | partial | **out-of-model** — this is a read-only linter (see sql-formatter for pretty-printing); rewriting SQL safely needs a full parser + dialect model. Listed, not built. |
| Unqualified-reference check (RF02) | SQLFluff | partial | **considered, rejected** — accurate detection needs full name resolution across the FROM tables; a regex version false-flags aliased/qualified refs. Covered indirectly by IMPLICIT-JOIN + SUBQUERY-NO-ALIAS. |
| Keyword-casing / whitespace style (CP01) | SQLFluff | yes but | **out-of-scope** — pure formatting, already owned by `blocks/sql-formatter`. |
| 30+ dialects, dbt/Jinja templating | SQLFluff | no | **out-of-model** — needs a templating engine + huge dialect grammar set. Listed, not built. |

## Notes on distinctness (dedup)

- `sql-danger-checker` flags *destructive* statements (DROP/TRUNCATE/WHERE-less DELETE) — a safety
  gate, not a style/quality linter. No overlapping rules.
- `sql-injection-scanner` scans *host-language code* for injection-prone query construction, not SQL
  quality.
- `sql-formatter` pretty-prints; it does not detect anti-patterns or syntax errors.

`sql-linter` occupies the code-quality / correctness lane none of them cover.
