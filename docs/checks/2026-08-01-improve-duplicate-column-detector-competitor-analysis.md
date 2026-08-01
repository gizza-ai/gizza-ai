# Competitor analysis — duplicate-column-detector (2026-08-01)

Tool function: find columns in a CSV/table whose values are identical (optionally
regardless of the header name) and report or remove the redundant copies. This is
the **column**-oriented counterpart to the existing row tools (`duplicate-row-finder`,
`csv-dedupe`, `fuzzy-dedupe`) — none of which detect duplicate columns.

## Landscape

Two distinct competitor classes exist; neither ships a dedicated "duplicate column"
web tool, so the table-stakes come from the pandas how-to pattern (the canonical
implementation everyone points to) plus the general online CSV-dedup UX conventions.

### 1. pandas / dataframe duplicate-column pattern (canonical function reference)
- geeksforgeeks "How to Find & Drop duplicate columns in a Pandas DataFrame"
- statology "How to Drop Duplicate Columns in Pandas"
- pandas `DataFrame.T.drop_duplicates().T` idiom

Extracted behaviour (paraphrased):
- A duplicate column is determined by an **exact value match across all rows,
  regardless of column name** (`df.T.drop_duplicates().T` / column-wise `.equals()`).
- **Keeps the first occurrence** and drops later duplicates.
- A separate variant (`df.columns.duplicated()`) matches on **name** only — i.e.
  the "name must also match" mode.
- Returns the **names of the dropped columns** for reporting.
- No case/whitespace normalization in the base pattern.

### 2. online CSV duplicate removers (UX conventions — all row-oriented)
- csvtools.com/remove-duplicates, csvduplicateremover.com, ivandt, doathingy,
  datablist. All operate on **rows**, but establish the shared UX vocabulary:
- Entire-row vs selected-columns matching; **keep first vs last occurrence**;
  header-row preservation; case-insensitive + trim-whitespace toggles; skip empty
  lines; copy-to-clipboard / download CSV output; **client-side, no upload**.

## Table-stakes → decisions

| Capability | Seen in | In/out of model | Decision |
| --- | --- | --- | --- |
| Detect columns with identical values regardless of name | pandas transpose | in-model | Core detection; `ignore_header_name` default true |
| Require the header **name** to also match | `df.columns.duplicated()` | in-model | `ignore_header_name=false` mode |
| Keep first occurrence, list which columns are redundant | pandas | in-model | Report + json name the kept vs dropped column |
| **Remove** redundant columns → emit de-duplicated CSV | pandas `T.drop_duplicates().T` | in-model | `output=csv` |
| Case-insensitive comparison | CSV removers | in-model | `ignore_case` default true (family norm) |
| Trim / collapse whitespace | CSV removers | in-model | `ignore_whitespace` default true (family norm) |
| Delimiter (comma/tab/semicolon/pipe/char) | family + CSV tools | in-model | `delimiter` |
| Header-row on/off | all | in-model | `header` default true |
| Structured output for scripting | (json convention) | in-model | `output=json` |
| Copy / download result | CSV removers | in-model | Provided generically by the page (Copy + Download) |
| Client-side, no upload | all | in-model | Native — runs as browser wasm |
| Keep **last** occurrence instead of first | csvtools | in-model, **rejected** | Keeping the first (leftmost) column is the single well-defined, order-stable convention pandas uses; a keep-last toggle adds schema surface for a rarely-meaningful choice when detecting *identical* columns (the copies are interchangeable). Documented as a considered rejection. |

## Out-of-model (listed, not built)
- Cloud batch / multi-file column reconciliation (needs a server/account).
- Fuzzy / typo-tolerant column matching beyond case+whitespace (would need an ML
  similarity model; gizza is pure-Rust — mirrors the row family's `fuzzy-dedupe`
  split, out of scope for an *identical*-column detector).

> Original work only — no competitor copy, branding, or trademarks reproduced.
