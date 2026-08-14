# ndjson-viewer — competitor analysis (2026-08-14)

Scan run **before** implementing, per `/create-next-tool` step 2 → `/improve-tool` Phases 2–4.
Everything below is **paraphrased** from public product pages; no competitor copy, branding, or
trademark text is reproduced or reused anywhere in the block, the page, or the tests.

Backlog row: `ndjson-viewer, data, "Pretty-prints and filters newline-delimited JSON streams,
with key search across records."` (type_hint `pure`).

## Duplicate check (done first)

`ls blocks/ | grep -iE 'json|ndjson|jsonl|log'` returns 74 neighbours. The three that could have
made this a duplicate were read directly:

| Existing block | What it actually does | Overlap verdict |
|---|---|---|
| `blocks/ndjson-filter` | A **predicate language** (`age > 28 and city == NYC`, `and`/`or`/`not`, parentheses, regex) plus a field-selection/rename expression, output as compact NDJSON, a pretty JSON array, or CSV. Every clause needs an explicit dotted path. | Overlaps on *filtering by a known path*. It cannot search when you do **not** know the path — there is no way to express "any key or value anywhere in the record contains `timeout`" — it cannot pretty-print records individually (its NDJSON output is compact; only the whole-array form is indented), it has no indent control, no key sorting, no aligned table view, no pagination offset, no per-line diagnostics beyond erroring on the first bad line, and no stats header. |
| `blocks/data-format-converter` | Format conversion including `ndjson → json array`, `ndjson → csv`, `json array → ndjson`, with `from=auto` detection. | Overlaps on the *array* view only. It is a converter: no search, no filters, no per-record pretty view, no pagination, no diagnostics. `docs/tool-skiplist.txt` lines 406–408 already skiplisted `ndjson-to-json`, `ndjson-to-csv` and `json-array-to-jsonl` against it — this tool deliberately does **not** ship a CSV target so it does not re-open that. |
| `blocks/json-beautify` | Pretty-prints/minifies **one** JSON document. | No NDJSON handling, no per-line recovery, no cross-record search. |

Conclusion: **not a duplicate, but a deliberately narrow neighbour.** The distinct, non-subsumed
capability set is (a) path-free key/value **search across every record at any depth**, (b)
per-record **pretty/minify rendering with indent control and line numbers**, (c) an **aligned
table** view, (d) **skip/limit pagination**, (e) **per-line invalid-JSON diagnostics** that keep
going instead of aborting, and (f) an optional **stats header with a top-level key inventory**.
To keep the family boundary clean this tool intentionally ships **no predicate language, no field
selection/rename, and no CSV output** — those stay `ndjson-filter`'s and
`data-format-converter`'s. Adjacent un-built backlog rows deliberately left alone:
`ndjson-field-stats` (per-field presence rate / type mix / min-max-distinct) and `ndjson-validator`
(pure validation report) — the stats header here is a one-line key inventory, not a field profile.

## Competitors reviewed

1. **DevToolEasy — JSON Lines Viewer** (`devtooleasy.com/dev-tools/jsonl-viewer`)
2. **Toolinix — JSON Lines / NDJSON Viewer** (`toolinix.com/en/tools/ndjson-viewer`)
3. **JSONtoTable — NDJSON Viewer** (`jsontotable.org/ndjson-viewer`)
4. *(context only, not a hosted competitor)* **frankvlotman/NDJSON-Viewer** on GitHub — a desktop
   Tkinter app; noted because it is the only one of the set whose headline feature is
   search-across-all-columns-by-any-keyword, which is this row's stated differentiator.

### Feature matrix (paraphrased)

| Capability | DevToolEasy | Toolinix | JSONtoTable | Ours (shipped) |
|---|---|---|---|---|
| Per-line parse, invalid lines isolated | yes, inline, keeps going | yes, reports invalid line numbers | yes, flags each bad line | yes — `invalid = report \| skip \| error`, marker carries the line number and the column |
| Pretty-print each record | yes (card view) | yes (toggle) | array only | yes — `view = pretty`, plus `indent` 0–8 |
| Minified / compact one-per-line | via copy | no | no | yes — `view = compact` |
| Single JSON array output | yes (download) | yes | yes (headline) | yes — `view = array` |
| Table view, columns = discovered keys | yes | yes | via a separate tool | yes — `view = table`, union of keys in first-seen order |
| Search across all fields | yes, case-insensitive substring | no (validation only) | no | yes — `search` + `search_in = any \| keys \| values` + `match_mode = contains \| exact \| regex` + `case_sensitive` |
| Filter by a specific key/path | no | no | no | yes — `path` (dotted, array indexes) + `value` |
| Blank / CRLF line tolerance | yes | `skip empty lines` toggle | yes | yes, always (blank lines skipped, `\r` stripped, BOM stripped) |
| Record / valid / invalid counts | yes | yes | yes | yes — `stats` header, plus a top-level key inventory |
| Pagination | scroll only | scroll only | scroll only | yes — `skip` + `limit` |
| Key sorting | no | no | no | yes — `sort_keys` |
| Line numbers shown | yes | line numbers on errors | error lines | yes — `line_numbers` (pretty header, compact prefix, table column) |
| Runs locally, nothing uploaded | yes | yes | yes | yes (WebAssembly in the page; CLI is offline) |

