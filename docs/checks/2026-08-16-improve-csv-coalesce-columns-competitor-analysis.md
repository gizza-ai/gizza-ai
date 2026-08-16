# csv-coalesce-columns — competitor analysis (2026-08-16)

Scan run BEFORE finalising the tool, per `create-next-tool` step 4. Everything below is
paraphrased from public product pages, READMEs and reference articles; no competitor copy,
branding, trademark or example text is reproduced here or in the shipped tool.

Backlog row: `csv-coalesce-columns — Creates one column from the first non-empty value across a
list of source columns (in priority order), optionally dropping the sources. (pure)`

## Semantic-duplicate check (done first)

`ls blocks/ | grep -iE 'csv|column|merge|null|fill|combine'` returns ~90 blocks. The near
neighbours were read:

| Existing block | Scope | Overlap verdict |
| --- | --- | --- |
| `csv-fill-down` | Fills an empty cell from the last non-empty value **above it** in the same column | Vertical (previous row) fill; never looks sideways at sibling columns |
| `csv-null-standardizer` | Rewrites assorted missing-value tokens (`NA`, `null`, `-`, …) to one representation | Normalises what "empty" *looks like*; never merges two columns |
| `csv-column-split` | Splits one column into many, or concatenates several into one | Concatenation keeps **every** value (`a / b`); coalescing keeps exactly one |
| `csv-cleaner` | Trim / dedupe / drop-empty rows / fill / delimiter normalise | Fixed row-level transforms, no cross-column priority |
| `csv-insert-column` | Inserts a **constant** column | No source values involved |
| `csv-formula-eval` | Derives a column from a formula expression | A general expression engine, not a declarative priority list; the user has to hand-write nested conditionals |
| `column-math` | Arithmetic between two numeric columns | Numeric, two-column, never emptiness-driven |
| `csv-collapse-rows`, `csv-merge`, `csv-join`, `csv-anti-join` | Row grouping / file concatenation / key joins | Operate across rows or files, not across columns of one row |
| `json-field-coalescer` (backlog, not built) | Same idea for JSON objects | Different input model; stays a separate backlog row |

Conclusion: **not a duplicate.** The distinct capability is *row-wise first-non-empty selection
across a priority-ordered list of columns, emitted as one new column* — no existing block expresses
it without hand-written conditionals. Building it.

## Competitors reviewed

1. **SmartQueryTools — coalesce-columns-in-CSV page** — the only direct browser-based analogue
   found: paste or upload a CSV, tick the columns to merge, get one merged column back.
2. **`TataneSan/csv-coalesce-columns` (GitHub)** — a small command-line utility that reads a CSV
   and writes a coalesced column; the closest functional analogue with a real CLI.
3. **SQL `COALESCE` reference material** — vendor docs plus tutorial write-ups (DataCamp and
   similar) that define the semantics everybody else copies: return the first non-`NULL`
   argument, left to right; `NULL` if all are `NULL`.
4. **Spreadsheet patterns** — the nested `IF(A<>"", A, IF(B<>"", B, …))` / `IFS` idiom, and Power
   Query's column-merge step, which is how most people do this today.
5. **pandas patterns** — `Series.combine_first`, `df[cols].bfill(axis=1).iloc[:, 0]`, and chained
   `fillna` — the analyst-side reference implementation.

## Table-stakes matrix

