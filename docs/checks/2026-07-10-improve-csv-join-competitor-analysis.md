# csv-join — competitor analysis (2026-07-10)

Function: join two CSV files on a key column, SQL-style (inner / left / right / full outer).
All findings PARAPHRASED — no competitor copy, branding, or trademarks reproduced.

## Competitors surveyed

1. **1000freetools — CSV Join/Merge** (`1000freetools.com/csv-tools/csv-join-merge`)
   - Four SQL-style joins: inner, left, right, full outer.
   - Left/right key columns selected independently — matches on the key **values**, so the two key
     columns may have different names.
   - Overlapping (non-key) column names disambiguated with configurable `left_` / `right_` prefixes
     (e.g. `left_name`, `right_name`).
   - Keys matched **exactly, case-sensitive**; `"1"` and `"001"` are distinct (string compare).
   - Stated limits: two tables only, single-column key (composite needs preprocessing), duplicate
     keys yield a Cartesian product (every combination), ~50 MB/file performance ceiling.

2. **Easy Data Transform — Join CSV** (`easydatatransform.com/join_csv_files.html`)
   - Horizontal side-by-side merge on a common key column.
   - Toggles for "include top non-matching rows" and "include bottom non-matching rows" — i.e. the
     unmatched-left / unmatched-right dial that composes into left/right/outer joins.
   - Composite keys built by concatenating columns first; row-number column for positional joins.
   - Some auto-detection of the likely key column.

3. **Mighty Merge — horizontal join** (`mightymerge.io/merge-csv-files/`)
   - Three modes on the first file's rows: Normal (all first-file rows + appended second-file cols),
     Match (only matched rows), Filter (only first-file rows with NO match in the second).
   - Requires selecting a matching column identifier; needs ≥1 shared column.

4. **CombineCSV** (`combinecsv.com`)
   - Matches related rows across files by an identifier column (ID / Email / Username).
   - Defaults to an **outer** join — unmatched rows kept with blanks for the missing side.
   - Auto-detects comma / semicolon / tab delimiters. Up to 3 files; single match column per file.

## Table-stakes → gizza fit

| Capability | Competitor(s) | In gizza model? | Decision |
| --- | --- | --- | --- |
| Inner / left / right / full-outer join | 1000freetools, CombineCSV (outer) | ✅ pure | `join_type` enum, default `inner` |
| Independent left/right key columns (match on values) | 1000freetools, Easy Data Transform | ✅ pure | `left_key` + `right_key` (right defaults to left's name) |
| Key by header name OR position | (implicit) | ✅ pure | accept header name or 1-based index |
| Overlapping non-key columns disambiguated | 1000freetools | ✅ pure | collision-only `_right` suffix on right columns |
| Case-sensitive vs insensitive key match | 1000freetools (case-sensitive) | ✅ pure | `case_sensitive` bool, default true |
| Configurable delimiter (`,`/`;`/tab/pipe) | CombineCSV | ✅ pure | `delimiter` param (matches csv family) |
| Duplicate keys → Cartesian product | 1000freetools | ✅ pure | natural join semantics |
| Unmatched rows filled with blanks | CombineCSV | ✅ pure | key cell populated from the present side |

## Out-of-model / considered, not built

- **File upload widget + 50 MB batch files** — gizza pages take pasted text (textarea); large-file
  drag-drop upload is a server/UX pattern outside the browser-local paste model. Textarea handles
  realistic pasted datasets.
- **3+ file / multi-way joins** — competitors cap at 2–3; keep to the clean two-table join. Chain
  runs for more.
- **Auto key-column detection ("guessed for you")** — a heuristic guess; explicit key selection is
  unambiguous and scriptable across chat/CLI/page. Considered, declined (guessing hurts the API
  surface).
- **Composite / multi-column keys** — competitors also punt to preprocessing (concat columns
  first). Single-column key matches table stakes; concat upstream with csv-insert-column /
  csv-formula-eval. Considered, declined for schema simplicity.
