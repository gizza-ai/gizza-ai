# json-to-json-schema — competitor analysis (2026-07-18)

Function: infer a JSON Schema from one or more JSON examples. Built new; this scan set the
descriptor scope before implementation. All notes are **paraphrased** — no competitor copy,
branding, or trademarks were copied.

## Competitors scanned

1. **JSONLint — JSON Schema Generator** (`jsonlint.com/json-schema-generator`) — infers a schema
   matching the sample structure; emits Draft 2020-12.
2. **jsonvalidator.dev — To Schema** (`jsonvalidator.dev/toschema`) — 100% in-browser, Draft-07;
   infers nested objects/arrays, union types for mixed fields, required vs optional, and format
   hints (email, uri, date-time, ipv4). Also advertises enum inference and min/max &
   minLength/maxLength constraints. Controls: Generate / Copy / Paste / Clear / Download.
3. **Liquid Technologies — Online JSON to Schema** (`liquid-technologies.com/infer-json-schema`) —
   options: array rules (Allow-anything / List validation / Tuple typing), defaultAdditionalItems,
   defaultAdditionalProperties, infer-enum-values, make-required, indent char/depth, quote style.
   Online version stores data server-side (retained) — a privacy gap gizza's local model closes.
4. **quicktype — JSON Schema** (`quicktype.io/schema`) — infers a schema (and code) from JSON;
   merges array elements; CLI + web.
5. **Itential — JSONtoSchema** (`itential.com/developer-tools/json-to-schema/`) — dynamic inference
   with an interactive schema editor after the initial inference.

## Table-stakes → decisions

| Capability | In model? | Decision |
|---|---|---|
| Infer schema from a JSON object OR array sample | yes | **built** — root object/array both handled |
| Merge array elements (keys missing in some → optional; mixed types → union) | yes | **built** — shared merge logic |
| Draft version selection (Draft-07 / 2020-12) | yes | **built** — `draft` enum, default `2020-12` |
| Required-field inference (present-in-all → required) | yes | **built** — `required` toggle (default on) |
| `additionalProperties: false` (strict) toggle | yes | **built** — `additional_properties` toggle (default off = strict) |
| String format detection (email, uri, date-time, date, uuid, ipv4) | yes | **built** — `detect_formats` toggle (default on); `uuid` only emitted for 2020-12 (not a Draft-07 format) |
| Root schema `title` | yes | **built** — optional `title` param |
| Local / no-upload / no-account | yes | **built** — gizza is browser-local wasm |
| Numeric constraints (min/max) & string length (minLength/maxLength) | yes but | **considered, rejected** — inferring bounds from a handful of examples over-constrains the schema (a sample `age:30` should not force `minimum:30`); competitors that do it hedge and it produces surprising schemas. Left out to keep the inferred schema faithful. |
| Enum inference from repeated string values | yes but | **considered, rejected** — reliable only across many samples; from sparse examples it wrongly locks fields to the values seen. |
| Interactive post-inference schema editor (Itential) | no | out-of-model — needs stateful editor UI; gizza surfaces are typed-in → typed-out. |
| Server batch / API keys / accounts (Liquid online) | no | out-of-model — gizza runs locally, which is the privacy advantage. |

Every table-stake lands in the descriptor or is explicitly listed above — none dropped silently.

## UX patterns adopted

- `draft` renders as a `<select>` (enum) with friendly labels.
- `additional_properties`, `required`, `detect_formats` render as checkboxes.
- `json` is a multiline textarea; preset `[[example]]` chips prefill real worked samples.
