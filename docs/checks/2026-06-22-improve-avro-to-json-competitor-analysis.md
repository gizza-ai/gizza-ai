# avro-to-json — competitor analysis (2026-06-22)

New tool built this run (not a pre-existing improve). Decodes an Apache Avro
Object Container File (OCF / `.avro`) into JSON. Input is the file bytes as a
base64 or hex string; output is a pretty JSON array (`records`), newline-delimited
JSON (`ndjson`), or a `full` object with the embedded writer schema + count +
records. Pure Rust (`apache-avro`, default-features off → deflate via miniz_oxide,
no C deps), so it runs on all surfaces: chat block, CLI, and standalone page.

## Competitors surveyed (paraphrased; no copy/branding reused)

1. **avrotojson.com** — single-purpose online .avro → JSON converter. Requires a
   valid OCF (schema embedded in the file). Upload → JSON output, view in browser.
2. **dataformat.net Avro Viewer** — converts .avro to **JSON and CSV**, downloadable;
   interactive display with expand/collapse of nested objects/arrays.
3. **dataviewer.com Avro Viewer** — extracts + displays the **full embedded schema**
   (field names, types, defaults, nested structures) alongside a table preview of records.
4. **dataconverter.io / table.studio** — multi-format data converters (Avro, Parquet,
   ORC, CSV); Avro → JSON among many target formats; "no sign-up, small or large files."
5. **avro-tools (Apache, local jar)** — the canonical CLI: `java -jar avro-tools tojson file.avro`
   emits one JSON object per line (effectively NDJSON) from an OCF.

## Gap diff vs our tool

| Capability | Competitors | Ours | Verdict |
|---|---|---|---|
| Decode OCF (schema embedded) | yes | yes | parity |
| Pretty JSON array output | yes | yes (`records`, default) | parity |
| NDJSON / one-object-per-line | avro-tools | yes (`ndjson`) | parity (matches the canonical CLI) |
| Show embedded writer schema | dataviewer | yes (`full` → `schema`) | parity |
| Record count | dataviewer | yes (`full` → `count`) | parity |
| Logical types (date/timestamp/uuid/decimal) | partial | decoded (ints / uuid string / base64 bytes) | parity+ |
| CSV output | dataformat | no | **out of scope** — CSV is a distinct flattening tool (json→csv already exists as `csv-json-convert`); records→JSON→CSV is chainable |
| Interactive nested expand/collapse | dataformat, dataviewer | no (static JSON text) | out-of-model — our page is a deterministic recompute-on-input text view; pretty JSON is already readable |
| Parquet/ORC/multi-format | dataconverter | no | out of scope — separate formats, separate tools |
| Local/private, no upload | varies (most upload to a server) | **yes — runs entirely in-browser via wasm** | **our differentiator** |
| Base64/hex paste input (no file picker) | no | yes | our differentiator (works in chat + CLI too) |
| LLM/chat + CLI surfaces | no (web-only) | yes | our differentiator |

## In-model gaps closed in this build

- `records` / `ndjson` / `full` output modes (covers the pretty-array, the avro-tools
  one-per-line convention, and the schema-inspection use case in one tool).
- Embedded writer schema + record count surfaced via `full`.
- Logical types decoded sensibly (dates/timestamps → integers, uuid → string,
  decimal/bytes/fixed → base64) rather than erroring on them.
- `auto`/`base64`/`hex` input encodings (hex tolerates spaces/`:`/`-`).

## Considered, not built (out of model or out of scope)

- **CSV output** — a flattening concern that differs per nesting strategy; JSON→CSV is
  already covered by `csv-json-convert`, and records output is chainable into it.
- **Interactive collapsible tree UI** — gizza pages are deterministic recompute-on-input
  text renderers; a stateful tree widget is outside that model. Pretty JSON suffices.
- **Parquet / ORC / multi-format conversion** — distinct binary formats; separate tools.
- **Bare / single-object-encoded Avro** (no embedded schema) — undecodable without an
  external `.avsc`; documented as a non-goal (this tool reads OCF containers only).

## Verification (this run)

- `cargo test --workspace` — 6 core tests + drift-guard schema test green.
- `wafer build` — chat block validates + instantiates (975 KiB; `apache-avro` + `rand`
  instantiate cleanly in wasm32-wasip1).
- `wafer test` — 2 OCF fixtures (records, full) pass.
- CLI: `gizza tool avro-to-json input=<b64>` → records / `format=full` → schema+count;
  bad-magic input returns a clear error.
- Page Playwright (3 specs incl. query-param deep-link): records array, full schema+count,
  ndjson deep-link — all pass.

Sources: [avrotojson.com](https://www.avrotojson.com/),
[dataformat.net](https://dataformat.net/avro/viewer-and-converter),
[dataviewer.com](https://dataviewer.com/avro-viewer.html),
[dataconverter.io](https://dataconverter.io/convert/avro-to-json),
[table.studio](https://table.studio/convert/avro/to/json).
