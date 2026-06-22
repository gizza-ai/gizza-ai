# csv-json-convert — competitor analysis (2026-06-20)

Built as the first `/create-next-tool` backlog pick (a clean pure tool; the
picker deferred the model-only rows ahead of it and skipped the `pdf-to-text`
dup). Research via `WebSearch` + `WebFetch` (no firecrawl in this env). All
findings are **paraphrased** — no competitor copy, branding, or trademarks used.

## Competitors surveyed (top, reachable, browser-local tools)

| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| TableConvert (tableconvert.com) | many delimiters auto-recognized; type/encoding auto-detect; multiple JSON output shapes (object array, 2D array, column array, key-value); local-only; editing extras (dedupe, transpose, sort, regex replace) | capabilities / UX |
| ConvertCSV (convertcsv.com) | nested JSON via header columns; JSONLines / NDJSON (Mongo) output mode | capabilities |
| json2csv.net / miniwebtool | flatten nested objects/arrays into dot-notation columns; "keep nested as JSON" alternative | capabilities |
| Jam (jam.dev) | PapaParse-based; optional lowercase-all-keys; handles very large files | capabilities |
| FormatJSONOnline / Tooloogle | first-row-as-headers toggle (objects vs arrays-of-arrays); semicolon/tab/pipe delimiters; optional number/boolean/null parsing; strictly in-browser | capabilities / UX |

## Gap diff vs our tool

Already covered before this pass: both directions + **auto-detect**; type
inference (number/bool/null) with leading-zero/`+` preserved as strings; custom
delimiter (char + `tab`/`comma`/`semicolon`/`pipe` words); `headers` toggle
(objects vs arrays-of-arrays); `pretty` JSON; nested values written as compact
JSON strings; RFC-4180 quoting (via the `csv` crate).

**In-model gaps closed in this pass:**
- **flatten** (json→csv) — expand nested objects/arrays into dot-notation columns
  (`addr.city`, `tags.0`), the most-cited differentiator (json2csv, miniwebtool).
  Added as a `flatten` boolean param across all three surfaces + tests.

**In-model gaps considered, deferred (would fit; not built this pass to keep the
first build focused — good follow-up `/improve-tool` candidates):**
- JSONLines / NDJSON output for csv→json (one object per line).
- Extra JSON output shapes (column arrays, key-value pairs).
- `lowercase-keys` option.
- Selectable quoting mode (minimal / quote-all / never).

**Out-of-model (not applicable to a browser-local pure-wasm tool):**
- Cloud batch / large-file streaming pipelines, accounts, saved conversions —
  need a backend; gizza is browser-local.

## Tested
unit (13, incl. flatten + the regenerated drift-guard) · wafer fixtures (4) ·
`wafer build` validates the block · `wasm-pack` web · generator renders the page ·
`gizza tool csv-json-convert` (CLI) · Playwright page + query-param deep-link.

> Original work only — no competitor copy, branding, or trademarks copied.