| Capability | Seen at | Fit | Our decision |
| --- | --- | --- | --- |
| Pick the first non-empty value across N columns | all five | in-model | The core operation |
| **Explicit priority order** (left-to-right precedence) | 3, 4, 5 (implicit in 1, 2) | in-model | `columns` is an ordered, comma-separated list — the order *is* the priority, and the page label says so |
| Choose the source columns by header name | 1, 2, 4, 5 | in-model | `columns` accepts header names when `header=true` |
| Address columns without a header row | 2 | in-model | 1-based indices (`2,3,4`); a purely numeric token is always read as an index |
| Name the resulting column | 1, 2, 4, 5 | in-model | `output`, default `coalesced`; collision with a kept column is an error, not a silent overwrite |
| Keep or drop the source columns | 1, 2, 4 | in-model | `drop_sources`, default `false` (keep — non-destructive by default, so the result can be checked first) |
| Where the new column lands | 4 (Power Query replaces in place) | in-model | `position = end \| start \| first-source`; `first-source` + `drop_sources` reproduces the in-place replace |
| Default when every source is empty | 3 (SQL: a trailing literal), 5 (`fillna`) | in-model | `fallback`, default `""` — the equivalent of `COALESCE(a, b, 'N/A')` |
| Treat whitespace-only cells as empty | 4, 5 (users hit this constantly) | in-model | `blank_is_empty`, default `true`; `false` restores strict zero-length-only semantics |
| Treat `NULL` / `NA` / `-` placeholders as empty | 1, 5 (`na_values`) | in-model | `null_tokens`, default `""` (off); comma-separated, matched case-insensitively on the trimmed cell |
| Header row present / absent | 2, 5 | in-model | `header`, default `true`; the header row is rewritten with the new column |
| Delimiter choice (comma / tab / semicolon / pipe) | 1, 2 | in-model | `delimiter`, default `,`; a single character or a `comma`/`tab`/`semicolon`/`pipe` word |
| Quoting / embedded newlines survive | 1, 2, 5 | in-model | Full CSV parse + re-emit; values are copied verbatim and re-quoted as needed |
| Ragged / short rows | 5 | in-model | Padded with empty cells, so a missing trailing column simply falls through to the next source |
| Worked example / preset buttons | 1, 4 | in-model | Four `[[example]]` chips (best phone number, replace-the-sources, fallback, `NULL`/`N/A` placeholders) |
| Several coalesced columns in one run | 1 (multi-group UI), 5 | out-of-model | Our surfaces return one output value from one declarative call; run the tool twice, or chain it. Listed, not hidden |
| File upload / drag-drop / multi-MB files | 1 | out-of-model | This is a paste-in text tool; `xlsx-to-csv` handles workbooks. Everything runs locally instead |
| Interactive column-picker grid with a live preview of the merged column | 1, 4 | out-of-model | The generated page renders plain labelled fields; the ordered `columns` box plus the always-on output panel is the in-model substitute |
| Provenance column (*which* source won for this row) | — (gap in all five) | in-model, declined | Genuinely useful, but it doubles the output shape and the width contract; recorded here for a follow-up rather than smuggled in |
| Type-aware emptiness (`0`, `false`, `NaN` count as null) | 5 (`NaN` only) | in-model, declined | Emptiness stays purely textual and documented as such; `null_tokens` covers the real-world cases without guessing types |
| Negative indices (`-1` = last column) / `2-4` ranges | 2 | in-model, declined | A coalesce list is short and order-sensitive by nature; ranges would obscure the priority the whole tool is about |
| Non-UTF-8 / raw-byte input | 2 | out-of-model | The block boundary is a UTF-8 string; `text-encoding-converter` transcodes first |
| Streaming multi-GB files | 2 | out-of-model | In-browser, whole-document compute by design |

Every table-stake above ends in the descriptor or in the out-of-model / declined list — none dropped
silently.

## Resulting parameter set and defaults

| Param | Type | Default | Why this default |
| --- | --- | --- | --- |
| `data` | string | required | The CSV text |
| `columns` | string | required | Ordered priority list; there is no sensible default guess |
| `output` | string | `""` → `coalesced` | Matches the SQL-ish vocabulary users already searched for |
| `position` | enum | `end` | Appending never disturbs existing column positions |
| `fallback` | string | `""` | Mirrors SQL: all-`NULL` in → `NULL` out |
| `drop_sources` | bool | `false` | Non-destructive by default |
| `blank_is_empty` | bool | `true` | A stray space winning the coalesce is never what anyone wants |
| `null_tokens` | string | `""` | Opt-in, because `NULL` is a legitimate value in some datasets |
| `header` | bool | `true` | The overwhelmingly common CSV shape, and it enables name addressing |
| `delimiter` | string | `,` | Comma-first, with the three usual alternates spelled out |

Error paths are explicit rather than silent: unknown column name, duplicate entry in `columns`,
index past the last column, invalid `position`, invalid delimiter, output-name collision, and empty
input each return a message naming the offending token.

## Gaps we close that the competitors do not

- **Priority is a first-class, visible concept.** The picker-style tools take a set of ticked
  columns; ours takes an ordered list, and both the label and the docs say the order decides.
- **Emptiness is configurable.** Whitespace-only cells and placeholder tokens (`NULL`, `N/A`, `-`)
  can be folded into "empty" — the difference between a clean merge and one poisoned by `"NULL"`
  strings that no browser tool we found handles.
- **A real fallback.** SQL gets it for free with a trailing literal; the browser tools do not
  expose it.
- **Placement control.** `first-source` + `drop_sources` gives the in-place replace that the
  spreadsheet workflow does manually, without shuffling columns afterwards.
- **Three deterministic surfaces.** Chat skill, CLI and page share one core, so the same arguments
  give byte-identical output; only competitor 2 has a CLI at all, and it is install-required.
- **Nothing is uploaded.** Competitor 1 is a hosted service; this runs as WebAssembly in the page.

## Notes

- Only emptiness decides the winner — a cell containing `0` or `false` is a real value and wins.
  This matches SQL `COALESCE` (which only skips `NULL`) and is documented on the page so nobody
  expects falsy-value skipping.
- Values are copied verbatim, never trimmed or reformatted, so a picked value round-trips exactly
  (quotes, inner commas, unicode).
- Coalesce vs concatenate is the single most common conceptual mix-up in the source material; the
  page FAQ answers it head-on.
