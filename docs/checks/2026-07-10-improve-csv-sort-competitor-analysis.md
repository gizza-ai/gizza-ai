# csv-sort — competitor analysis (2026-07-10)

`/create-next-tool` backlog pick. Pure-Rust (`csv` crate) text tool, all 3
surfaces (chat/LLM API, CLI, page). Survey paraphrased — no competitor copy,
branding, or trademarks reproduced.

## Top 3 competitors surveyed (general landscape)
| # | tool type | does well (paraphrased) | dimension |
| - | --------- | ----------------------- | --------- |
| 1 | online "sort CSV by column" web tools | pick a column, ascending/descending, header-aware, in-browser | capabilities / UX |
| 2 | spreadsheet sort (multi-level sort dialog) | sort by several columns in priority order, each with its own direction, number-vs-text handling | capabilities |
| 3 | command-line `sort`-style CSV utilities | numeric vs lexical keys, custom delimiter, index-based columns, stable ordering | capabilities |

## Gap diff vs our tool
Our tool: sort CSV/TSV rows by one or more `columns` (header names or 1-based
indices) in priority order; per-column `:asc`/`:desc` overrides plus a global
`order`; `numeric` = auto (numeric when a column is all numbers, else lexical) /
number / text; `case_sensitive` for text ordering; header row preserved on top;
configurable `delimiter`. Sort is stable. Covers the core sort feature set of all
three competitor classes.

**In-model (delivered):**
- Multi-column priority sort — **done** (comma-separated `columns`).
- Per-column ascending/descending — **done** (`:asc`/`:desc` suffix + global `order`).
- Numeric-aware vs lexical ordering — **done** (`numeric` = auto/number/text).
- Case sensitivity toggle for text — **done** (`case_sensitive`).
- Header-aware naming + index addressing without a header — **done** (`header`).
- Delimiter support (comma/tab/semicolon/pipe/any char) — **done** (`delimiter`).

**In-model gaps considered, deferred (minor):**
- **Sort by cell length / natural (alphanumeric) order** — niche key types beyond
  numeric/lexical; would bloat the `numeric` enum for a small audience.
- **Locale-aware collation** — depends on ICU/locale data (heavier, separate concern).
- **"Keep header row out of the sort but move it to bottom"** style toggles — the
  single header-on-top behavior matches the common case.

**Out-of-model:** cloud/server batch sorting of huge files, accounts, and saved
sort presets — need a backend; gizza is browser-local, no account.

## In-model vs out-of-model decision summary
- **In-model:** everything expressible as a pure-Rust transform over the pasted
  CSV — multi-key sort, direction, numeric/text ordering, case sensitivity,
  delimiters, index/name columns. All shipped in the descriptor.
- **Out-of-model:** anything needing a server, login, or persisted state (batch
  pipelines, saved presets) — listed, not built.

## Tested
unit (13: name asc, numeric auto beats lexical, descending, multi-column
per-col direction, index no-header, text forces lexical, case-insensitive
default, case-sensitive, tab delimiter, blanks after numbers, stable ties,
errors ×6) + drift-guard schema test · wafer fixture (1) · `wafer build` ·
wasm-pack web · generator · CLI (exact-output sort by `age`) · Playwright page
(numeric sort, multi-column, query-param deep-link — 3) · hygiene gate.

> Original work only — no competitor copy, branding, or trademarks copied.
