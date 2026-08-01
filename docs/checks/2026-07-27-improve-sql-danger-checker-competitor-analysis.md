# SQL Danger Checker — Competitor Research (2026-07-27)

## 1. Competitor Table

| Tool | URL | What it does |
|---|---|---|
| Squawk | https://squawkhq.com | CLI/CI linter for Postgres migrations flagging locking/destructive DDL |
| strong_migrations | https://github.com/ankane/strong_migrations | Rails gem that intercepts and blocks unsafe migrations before they run |
| SQLFluff | https://www.sqlfluff.com | Dialect-flexible SQL linter/formatter with configurable rules |
| Redgate SQL Prompt | https://www.red-gate.com/products/sql-prompt/ | SSMS add-in with "Execution Warnings" prompting before risky statements |
| sql-lint | https://sql-lint.readthedocs.io | Lightweight MySQL/Postgres linter that flags missing-WHERE and errors |

## 2. Feature Matrix

| Capability | Squawk | strong_migrations | SQLFluff | SQL Prompt | sql-lint |
|---|---|---|---|---|---|
| UPDATE/DELETE w/o WHERE | – | – | via rules | ✅ | ✅ |
| DROP TABLE/COLUMN | ✅ | ✅ | – | (warn) | – |
| TRUNCATE | partial | – | – | – | – |
| Unsafe ALTER / NOT NULL / constraints | ✅ | ✅ | – | – | – |
| Index without CONCURRENTLY | ✅ | ✅ | – | – | – |
| Severity levels (warn/error) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Multi-dialect | PG only | PG/MySQL | 17+ dialects | SQL Server | MySQL/PG |
| Allowlist / disable rule / inline ignore | ✅ | ✅ (safety_assured) | ✅ (noqa) | ✅ | ✅ |
| Confirmation prompt (interactive) | – | blocks w/ guidance | – | ✅ dialog | – |
| CI / GitHub PR comments | ✅ | – | ✅ | – | – |

## 3. Table-Stakes Features

- Detect **UPDATE/DELETE without WHERE** (the flagship case) — and treat a `WHERE 1=1` / always-true predicate as still risky.
- Detect **DROP TABLE/DATABASE/COLUMN, TRUNCATE**, and destructive `ALTER` (drop/rename column, change type).
- **Severity classification** (info / warning / danger) rather than binary pass/fail.
- **Parse multiple statements** in one script and report per-statement with line location.
- Human-readable and machine-readable (JSON) output.
- **Allowlisting / suppression** so intentional full-table ops aren't blocked forever.
- **Dialect awareness** (at minimum Postgres + MySQL keyword/quoting differences).
- Clear message per finding: what's risky, why, and the safer alternative.

## 4. Differentiators / Nice-to-Haves

- **"Blast radius"** framing — state plainly "affects ALL rows" for a WHERE-less statement.
- **Strict mode vs. advisory mode** — flag every destructive statement (even guarded ones) for confirmation.
- Transaction/rollback hints — suggest wrapping in a transaction, taking a backup.
- Comment/string-aware parsing so a `DROP` inside a comment or string literal isn't false-flagged.
- **Paste-and-scan web UI** with zero setup, no DB connection required — a clear gap; most competitors are CLI/IDE/CI-bound.

## 5. Copy / UX Ideas (generic)

- Frame around **prevention of accidental data loss**: catch the DELETE that hits every row before it reaches the database.
- Use a severity system (critical / high / medium / low) with a clear tag per finding.
- Show findings per statement with the offending statement echoed and a one-line "why this is dangerous."
- Position as a fast **pre-flight check** — not a replacement for backups or permissions.
- Emphasize **no connection / no credentials** — it only reads the text, never touches the database.

## Fit-to-model decisions (this pure, in-browser tool)

- **Built:** UPDATE/DELETE-without-WHERE (incl. always-true `WHERE 1=1`/`WHERE true`), DROP DATABASE/SCHEMA/TABLE (critical), DROP of other objects + `ALTER … DROP` (high), other ALTER (medium), TRUNCATE (critical); multi-statement split; comment/string-aware masking; severity + `min_severity` filter; `strict` confirmation mode; `allow` suppression list; dialect knob (MySQL `#` comments); text + JSON output.
- **Out-of-model (documented, not built):** live row-count/blast-radius estimation (needs a DB connection), lock/downtime analysis, auto-fix rewrites, CI exit codes/PR comments, GitHub integration. These require infrastructure a pure in-browser tool doesn't have.
