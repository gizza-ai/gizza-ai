# fuzzy-csv-join — competitor analysis (2026-08-06)

Scan run **before** implementing, per `create-next-tool` step 4. All findings are paraphrased
from public documentation; no competitor copy, branding, or trademarks are reused.

## Scope

`fuzzy-csv-join` joins two CSV tables on a key column using **approximate** string matching
instead of exact equality, and surfaces both the matched pairs and the rows that found no
partner. The nearest existing gizza blocks were checked first and are **not** duplicates:

- `csv-join` — same two-table join, but exact equality only (`case_sensitive` is its only
  fuzziness). No similarity algorithm, no score, no threshold.
- `fuzzy-dedupe`, `cluster-similar-values`, `fuzzy-name-matcher` — all operate on **one**
  table/list (dedupe, cluster, entity-resolve). None joins two tables or emits joined rows.

## Competitors reviewed

| # | Tool | Shape | Notes |
|---|------|-------|-------|
| 1 | csvmatch (maxharlow) | Python CLI | Two CSVs in, one CSV out; several algorithms; join modes. |
| 2 | fuzzyjoin (PyPI) | Python CLI | Single-column-per-side join, n-gram blocking, edit distance. |
| 3 | Power Query fuzzy merge (Excel / Power BI) | GUI merge dialog | The mainstream GUI reference for this operation. |

### 1. csvmatch

- Column selection per side (`--fields1` / `--fields2`), defaulting to all columns.
- Comparison methods: literal (exact after normalisation, the default), Levenshtein (Damerau
  variant), Jaro-Winkler, a Double-Metaphone phonetic mode, plus an interactive
  machine-learning mode (`bilenko`) built on the Dedupe library.
- `--ignore` normalisations: case, non-alphanumeric characters, non-Latin characters,
  leading/trailing words, word order, titles, custom regex.
- `--threshold` in 0.0–1.0, default 0.6.
- `--join`: inner (default), left-outer, right-outer, full-outer.
- `--output` chooses which columns land in the result, including `degree` (the match score).

### 2. fuzzyjoin

- `-f/--fields` names exactly one column per side (the common case it optimises for).
- Normalised Levenshtein edit distance; `-t/--threshold` default 0.7.
- N-gram blocking (`--ngram-size`, default 3) to avoid a full cross product.
- Rows with **multiple** candidate matches can be written to a separate file (`--multiples`),
  i.e. the tool treats "how many matches per row" as a first-class concern.
- Number-handling modes (exact / permutation / subset) for keys containing digits.

### 3. Power Query fuzzy merge

- Jaccard-style similarity over text columns; **similarity threshold 0.00–1.00, default 0.80**.
- **Ignore case** toggle.
- **Match by combining text parts** (so `Micro soft` can match `Microsoft`).
- **Show similarity scores** toggle — adds a score column to the merged output.
- **Number of matches** — caps how many right rows a single left row may match.
- **Transformation table** — a user-supplied from→to mapping applied before comparison.
- Reports coverage back to the user ("N of M rows matched"), i.e. unmatched rows are the
  headline diagnostic.

## Table stakes → decisions

| Capability | Seen in | Decision |
|---|---|---|
| Key column per side, by name **or** position | 1, 2 | **In** — `left_key` / `right_key`, header name or 1-based index; blank right key reuses the left reference (matches sibling `csv-join`). |
| Similarity threshold | 1, 2, 3 | **In** — `threshold`, 0–100 integer, default **85**, rendered as a page slider. Expressed 0–100 rather than 0.0–1.0 for consistency with the sibling fuzzy blocks; 85 sits between the 0.7/0.8 defaults of (2)/(3) and the stricter needs of a join key. |
| Multiple algorithms | 1, 2 | **In** — `algorithm` enum: `levenshtein`, `jaro_winkler` (default), `token_sort`, `soundex`. |
| Phonetic matching | 1 | **In** as `soundex` (Double Metaphone is out; Soundex is already proven wasm-safe in `fuzzy-name-matcher` and covers the same "sounds alike" case). |
| Word-order-insensitive matching / "combine text parts" | 1, 3 | **In** as the `token_sort` algorithm (tokens sorted before comparison, so `Acme Ltd`/`Ltd Acme` and split words align). |
| Ignore case | 1, 3 | **In** — `normalize_case`, default on. |
| Ignore punctuation / non-alphanumerics | 1 | **In** — `ignore_punctuation`, default off. |
| Join modes (inner/left/right/outer) | 1, 3 | **In** — `join_type`, default `inner`. |
| Show similarity score in the output | 1, 3 | **In** — `show_score`, default on, appends a `match_score` column. |
| Cap matches per left row | 2, 3 | **In** — `max_matches`, default 1 (best match wins), up to 100. |
| Surface unmatched rows explicitly | 2, 3 | **In** — `output` enum adds `unmatched_left` / `unmatched_right` views plus a `json` report carrying both lists and coverage stats. This is the differentiator named in the tool's own description. |
| Custom delimiter | — | **In** — `delimiter` (single char or comma/tab/semicolon/pipe), matching sibling CSV blocks. |
| Preset examples on the page | 3 (GUI presets) | **In** — `[[example]]` chips for the typical flows (typo join, phonetic, keep-all-left, unmatched report). |

### Out of model (listed, not built)

- **Interactive machine-learning record linkage** (csvmatch's `bilenko`/Dedupe mode) — needs a
  trained model plus an interactive labelling loop; gizza blocks are pure Rust, deterministic
  and non-interactive.
- **Double Metaphone** specifically — Soundex covers the phonetic case; adding a second
  phonetic encoder is capability duplication, not a gap.
- **Transformation table** (Power Query) — a synonym mapping is a separate preprocessing step;
  it would need a third tabular input, which the single-run page form does not model.
- **N-gram blocking / index-based candidate generation** (fuzzyjoin) — an internal performance
  optimisation, not user-visible behaviour. Instead the block does an exact full comparison and
  caps each side at 2,000 data rows, which keeps worst-case work bounded and predictable in the
  browser. Stated on the page as a documented limit.
- **Parquet input** (csvmatch) — out of scope for a CSV tool; a separate converter block already
  covers columnar formats.
- **Multi-column composite keys** — deliberately deferred: it multiplies the parameter surface
  (per-column weights, per-column algorithms) for a case a user can pre-solve by concatenating
  columns. Noted here so it is not silently dropped.

## Sources

- <https://github.com/maxharlow/csvmatch>
- <https://pypi.org/project/fuzzyjoin/>
- <https://learn.microsoft.com/en-us/power-query/merge-queries-fuzzy-match>