### Table stakes → where each one landed

Every table stake from the scan is either in the descriptor or in the out-of-model list below —
none dropped silently.

- Per-line validation that does not abort the whole paste → `invalid = report` (default).
- Pretty view with readable indentation → `view = pretty`, `indent` (default 2).
- NDJSON → JSON array → `view = array`.
- Table with a column per discovered key → `view = table`.
- Case-insensitive substring search over all fields → `search` with `case_sensitive = false`
  default; extended past the competitors with `search_in` (keys only / values only) and
  `match_mode = regex`.
- Blank-line and CRLF tolerance → always on, no toggle needed (Toolinix's "skip empty lines"
  option is a setting we do not need to expose because blank lines are never meaningful NDJSON).
- Valid/invalid/record counts → `stats`.
- Sample data → shipped as five `[[example]]` preset chips instead (the platform's declarative
  answer to "load sample").

### UX / control patterns adopted

- `[[example]]` chips (5) — the declarative equivalent of every competitor's "load sample data"
  button, and they double as the page's worked examples.
- `[input.labels]` friendly `<select>` labels on all four enums, so the view picker reads
  "Pretty — one indented record per block" rather than `pretty`.
- `multiline = true` on the data field so a pasted stream keeps its newlines.
- Placeholders on every text/number field showing a real value.
- `wide = true` — the table and pretty views are wide output.

## Out-of-model (considered, not built)

- **File upload / drag-and-drop of a `.jsonl` file** — the page's field input is a text field;
  a file source belongs to the `runtime = "ffmpeg"` media path, not to a pure text block. Paste is
  the supported input, and the CLI reads a file through the shell (`--json "$(...)"`).
- **Download as `.json` / `.csv`** — the generator already gives every `format = "text"` page a
  Copy result button and a Download link generically; no per-tool work, and CSV is deliberately
  `data-format-converter`'s job.
- **Interactive collapsible tree / expandable cards, sortable table columns, syntax highlighting** —
  these need per-tool client-side JS and stateful DOM. The block's contract is text in → text out
  across chat, CLI and page, so the equivalents shipped are `indent`, `sort_keys`, `line_numbers`
  and `skip`/`limit`.
- **Streaming a 25 MB+ file without loading it** — the wasm boundary takes the whole string; the
  honest answer is a stated cap (50,000 lines) with a clear over-cap error, which is on the page.
- **Export to Excel** (the GitHub desktop app's feature) — binary XLSX writing, out of scope for a
  text block.

## Considered, rejected (in-model but declined)

- **A predicate language** (`status == error and latency > 100`) — that is exactly
  `blocks/ndjson-filter`. Adding it here would make the two blocks indistinguishable for chat and
  CLI users. `path` + `value` + `match_mode` covers the single-condition case a viewer needs.
- **Field selection / renaming** (`id=user.id,msg`) — same reason; `ndjson-filter` owns it.
- **CSV output** — `data-format-converter` owns `ndjson → csv`, and three backlog rows were
  already skiplisted against it.
- **A per-field statistics profile** (presence rate, type mix, min/max/distinct) — that is the
  un-built `ndjson-field-stats` row; the `stats` header here stops at counts plus a key inventory
  so that row stays buildable.
- **`kind = "tag-list"` for the search field** — search is a single free-text needle, and a regex
  needle can contain commas; a pill list would corrupt it.

## Verification (this build)

Recorded in the build report, not claimed here in advance: `cargo test --workspace` (core + block
drift-guard), `scripts/build-block-wasm.sh ndjson-viewer`, `wasm-pack build`,
`sync-tool-manifest.py`, the page generator, `check-tool-hygiene.py`, `gizza tool ndjson-viewer`
including one exact-output case per enum value, and the Playwright spec
`tests/tool-page-ndjson-viewer.spec.ts` (real output assertions, one `?param=` deep link, every
enum choice, a non-default checkbox state, and the exact 50,000-line cap boundary).
