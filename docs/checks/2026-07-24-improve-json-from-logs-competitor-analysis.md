# json-from-logs — competitor analysis (2026-07-24)

Function scanned: "extract JSON from logs / find embedded JSON objects in mixed console text".
Searches: *extract JSON from logs online*, *console log JSON extractor*, *extract all JSON
objects embedded in text*. All findings paraphrased — no competitor copy, branding, or
trademarks reproduced.

## Competitors inspected (top 3 real tools)

### 1. DevToolsLabs — JSON Data Extractor
- **Function:** paste a *complete* JSON document, then pull values out with a JSONPath
  (`$.users[*].id`) or dot-notation expression; recursive-descent (`..`) to find every
  occurrence of a key.
- **Params/UX:** two-pane editor (source left, result right), a query bar, quick-example
  buttons for common expressions, filter expressions (e.g. price comparison), copy button.
- **Input/output:** whole JSON in → queried subset out. All client-side (WebAssembly + JS,
  nothing uploaded).
- **Limits stated:** none published.
- **Gap vs us:** requires the input to *already be* valid JSON — it does **not** scan mixed
  log/console text and pull embedded JSON out. Querying is a different job (we already ship
  `jsonpath-query` for that).

### 2. json-text-extract.com — JSON Text Extractor
- **Function (from listing; site 403'd the fetch):** extract fields or run JSONPath over JSON,
  with a simple-field mode and a JSONPath mode plus dual view. Markets "parse structured log
  files to extract error messages, timestamps, user actions" and "extract error codes / stack
  traces from application log JSON".
- **Params/UX:** field-name vs JSONPath toggle, two view modes.
- **Gap vs us:** still oriented at *structured* JSON logs (each line already JSON) and at
  querying, not at recovering JSON that is embedded inside otherwise-unstructured text.

### 3. onlinetools.com — Extract JSON Values / Extract JSON Keys (+ jsonformatter.online JSON Log Viewer)
- **Function:** paste whole JSON, one-click extract of all keys or all values to a flat list;
  the sibling JSON Log Viewer renders JSON log files, highlights levels (info/warn/error),
  groups by time/category.
- **Params/UX:** one-click extract, instant browser-side result, copy.
- **Limits stated:** none published.
- **Gap vs us:** whole-JSON input again; the viewer is a rendering/grouping surface, not an
  embedded-JSON extractor.

## Table stakes (present across competitors → we must match)
- Paste text in, JSON out, instantly, **client-side / private** (no upload). ✔ we run local wasm.
- **Pretty-print + validate** the extracted JSON. ✔ core validates with serde_json and
  pretty-prints; malformed candidates are skipped.
- **Copy result** button + at least one worked example. ✔ generator gives Copy/Reset; page ships
  worked examples + preset chips.

## Our differentiator (the actual pain point — see the Splunk "JSON mixed with unstructured"
## thread that keeps recurring)
None of the inspected tools **scan arbitrary log/console text and pull each embedded JSON
object/array out**; they all assume the whole input is already JSON. `json-from-logs` brace-
matches balanced `{…}`/`[…]` runs anywhere in the text, validates each with serde_json, and
pretty-prints them separately — turning a wall of log lines with `state={…}` fragments into a
list of clean, validated blocks. This is the capability, not a copy/branding element.

## Defaults / controls decided (in-model)
- `text` — the raw log/console text to scan (required).
- `indent` — spaces per level, 0–8, default **2**; `0` minifies each block (matches the
  indentation control competitors expose on their formatters).
- `output` — `blocks` (default): each extracted JSON block pretty-printed under a
  `// block N (line L)` header, line-numbered for log triage; `array`: all extracted blocks
  wrapped into one pretty JSON array for machine reuse.

## Out-of-model / considered, not built
- **JSONPath / dot-notation querying** of the extracted JSON — real feature, but it is a
  *different tool*; gizza already ships `jsonpath-query` and `jsonata-query`. Extracting is the
  distinct job here; chaining is the user's to compose.
- **Log-level highlighting / time grouping / colorized viewer** (jsonformatter.online) — a
  rendering surface, out of scope for a pure text→text extractor; `log-analyzer`/`log-parser`
  already cover structured-log analysis.
- **Repairing malformed JSON** before extraction — deliberately declined; we validate strictly
  and skip non-JSON candidates. `json-repair` is the tool for fixing broken JSON.
- Server-side batch / file upload / accounts — out of gizza's browser-local model.
