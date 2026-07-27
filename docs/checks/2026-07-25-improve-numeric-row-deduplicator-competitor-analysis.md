# Competitor analysis — numeric-row-deduplicator (2026-07-25)

Scan done BEFORE implementation. Sources are paraphrased; no competitor copy, branding, or
trademarks are reproduced. Goal: identify table-stakes params/defaults/UX so ours ships complete,
and separate what fits gizza's browser-local pure-Rust model from what does not.

## Competitors surveyed

1. **Sheetgo — Remove Duplicates** (sheetgo.com) — CSV/Excel/Sheets dedupe. Check all columns or a
   subset; choose keep-first vs keep-last; browser-side, no upload.
2. **Ivandt — Remove CSV Duplicates** (ivandt.com) — in-browser CSV dedupe; pick key columns;
   normalization toggles (case-insensitive, trim spaces).
3. **CSV Duplicate Remover** (csvduplicateremover.com) — case sensitivity, column selection,
   keep-first vs keep-last.
4. **Gizmoop — Remove Duplicate Lines** (gizmoop.com) — list/line dedupe with case toggle,
   whitespace trim, keep first/last, no signup.
5. **PicoToolkit — Remove Duplicates** (picotoolkit.com) — keeps the earliest occurrence, drops
   later matches.

## Table-stakes (common across competitors) → decision

| Capability | Competitors | Our decision |
|---|---|---|
| Remove exact duplicate rows, preserve order | all | **in** — core dedupe, keep-first default |
| Key on a subset of columns | Sheetgo, Ivandt, CSV-DR | **in** — `columns` (1-based indices or header names) |
| Keep first vs keep last occurrence | Sheetgo, CSV-DR, Gizmoop | **in** — `keep` enum (first/last) |
| Configurable delimiter | CSV tools | **in** — `delimiter` (char or comma/tab/semicolon/pipe) |
| Header row handling | CSV tools | **in** — `header` (kept, enables name keying) |
| Browser-local, no upload | all | **in** — gizza is wasm, no server |

## Our differentiator (why this is NOT csv-dedupe)

`blocks/csv-dedupe` compares rows as **raw strings** — `1.0`, `1.00`, `1`, and `1e0` are four
different rows to it and all survive. This tool compares each cell **by numeric value**, so those
four collapse to one duplicate. That is the whole point of a *numeric* row deduplicator and the
feature no generic CSV deduper on the list offers:

- **Numeric-value equality** — parse each cell as a number; equal values dedupe regardless of
  textual form (`+1`, `1`, `1.0`, `1e0`, `100e-2`→`1`). Non-numeric cells fall back to a trimmed
  string compare so mixed tables still work.
- **`precision` (rounding tolerance)** — round each numeric cell to N decimals before comparing, so
  float-noise near-duplicates (`0.30000000000000004` vs `0.3`) collapse. `-1` = exact numeric value.

## UX decisions (declarative controls / preset chips)

- `keep` → `Param::enumv` → `<select>` with friendly `[input.labels]`.
- `precision` → `Param::integer` (min -1, max 12); placeholder shows `-1`.
- `header`/`data`/`columns`/`delimiter` mirror the sibling CSV tools' proven controls.
- **Preset chips** (`[[example]]`) for the three headline cases: representation-agnostic dedupe,
  keyed dedupe on one column, and rounding-tolerance dedupe.

## Out-of-model / considered, not built

- **File upload / drag-drop of multi-MB CSV, XLSX parsing** — out of model (no file picker for pure
  text tools; paste is the surface). Listed, not built.
- **Fuzzy / near-duplicate string matching** — already covered by `blocks/fuzzy-dedupe`; out of
  scope here (this tool is exact numeric equality, not fuzzy).
- **Case-insensitive / whitespace normalization for text cells** — text cells are trimmed before
  compare; a full case-fold toggle belongs to the string-oriented dedupers
  (`csv-dedupe`, `duplicate-row-finder`) and would blur this tool's numeric focus. Considered,
  rejected to keep the schema tight.
