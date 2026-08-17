# ndjson-to-matrix — competitor analysis (2026-08-17)

Scope: online tools that turn NDJSON / JSON Lines records into an aligned table (CSV or
spreadsheet-shaped output) with a unified column set. All findings are **paraphrased** from public
product pages; no competitor copy, branding, or assets were reused.

## Competitors profiled

| # | Tool (paraphrased role) | Options exposed | Nested handling | Missing cells | Output formats | Notable UX | Stated limits |
|---|---|---|---|---|---|---|---|
| 1 | jsonl.co — JSONL→CSV converter | None surfaced (no delimiter/depth/header controls) | Nested objects become dotted-path columns; arrays are stringified into a single cell | Left blank for records that lack the key | CSV only (separate tool for Excel) | Paste, file upload, drag-and-drop, sample loader, live preview, download, all client-side | No hard row limit; "hundreds of thousands of rows" depends on the device |
| 2 | jsontotable.org — NDJSON→CSV | None surfaced | Objects and arrays are serialized as JSON text inside the cell (no path expansion) | Blank cell | CSV | Paste / upload (.ndjson/.jsonl/.txt), preloaded log sample, copy + download, invalid lines skipped with a count shown | None stated |
| 3 | data.page — NDJSON→CSV | None surfaced; flattening is a *separate* tool | Recommends a different tool for nested objects | Records may simply differ in columns | CSV (+ Sheets export, filter/sort on paid tiers) | Upload/paste → download, advises checking for blank/nested columns | Free tier ~1 MB/day; 50 MB/file on paid; unlimited only in a desktop app |
| 4 | csvjson.com — JSON→CSV | Separator (comma default / tab / semicolon), "flatten nested" toggle, JSON-typed cell variant | Nested values stringified by default; opt-in flatten expands nested object arrays | Not documented | CSV, TSV, JSON-valued CSV variant | Paste or upload, live preview, copy, download, npm package | No size limit stated |
| 5 | table.studio — NDJSON→CSV (multi-format converter) | Format pickers only; conversion internals not documented | Not documented | Not documented | Very wide output matrix (CSV, TSV, XLSX, Parquet, Arrow, SQL, Markdown, HTML, LaTeX…) | Drag-drop, URL paste, up to 10 files, no login | 10 files per batch |
| — | Reference libraries (not web tools) | `pandas.json_normalize`: `sep` (default `.`), `max_level`, `record_path`, `meta`; missing keys become NaN | Dotted paths, level cap | NaN fill | DataFrame | — | — |

## Table stakes (must have)

1. **Union column set across heterogeneous records** — every competitor derives headers from the
   union of keys seen across all lines. ✅ in model.
2. **Dotted-path flattening of nested objects** — the differentiator between jsonl.co (has it) and
   jsontotable.org / data.page (don't). ✅ in model.
3. **Blank/consistent fill for absent keys** — universal. ✅ in model, and worth making
   *configurable* (`0` / `NaN` / `null` / empty) since numeric matrices usually want `0` or `NaN`,
   which no competitor offers.
4. **RFC 4180 quoting/escaping** for embedded commas, quotes, newlines. ✅ in model.
5. **Header row present by default** — universal, but nobody lets you turn it off (needed when the
   output feeds `numpy.loadtxt` / a matrix import). ✅ in model as `headers`.
6. **Delimiter choice** — only csvjson exposes it (comma/tab/semicolon). ✅ in model, plus a
   dedicated `tsv` format.
7. **Tolerant parsing of malformed lines** — jsontotable.org skips and counts them; nobody reports
   *which* line failed. ✅ in model, and we do better: an `error` default that names the line
   number and column, plus `skip`.
8. **Paste + preloaded sample + copy/download** — universal UX. ✅ the shared page runtime already
   gives copy/download; sample coverage comes from placeholders + `[[example]]` chips.

## Gaps we close that competitors don't (in-model)

| Capability | Why it matters | Competitor coverage |
|---|---|---|
| `arrays = index` → `scores.0`, `scores.1` columns | Numeric records commonly carry fixed-length vectors; stringified arrays are useless in a matrix | None — all stringify |
| Configurable `fill` (`""`, `0`, `NaN`, `null`, anything) | A numeric matrix with blanks won't load into numpy/R/Matlab | None |
| `numeric_only` — drop columns that aren't fully numeric | Turns a mixed log stream into a clean numeric matrix in one click | None |
| `columns` selection **and** explicit ordering | Matrix column order is semantically load-bearing | None (csvjson has none) |
| `column_order` = first-seen / alpha / coverage | Reproducible headers across differently-ordered producers | None |
| `matrix` output (whitespace-aligned grid) | Readable numeric grid for eyeballing / pasting into a report | None |
| `json` output (2D array of arrays) | Direct feed for JS/py matrix code | None |
| `transpose` | Records-as-columns is what plotting and per-series code wants | None |
| `row_index` column | Row labels for a matrix without an id field | None |
| `max_depth` flatten cap | Keeps a deeply nested payload from exploding into thousands of columns | pandas only (`max_level`), no web tool |
| `limit` row cap | Preview a huge stream quickly | None |
| Line-numbered parse errors | Fixing bad data needs the line number | None |
| Fully local, no tier limits | data.page caps the free tier at ~1 MB/day | Only jsonl.co / jsontotable.org are local |

## Considered, out of model (not built)

- **File upload of .ndjson/.jsonl and drag-and-drop** — the shared page widget is a paste field;
  a per-tool uploader would be a bespoke control, and paste covers the CLI/chat surfaces too.
- **XLSX / Parquet / Arrow / SQL output** (table.studio) — needs heavy encoders; the repo already
  has dedicated `csv-to-xlsx`, `csv-to-sql` tools to chain after this one.
- **Multi-file batch conversion** — no multi-input surface here.
- **Google Sheets / cloud export, accounts, paid tiers** — server-side by definition.
- **Interactive sortable/filterable result grid** — belongs to a viewer tool (`ndjson-viewer`),
  not a converter; `csv-query`/`csv-sort` cover it downstream.

## Considered, rejected (judgment)

- **Type inference toggle for cell values** — NDJSON values already carry JSON types; re-inferring
  from text would only matter for string-encoded numbers, and `numeric_only` handles the real
  numeric-matrix use case without another param.
- **`record_path`-style explode of arrays into extra ROWS** (pandas `record_path`) — that is a
  different transform (one-to-many), it conflicts with "one record = one row" and the aligned
  matrix contract; `ndjson-filter` + `json-array-pluck-field` cover reshaping.

## Copy / SEO angles observed (ideas only, original wording used)

Competitors lean on: log/event-stream conversion, MongoDB / Elasticsearch / BigQuery exports,
"open it in Excel or Sheets", RFC 4180 correctness, and privacy ("stays in your browser"). Our
page emphasizes the *matrix* framing — unified column set, deterministic column order, numeric fill
— which none of them claim.
