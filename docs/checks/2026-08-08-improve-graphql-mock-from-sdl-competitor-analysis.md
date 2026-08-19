# graphql-mock-from-sdl competitor analysis (2026-08-08)

Tool: `graphql-mock-from-sdl` — generate deterministic mock JSON response data from GraphQL SDL schema definitions.

## Scan summary

I compared the table-stakes behavior exposed by common GraphQL mocking workflows:

| Reference workflow | What users expect | In gizza model? | Decision for this tool |
| --- | --- | --- | --- |
| GraphQL Tools schema mocks | Build mock responses directly from SDL/type definitions; allow per-scalar and per-field mock shaping; return deterministic fixture-like objects when wired with a seed. | Partly | Implement SDL parsing, object/interface/union/enum/scalar traversal, deterministic seed, field-name-aware values, and common custom scalar shapes. Do not execute GraphQL queries or resolver functions. |
| Apollo Server / Apollo Sandbox mock examples | Mock a full `Query` root response, add `__typename` for polymorphic shapes, and keep nullable behavior explicit for client-state testing. | Yes | Add `query-response`, `single-type`, and `all-types` modes; force `__typename` on interface/union mocks; expose `typename` toggle for every object; expose nullable fill/null/omit modes. |
| graphql-faker-style SDL directives | SDL authors often annotate fields with `@fake`, `@examples`, and list-length hints to make mock values realistic without writing JavaScript. | Yes, bounded | Parse `@fake(type: ...)`, `@examples(values: [...])`, and `@listLength(min/max: ...)` on fields. Unknown faker names fall back to smart field-name inference instead of failing. |
| Online JSON/mock data generators | Users expect a pasted schema, preset examples, sliders/selects, stable output, and copyable pretty JSON in the browser. | Yes | Add a textarea schema input, enum selects, sliders for list length/depth, checkboxes for smart values / `__typename` / pretty JSON, preset chips, and a generic browser page. |
| Full mock API servers | Serve HTTP GraphQL endpoints, execute arbitrary query documents, merge multi-file projects, plug custom resolver code, and stream paginated responses. | No | Out of scope for a pure browser/CLI block. The tool rejects executable query documents and generates schema-shaped fixture JSON only. |

## Table-stakes mapped to implementation

| Capability | Built | Notes |
| --- | --- | --- |
| Paste GraphQL SDL, not JSON | Yes | Hand-rolled parser for definitions, descriptions, comments, directives, field args/defaults, nested list/non-null wrappers, schema root, and `extend`. |
| Mock every response type | Yes | `all-types` emits object/interface/union mocks and skips input/enum/scalar declarations as top-level response data. |
| Query response envelope | Yes | `query-response` emits `{ "data": ... }` from `schema { query: ... }` or `type Query`. |
| Single named type | Yes | `single-type` works for objects, interfaces, unions, enums, built-ins, custom scalars, and input types. |
| Deterministic random data | Yes | Seeded SplitMix64-like PRNG; seed + SDL + options produce stable output. |
| Enum values from schema | Yes | Values come from the SDL enum list. |
| Interfaces/unions | Yes | Interface resolves to first implementor; union resolves to first member; both carry `__typename`. |
| Nullable controls | Yes | Fill, force null, or omit nullable fields while non-null fields remain generated. |
| List controls | Yes | Global 0–10 list length plus field-level `@listLength`, clamped to the cap. |
| Smart fake values | Yes | Field-name inference for common names plus `@fake(type: ...)` and `@examples(values: [...])`. |
| Custom scalar handling | Yes | Recognizes common scalar names and labels unknown scalars as `mock-<name>`. |
| Browser UX controls | Yes | Textarea SDL, enum selects with labels, sliders for bounded numbers, checkboxes, and example chips. |
| Query execution / resolver JS | Out of model | Requires a GraphQL executor plus arbitrary user resolver code; not suitable for this pure WebAssembly block. |
| Hosted mock endpoint | Out of model | This repository ships blocks/pages/CLI, not a hosted API service. |
| Multi-file schema import | Out of model | Users can paste the merged SDL; filesystem/project import is intentionally not in the page model. |

## Verification focus

The verification matrix should cover:

- exact deterministic output for a small `Query` schema;
- `single-type` mode with `__typename`, non-default list length, and smart values;
- nullable `null`/`omit` options;
- cap boundaries (`list_length=10`, `depth=6`) and errors above the caps;
- field directives (`@fake`, `@examples`, `@listLength`);
- browser deep links for schema text and options;
- CLI invocation generated from the page manifest.
