# csv-regex-replace — competitor analysis (2026-08-16)

Scan run BEFORE implementing, per `create-next-tool` step 4. All findings are paraphrased from
public product pages and READMEs; no competitor copy, branding or trademarks are reproduced here or
in the shipped tool.

Backlog row: `csv-regex-replace — Applies a find-and-replace regex across selected columns with
capture-group substitution. (pure)`

## Semantic-duplicate check (done first)

`ls blocks/ | grep -iE 'csv|regex|replace|find'` returns 80+ blocks. The near neighbours were read:

| Existing block | Scope | Overlap verdict |
| --- | --- | --- |
| `find-replace` | Literal/regex find-and-replace over **free text**, one rule, `$1`/`${name}` refs | No CSV parsing, no column scope — replacing inside a quoted CSV field or a single column is not expressible |
| `regex-column-validate` | Applies a regex to a CSV column and **reports** pass/fail rows | Read-only; never rewrites a cell |
| `regex-capture-to-csv` | Turns regex captures found in **free text** into new CSV rows | Generates a table; does not edit one |
| `csv-cleaner` | Trim / dedupe / drop-empty / fill / delimiter normalize | Fixed transforms, no pattern matching |
| `csv-null-standardizer` | Rewrites a fixed **token list** (NA, N/A, null, …) to one representation | Literal token equality, no regex, no captures |
| `csv-pii-redactor` | Mask / salted-hash / label whole cells in chosen columns | Whole-cell replacement by mode, not pattern-driven substitution |
| `csv-date-normalizer`, `csv-header-sanitizer`, `csv-column-split`, `csv-cell-diff`, `csv-filter` | Date reformat / header rewrite / split-concat / diff / row filter | Different transforms entirely |

Conclusion: **not a duplicate.** The distinct capability is *pattern-driven substitution inside
selected columns of a parsed CSV, with capture-group references* — no existing block combines CSV
column scoping with regex replacement. Building it.

## Competitors reviewed

1. **SmoothSheet — CSV Find & Replace** (`smoothsheet.com/tools/csv-find-replace`)
2. **CSV Tools — Find and Replace** (`csvtools.com/find-and-replace/`)
3. **QuickTextTools — Regex Find and Replace** (`quicktexttools.in/tools/regex-find-and-replace`)
4. **Online CSV Tools — Replace a CSV Column** (`onlinetools.com/csv/replace-csv-columns`)
5. **csvre** (`github.com/oskaritimperi/csvre`) — a Rust CLI, the closest functional analogue

## Table-stakes matrix

| Capability | Seen at | Fit | Our decision |
| --- | --- | --- | --- |
| Find + replace fields | all five | in-model | `pattern` + `replacement` |
| Regex vs plain-text toggle | 1, 2, 3 | in-model | `mode = regex \| literal` (literal escapes the pattern and takes the replacement verbatim) |
| Capture-group refs `$1`, `${name}`, `$0`, `$$` | 3, 5 | in-model | Rust `regex` expansion — same syntax; documented on the page |
| Case-insensitive match | 1, 2, 3 | in-model | `ignore_case` |
| Match-entire-cell option | 2 | in-model | `match_scope = substring \| whole_cell` (anchors the pattern) |
| Replace-all vs replace-first (`g`) | 3 | in-model | `replace_all` (default on, matching the `g`-by-default convention) |
| Multiline `m` and dot-all `s` flags | 3 | in-model | `multiline`, `dotall` — meaningful because a quoted CSV cell can legally contain newlines |
| Column-scoped replacement | 1, 4, 5 | in-model | `columns` — blank = every column; names, 1-based indices and `2-4` ranges |
| Column by name **or** index | 4, 5 | in-model | Both, plus ranges (superset of csvre's name-or-index) |
| Header row present / don't search the header | 2, 5 | in-model | `has_header` + `include_header` (header excluded by default) |
| Delimiter choice / auto-detect | 2, 5 | in-model | `delimiter` — `auto`, a single char, or comma/tab/semicolon/pipe |
| Quote-character / output quoting control | 2 | in-model | `quote_style = minimal \| always \| non_numeric` |
| Replacement counter | 1, 2 | in-model | `output = report` — a per-column `column,cells_changed,replacements` audit table |
| Empty replacement deletes matches | 2 | in-model | Falls out of the design; called out in the docs and covered by a test |
| Show only affected rows | — (gap we close) | in-model | `output = changed` — header plus only the rows that changed |
| Diff view (red/green line diff) | 3 | out-of-model | Our surfaces render one output value; `output = changed` is the in-model substitute. `text-diff` already covers before/after diffing |
| Live match highlighting in the input box | 3 | out-of-model | Requires a rich editor overlay; the page renders plain fields |
| Preset chips / quick patterns | 3 (20+ presets) | in-model | Five `[[example]]` chips: phone digits-only, `Last, First` → `First Last`, strip currency symbols, blank out a code, whole-cell code remap |
| Multi-rule / ordered rule list | 1, 3 | out-of-model **here** | Already adjudicated in `docs/tool-skiplist.txt` (`find-replace-batch`, `find-replace-with-diff`): the batch delta belongs on `find-replace`, not as a new tool. One rule per run |
| File upload (CSV/XLSX, 50 MB) + drag-drop | 1 | out-of-model | This is a paste-in text tool; `xlsx-to-csv` handles workbooks. 5,000,000-byte cap documented |
| Raw-byte (non-UTF-8) mode | 5 | out-of-model | The block boundary is a UTF-8 string; `text-encoding-converter` transcodes first |
| Negative column indices (`-1` = last) | 4 | in-model, declined | Ranges plus names cover the need; negatives collide with the `2-4` range syntax. Listed rather than built |

Every table-stake above ends in the descriptor or in the out-of-model list — none dropped silently.

## Gaps we close that the competitors do not

- **Column scope *and* full regex flags together.** The CSV-shaped tools (1, 2, 4) either scope by
  column with weak matching, or match with regex over the whole file. The regex-shaped tool (3) has
  the flags but no CSV parser, so it happily corrupts quoted fields.
- **A parsed, re-emitted CSV.** Replacement happens on decoded cell values, so a pattern can never
  eat a delimiter, a quote, or an embedded newline; output quoting is re-derived.
- **An audit output.** `report` gives per-column counts, not just one global number.
- **Deterministic CLI + chat surfaces.** csvre is the only competitor with a CLI, and it is
  install-required and single-column.

## Notes

- Rust `regex` has no backreferences or lookaround (linear-time guarantee). Competitor 3 runs
  JavaScript `RegExp` and does. Documented as a limit on the page rather than papered over.
- Rust replacement expansion treats `$name` greedily, so `${1}x` is the documented way to write a
  group reference followed by a word character. Called out in the FAQ.
