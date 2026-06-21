# csv-stats — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/csv-stats` — per-column summary statistics for a CSV (count,
empty, distinct; min/max/mean/sum for numeric columns). Pure-Rust (`csv`).
Pure-text input → text/data output: chat + CLI + a page. Complements the csv-*
family (group-by aggregates *by key*; csv-stats summarises *every column at once*).

## What competitors do

- **`pandas df.describe()` / R `summary()`** — the gold standard, but need a
  Python/R environment and code.
- **`csvstat` (csvkit)** — exactly this, locally, but a native install + CLI.
- **Online CSV analysers** — upload a CSV, see stats; convenient but the data is
  uploaded and they're ad-supported.
- **Spreadsheets** — `COUNT`/`AVERAGE`/`MIN`/`MAX` per column, manual and tedious.

## How this tool competes / improves

1. **Runs locally + everywhere.** Pure-Rust compiled to wasm: chat, CLI, and an
   in-browser page. The CSV never leaves the device.
2. **Auto numeric vs text.** A column is treated as numeric only if **every**
   non-empty value parses as a number; numeric columns get min/max/mean/sum, text
   columns get count + distinct — no manual type hints.
3. **Counts what matters.** Per column: non-empty count, **empty-cell** count, and
   **distinct** values — so you immediately see missing data and cardinality.
4. **Delimiter- and quoting-correct** (`,` / tab / `;` / `|`, real CSV parser),
   with or without a header.
5. **Structured + same everywhere.** Chat/CLI return JSON (one object per column +
   row count); the page shows a readable per-column table.

## Honest scope

- **count / empty / distinct / min / max / mean / sum** — not median, stddev,
  quartiles, or histograms (kept lightweight; those could be a future addition).
- A column with any non-numeric value is reported as text (no partial numeric
  stats), matching `describe`-style behaviour for mixed columns.

## Tests

6 core unit tests: numeric vs text columns with correct count/distinct/min/max/
mean/sum; **empty cells** counted separately and excluded from distinct; no-header
mode uses `col1…` names; a mixed column is text (no sum); the summary text format;
and errors (empty input, bad delimiter). Plus the block drift-guard schema test.
**CLI verified** end-to-end. **Page** verified with Playwright. `wafer build`
instantiates the chat block (349 KiB).
