# json-schema-batch-validate — competitor analysis (2026-07-24)

Scan of online JSON-Schema validators to set the table-stakes for a **batch** validator: an
array or NDJSON stream of records checked against **one** schema, returning a pass/fail summary
plus a per-record error list. All findings are **paraphrased** — no competitor copy, branding,
or trademarks are reproduced.

## Competitors reviewed (paraphrased)

- **jsonschemavalidator.net (Json.NET Schema)** — one schema + one instance textarea; no
  array-batch or NDJSON per-record mode. Widest draft coverage (3/4/6/7/2019-09/2020-12) via a
  dropdown; `$schema` auto-detect not surfaced. Valid/invalid message panel, no explicit
  JSON-Pointer/keyword breakdown.
- **JSONLint schema validator (Ajv-backed)** — split schema/data panes, Validate / Load-example /
  Clear. Single document, draft-07 default, no draft dropdown. Human-readable messages, no
  pointer/line/count surfaced.
- **Hyperjump JSON Schema** — schema pane plus one or more instance panes (multi-instance against
  one schema — closest thing to batch found here). Dialect recognized from the schema's `$schema`
  (auto-detect). Detailed error structure not exposed in the landing UI.
- **onDevTools JSON Schema Validator (Ajv)** — two textareas, single document, draft-07/2019-09/
  2020-12, auto-selects meta-schema from `$schema` (falls back to draft-07). Errors as
  `path: message` with RFC-6901 JSON Pointers, collects **all** errors in one pass, "Copy Errors"
  as newline-separated text. Fully client-side.
- **go-tools.org JSON Schema Validator** — two panels, live validation, single document. 2020-12
  default / 2019-09 / draft-07 dropdown with `$schema` auto-detect. Errors show the trio
  **path + keyword + message**, clickable. Internal `$ref` resolved; external HTTP `$ref`
  intentionally disabled for privacy. Four example presets.
- **NDJSON validators (ndjson.com and similar)** — the only batch/per-record family: line-by-line
  validation, per-line results with line numbers, a summary strip (total lines, valid, error
  lines, success rate). No formal JSON-Schema draft selection; leans structural.

**Market gap.** The draft-selectable, pointer+keyword validators are all single-document; the
NDJSON batch tools do per-record reporting but skip formal draft selection and pointer/keyword
detail. Combining **batch input (array + NDJSON)** with **per-record JSON-Pointer/keyword errors**
**and** draft selection/auto-detect is an unoccupied niche — the position this tool takes.

## Table-stakes → in-model / out-of-model decisions

| Feature | Decision | Where it landed |
|---|---|---|
| Two inputs: one schema + one records batch | **in-model** | `schema` + `records` params, both multiline textareas |
| Accept JSON array of records AND NDJSON | **in-model** | `input_format` = auto/json/ndjson; auto tries JSON then NDJSON |
| Also accept a single JSON value as one record | **in-model** | non-array top-level value → one-record batch |
| Per-record pass/fail + overall summary (counts) | **in-model** | `Report` (total/passed/failed/total_errors) + text summary |
| Each error: JSON Pointer path + keyword + message | **in-model** | `RecordError { path, keyword, message }` from the SIMD-safe in-repo validator |
| Draft selector draft4/6/7/2019-09/2020-12 | **in-model as a reported label** | `draft` enum param; validation is draft-agnostic over the supported subset |
| Auto-detect draft from `$schema` | **in-model as a reported label** | `draft = auto` reads `$schema`, falls back to draft2020-12 |
| Collect ALL errors per record (not fail-fast) | **in-model** | the in-repo validator walks every supported keyword and reports every failure |
| Max-errors cap for large batches | **in-model** | `max_errors` integer (default 50); counts stay exact, report marks `truncated` |
| Output toggle: readable text vs machine JSON | **in-model** | `output` = text/json |
| Example presets / load-sample | **in-model** | `[[example]]` chips on the page (array + NDJSON) |
| Client-side / offline privacy | **in-model** | runs as browser-local wasm; nothing leaves the device |
| NDJSON line numbers in errors | **considered, partial** | records are reported by 0-based index (array index / NDJSON line order, blank lines skipped); a bad NDJSON line reports its 1-based line number in the parse error |
| `additionalProperties` / strict handling | **in-model (schema-driven)** | honored straight from the schema; no separate global strict toggle (schema bloat, and the schema already expresses it) |
| Complex composition/conditional keywords (`oneOf`, `allOf`, `if/then/else`, unevaluated*) | **out-of-model for this build** | documented as unsupported annotations; full crate emitted wasm SIMD that wafer cannot instantiate |
| Remote/external `$ref` over HTTP | **out-of-model** | needs network fetch; this tool is local-only. Internal `$ref` inside the pasted schema still resolves |
| Multi-file / bulk file upload | **out-of-model** | paste-in textareas only; no file workflow |
| Saved/shareable schema URLs, accounts, history | **out-of-model** | requires server persistence + auth |
| Meta-schema validation against live spec URIs | **out-of-model** | would fetch spec meta-schemas over network |

## Worked examples (used on the page / tests)

**A — required + type, JSON array, draft auto-detected (draft-07):**
schema `{"$schema":"http://json-schema.org/draft-07/schema#","type":"object","required":["id","name"],"properties":{"id":{"type":"integer"},"name":{"type":"string"}}}`,
records `[{"id":1,"name":"Ada"},{"id":"2","name":"Grace"},{"name":"Kay"}]` →
**FAIL**, 1 passed / 2 failed: record 1 `/id` keyword `type`; record 2 root keyword `required`.

**B — NDJSON stream, format + range (draft2020-12 auto):** three lines, one valid, one with a
bad email `format` + an out-of-range `maximum`, one missing the required `email` → 1 pass / 2 fail.

**C — additionalProperties + max_errors cap (draft-07):** a strict `additionalProperties:false`
schema against a record with several extra keys and `max_errors` low → report marked `truncated`.

## Controls / UX matched

- Two multiline textareas (schema + records) with a draft dropdown and a max-errors number field.
- Draft `<select>` labeled with clean names ("Auto (detect from $schema)", "Draft 2020-12", …).
- `input_format` and `output` dropdowns with friendly labels.
- Example preset chips: an array batch, an NDJSON batch, and a JSON-output case.
- Privacy/limits stated on the page: local-only, no network `$ref` fetching, drafts supported,
  how truncation and the max-errors cap behave.
