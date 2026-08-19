# JSON Schema to sample competitor analysis (2026-08-09)

Backlog tool: `json-schema-to-sample` — generate a deterministic minimal JSON instance from a JSON Schema.

## Competitor scan

| Competitor | Observed surface | Table-stakes controls and UX | In-model decisions for this tool | Out-of-model / not built |
| --- | --- | --- | --- | --- |
| JSON Schema Faker | Library/online demo that generates fake JSON from JSON Schema, supporting required fields, properties, arrays, formats, refs and options for optional fields. | Paste schema, generate valid-looking JSON, honor defaults/enums/formats, optionally include optional fields. | `schema` is a multiline input; `include_optional` controls optional properties; formats produce deterministic examples such as email/uuid/date; enum/const/default/examples are honored before generated values. | Random faker data, seeds, locale-specific names and large synthetic datasets are not built; this tool is deterministic and minimal. |
| Liquid Technologies JSON Schema Sample Generator | Web UI to paste JSON Schema and produce a sample JSON document for documentation/testing. | One paste field, readable formatted JSON result, and support for object/array structure. | `pretty=true` is the default; arrays, objects, required fields, numeric/string bounds and examples produce documentation-ready output. | XML/sample generation for non-JSON schema languages is out of scope. |
| quicktype / transform.tools style JSON Schema example tools | Developer utilities that convert or preview schemas and examples in-browser. | Immediate browser-local transformation, compact output option, copyable result, helpful errors for invalid JSON. | The page runs in WebAssembly, reports invalid JSON and unsupported remote `$ref`s clearly, and `pretty=false` gives compact one-line output. | Type generation and schema inference are covered by other tools; this block only emits an instance from an existing schema. |
| OpenAPI/example generators | Often generate request/response examples from component schemas, with `$ref`, `allOf`, enum/default and required support. | Resolve local component refs, merge `allOf`, pick a branch for `oneOf`/`anyOf`, and honor OpenAPI `example`. | Local JSON Pointer `$ref`s, `$defs`/`definitions`, `allOf`, first `oneOf`/`anyOf`, `example`, `examples[0]` and `default` are implemented. | Full OpenAPI document traversal, remote refs, discriminator-aware oneOf selection and readOnly/writeOnly filtering are not implemented. |

## Table-stakes matrix

| Capability | Decision | Notes |
| --- | --- | --- |
| Generate object/array/string/number/boolean/null samples | In model | Supports JSON Schema primitive types and inferred object/array types from structural keywords. |
| Honor `required` | In model | Required properties are emitted by default; optional properties appear when requested. |
| Honor defaults/examples/enums/constants | In model | Precedence is `const` → `default` → `examples[0]` → `example` → `enum[0]` → generated value. |
| Optional-property toggle | In model | `include_optional=false` keeps minimal examples; true emits every property. |
| Array count control | In model | `array_items` default 1, with `minItems`/`maxItems` bounds still respected. |
| String formats and bounds | In model | Common formats (email, uuid, date, uri, ipv4, etc.) plus min/max length. |
| Numeric bounds | In model | minimum/maximum/exclusive bounds and multipleOf. |
| Local `$ref` | In model | Resolves local JSON Pointers including `$defs` and `definitions`; recursive refs cut to null. |
| Composition | In model | `allOf` deep-merged; `oneOf`/`anyOf` use first branch for deterministic examples. |
| Pretty/compact output | In model | `pretty` boolean controls formatting. |
| Regex-based synthesis | Out of model | `pattern` and `patternProperties` are documented as not generated. |
| Remote refs / network fetch | Out of model | Browser-local and CLI-local only; remote `$ref` is rejected. |
| Random fake data | Out of model | Determinism is preferred for docs and golden fixtures. |

## Defaults and UX choices

- Default output is minimal, required-only and pretty-printed: best for API docs and request examples.
- Optional defaults are still included even when `include_optional=false`, because defaults are useful effective values.
- Output is deterministic; no clock, randomness or faker library is used.
- Example chips cover required-only object generation, `$defs`/arrays, and compact optional output.
- Competitor capabilities informed the control set; no competitor wording or branding was reused.
