# csv-window-functions — competitor analysis (2026-07-26)

Tool: **csv-window-functions** — "Computes running totals, moving averages, lag/lead,
and rank over partitions of CSV data." Pure-Rust, chat + CLI + page.

## Scan

No dedicated single-purpose "CSV window functions" web utility dominates search; the
category is owned by the **SQL window-function** feature of BigQuery / Snowflake /
PostgreSQL and the many tutorials teaching it. The de-facto spec is therefore the SQL
`function() OVER (PARTITION BY … ORDER BY …)` model, which every source agrees on.
Surveyed (paraphrased only — no copy/branding reused):

1. **PostgreSQL / Snowflake / BigQuery window functions** (official docs + oneUp/Count
   tutorials) — the canonical semantics for `SUM() OVER (ORDER BY …)` running totals,
   `AVG() … ROWS BETWEEN n PRECEDING AND CURRENT ROW` moving averages, `RANK`/`DENSE_RANK`,
   `LAG`/`LEAD(col, offset)`.
2. **DataCamp / DbGate / DataLemur tutorials** — practical LAG/LEAD (offset default 1,
   NULL past the partition edge), running totals and moving averages within `PARTITION BY`
   groups.
3. **ThoughtSpot / Modern Age Coders / DriveDataScience guides** — the ROW_NUMBER / RANK /
   DENSE_RANK / LAG / LEAD family and the PARTITION BY + ORDER BY frame model.

## Table-stakes → decision

| Capability | Fit | Decision |
|---|---|---|
| Running total (cumulative SUM) | in-model | `function=running_total` |
| Moving average over last N rows | in-model | `function=moving_average` + `window` |
| LAG (value n rows back) | in-model | `function=lag` + `offset` |
| LEAD (value n rows ahead) | in-model | `function=lead` + `offset` |
| RANK (ties share rank, gaps) | in-model | `function=rank` |
| DENSE_RANK (ties share rank, no gaps) | in-model | `function=dense_rank` |
| ROW_NUMBER (sequential position) | in-model | `function=row_number` |
| PARTITION BY (per-group, rows preserved) | in-model | `partition_by` |
| ORDER BY within partition | in-model | `order_by` |
| Direction asc/desc | in-model | `descending` |
| Custom output column name | in-model | `output_column` |
| Column addressing by name or 1-based index | in-model | resolved in core |
| Delimiter comma/tab/semicolon/pipe | in-model | `delimiter` enum |

Every table-stake from the SQL window model lands in the descriptor above — none dropped.

## In-model but deliberately deferred (not built this pass, listed not hidden)

- `percent_rank`, `cume_dist`, `ntile(n)`, `first_value`/`last_value`/`nth_value` — all are
  pure-Rust feasible (same partition/order machinery) and are honest future additions; left
  out this pass to keep the surface focused on the four capabilities named in the description
  (running totals, moving averages, lag/lead, rank). No out-of-*model* items: everything here
  is deterministic pure compute with no I/O.
- Explicit SQL frame clauses (`ROWS/RANGE BETWEEN …`) — the moving average exposes the common
  trailing-N frame via `window`; arbitrary frames are out of scope for a focused tool.

## UX / controls

- `function` → `<select>` (`Param::enumv`) with friendly `[input.labels]`.
- `window` / `offset` → number fields (integers).
- `descending` → checkbox (default off).
- `delimiter` → `<select>` enum.
- `[[example]]` preset chips for the headline cases (running total, moving average, rank by
  partition, lag) — competitors ship canned examples in every tutorial; chips are our
  declarative equivalent.

## Differentiators (honest)

Runs fully client-side (WebAssembly, no upload), no SQL to write, addresses columns by header
name or index, and preserves every input row (window functions never collapse rows).
