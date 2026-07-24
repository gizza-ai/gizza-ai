# csv-anti-join — competitor analysis (2026-07-23)

Tool: `csv-anti-join` — "Returns the rows in CSV A whose key has no match in CSV B
(and optionally the reverse)." Pure, browser-local, no upload.

## Scope vs. existing gizza CSV tools (dup check)

- **`csv-join`** — SQL inner/left/right/outer join; the `left` join keeps *all* left rows
  AND appends the right file's columns. It cannot emit "only A's unmatched rows, A's
  columns only" — different output shape and intent. Anti-join is the SQL `LEFT ANTI JOIN`
  / `NOT IN` primitive, distinct from every `join_type` csv-join ships.
- **`csv-cell-diff`** — cell-level diff of two aligned tables; not key-based row membership.
- **`list-set-diff`** — set difference over line-based lists, not keyed CSV rows / multi-column.
- **`csv-dedupe` / `fuzzy-dedupe`** — dedupe within one file, not cross-file membership.
- **`csv-filter`** — predicate filter on one file.

Conclusion: **viable, not a dup.** Anti-join fills the "which rows are new / dropped
between two exports keyed on id/email/sku" workflow that none of the above cover.

## Competitors surveyed (paraphrased — no copy/branding reused)

1. **Datablist CSV Diff** — match rows by a stable key (id/email/sku/uuid); auto-suggests a
   likely key, allows overriding it and choosing **multiple key columns**; falls back to
   full-row compare. Reports added/removed/changed. Local, no upload.
2. **CSVKit CSV Diff** — key-column-aware row diff in the browser; highlights added/removed/
   changed; no upload/account.
3. **CSVTool CSV Compare** — match by a key column (ID/SKU/email) so re-sorted/re-exported
   files still compare correctly.
4. **DataflowMapper / AllFileTools CSV Diff** — added/removed/modified with summary counts;
   options for **delimiter, case sensitivity, whitespace ignoring**.
5. **Power Query "Merge (Left Anti)" / R `dplyr::anti_join` / pandas anti-join** — the
   reference semantics: keep rows of the left frame with **no** matching key on the right.

## Table-stakes → decision (every one lands in the descriptor or is listed here)

| Capability | Competitor | Decision |
|---|---|---|
| Match by key column (name or index) | all | ✅ `key` (+`key_b`, header name or 1-based index) |
| **Composite / multiple key columns** | Datablist | ✅ `key` accepts a comma-separated list |
| A-only anti join (rows in A not in B) | Power Query/dplyr | ✅ `direction=a-only` (default) |
| B-only / reverse | Power Query (right anti) | ✅ `direction=b-only` |
| Both sides (symmetric difference) | diff tools | ✅ `direction=both` (prepends a `_source` A/B column) |
| Custom delimiter | AllFileTools | ✅ `delimiter` (char or comma/tab/semicolon/pipe) |
| Case sensitivity | AllFileTools | ✅ `case_sensitive` (default true) |
| Whitespace ignoring | AllFileTools | ✅ `trim_keys` (trim surrounding whitespace on key values) |
| Duplicate-key handling | dplyr | ✅ B treated as a key SET (no Cartesian blow-up); all unmatched A dupes kept |
| Local, no upload | all | ✅ browser-local wasm |

## Considered, not built (out of model / rejected)

- **Auto-suggest the key column** (Datablist) — pleasant UX but heuristic; the descriptor
  makes `key` explicit which is clearer for the chat/CLI surface. Considered, rejected.
- **Cell-level "changed" reporting** — that is `csv-cell-diff`'s job; anti-join is row
  membership only. Out of scope.
- **Full-row compare with no key** — degenerate case; a user can set `key` to every column.
  Not a separate mode.
- **Cloud batch / files > memory / accounts** — out of model (browser-local only).

## Notes

- `direction=both` stacks A-only and B-only rows into one CSV with a leading `_source`
  column (`A`/`B`); B rows are mapped onto A's header positionally, so `both` assumes the
  two files share the same column layout (the "two exports of one table" case). Stated on
  the page.
