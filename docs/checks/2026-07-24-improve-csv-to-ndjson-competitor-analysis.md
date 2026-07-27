# csv-to-ndjson — competitor analysis (2026-07-24)

Tool function: convert CSV text to NDJSON (newline-delimited JSON) — one JSON value per
line, no enclosing array — for streaming/ingest pipelines (jq, BigQuery, Elasticsearch bulk,
log shippers). Paraphrased research only; no competitor copy/branding reproduced.

## Competitors scanned (top 3 reachable)

1. **table.studio — CSV to NDJSON.** Free, no signup, upload-and-convert (small or large
   files). Emphasizes NDJSON as "one JSON object per line" for streaming. UI is upload/paste →
   convert → download; specific parse toggles are not exposed in its public copy. Notes standard
   CSV quoting rules (fields with commas/newlines/quotes wrapped in double quotes). SEO angle:
   "convert CSV to NDJSON online", large-file friendly.

2. **SimonJang/csv-to-ndjson (npm library).** Options: custom **delimiter** (default `,`),
   custom **header** names (array replacing the CSV header row), destination path (stream vs
   file). Output = one JSON object per row; **all values stay strings by default**
   (`{"name":"John","age":"30"}`). Requires `.csv` extension. Confirms the canonical NDJSON
   shape and the "strings unless told otherwise" default.

3. **csvjson.com (CSV→JSON, closest interactive analog).** Toggles: **delimiter** (auto /
   comma / semicolon / tab), **parse numbers** (numeric strings → JSON numbers), **parse JSON**
   (`null`/`false`/`true`/`[]`/`{}` literals parsed), **transpose**, output **array vs hash**,
   **minify**. First line treated as header/column names. Establishes type-inference toggles and
   delimiter choice as table-stakes for CSV-to-JSON-family tools.

## Table-stakes → decisions

| Feature | Source | In-model? | Decision |
| --- | --- | --- | --- |
| Delimiter select (comma/semicolon/tab/pipe or any char) | all | in-model | `delimiter` param (word or single char), default `,` |
| First row = field names (objects) vs arrays | csvjson, SimonJang | in-model | `headers` boolean, default true |
| Parse numeric strings → JSON numbers | csvjson | in-model | `parse_numbers` boolean, default false |
| Parse `true`/`false`/`null` literals | csvjson (parse JSON) | in-model | `parse_bools` boolean, default false |
| Trim whitespace around fields | common | in-model | `trim` boolean, default false |
| RFC-4180 quoting (embedded commas/newlines/`""`) | table.studio | in-model | handled by the `csv` crate reader |
| NDJSON output (one value per line, no array) | all | in-model | fixed output format (the tool's purpose) |
| Custom/renamed header names | SimonJang | in-model but niche | **considered, rejected** — schema bloat; users can edit the CSV header row. Noted below. |
| Minify/pretty | csvjson | out-of-model here | NDJSON is inherently one compact object per line; pretty-printing would break the line-delimited contract. N/A. |
| Large-file upload / streaming to disk | table.studio, SimonJang | out-of-model | browser-local wasm compute on pasted/loaded text; no server streaming. |

## Defaults chosen

- `delimiter=","`, `headers=true`, `parse_numbers=false`, `parse_bools=false`, `trim=false`.
- Values are kept as **strings by default** (matches SimonJang; predictable for downstream
  pipelines) with opt-in type inference — the opposite of `csv-json-convert`, which always
  infers. This is the deliberate distinction: NDJSON consumers usually want lossless strings
  unless they explicitly request coercion.

## Not a duplicate of `csv-json-convert`

`csv-json-convert` emits a single JSON **array** (optionally pretty-printed) and always
type-infers. `csv-to-ndjson` emits **newline-delimited** JSON (streaming shape, no array) with
opt-in inference. Different output contract and default behavior → distinct tool.

## Worked example

Input (`data`):
```
name,age,active
Ada,36,true
Grace,,false
```
Output (defaults — strings preserved):
```
{"name":"Ada","age":"36","active":"true"}
{"name":"Grace","age":"","active":"false"}
```
With `parse_numbers=true` and `parse_bools=true`:
```
{"name":"Ada","age":36,"active":true}
{"name":"Grace","age":null,"active":false}
```
