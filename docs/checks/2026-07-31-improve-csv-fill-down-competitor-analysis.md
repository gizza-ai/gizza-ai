# csv-fill-down — competitor analysis (2026-07-31)

Function: forward-fill (spreadsheet "fill down") empty cells in chosen CSV columns
with the last non-empty value above them. Paraphrased research; no competitor copy,
branding, or trademarks reproduced.

## Competitors scanned

### 1. Doathingy — "Fill Incomplete Records"
- **Features:** four per-column strategies — forward fill (carry last non-empty value
  down), backward fill (carry next value up), fixed custom value, and column average
  (numeric mean).
- **Params/defaults:** each column configured independently via a dropdown; no delimiter
  or explicit header control surfaced.
- **Edge case stated:** leading empty cells with no preceding value stay empty on a
  forward fill.
- **UX:** file upload, per-column strategy dropdown, preview before download, no account.
- **Classification:** forward/backward direction → **in-model**; per-column *different*
  strategies + column-average imputation → **out-of-model** (schema bloat / separate
  imputation concern).

### 2. QuickTable — "Fill blank cells with above value"
- **Features:** fills null/empty cells in a column with the nearest non-empty value above
  (forward fill), online, no coding, no file-size cap advertised.
- **UX:** column-oriented, instant result. (Site host was unreachable at scan time —
  ENOTFOUND — so profile is from the search snapshot; noted honestly.)
- **Classification:** forward fill on chosen column → **in-model**.

### 3. Microsoft Power Query — "Fill values in a column"
- **Features:** **Fill Down** (replace nulls with the last non-empty value above,
  row-by-row until a new value appears) and **Fill Up** (replace nulls with the next
  non-empty value below). Applied per selected column.
- **Edge case stated:** distinguishes truly *empty* string cells from *null* — empties
  must be converted to null first before fill up/down acts on them.
- **UX:** right-click a column → Fill → Down/Up; desktop app, not browser-local.
- **Classification:** down/up direction + per-column targeting → **in-model**; the
  empty-vs-null distinction is a spreadsheet-engine detail — we treat any blank/whitespace
  cell as fillable (documented on the page).

## Gap list vs our tool (as built)

| gap | dimension | fit | action |
| --- | --- | --- | --- |
| Direction: fill down **and** fill up | capability | in-model | shipped as `direction` enum (down/up) |
| Choose which columns to fill (names or indices), blank = all | capability | in-model | shipped as `columns` |
| Header row kept + name-matchable | capability | in-model | shipped as `header` |
| Delimiter (comma/tab/semicolon/pipe/char) | capability | in-model | shipped as `delimiter` |
| Leading/trailing empties with no source value stay empty | copy/edge | in-model | documented on page + covered by a test |
| Fixed custom fill value | capability | out-of-model here | that is a distinct concern — `csv-cleaner` already fills blanks with a constant; listed, not duplicated |
| Column-average / mean imputation | capability | out-of-model | numeric imputation is a different tool; considered, not built |
| Per-column *different* strategy in one pass | capability | rejected | schema/UX bloat for the common case; run the tool twice with different `columns` instead |

## Positioning
Browser-local, no upload, free, no account. Matches the direction + column-selection
table-stakes of the strongest competitors while staying a single-purpose fill-down/up
transform (constant-value and mean imputation stay out of scope by design).
