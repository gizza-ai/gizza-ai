# openapi-stub-from-json — competitor analysis (2026-08-21)

Scan run BEFORE implementation. One WebSearch ("generate OpenAPI schema from JSON example
request response online tool") + WebFetch of the top 3 reachable tools. All notes are
paraphrased observations of *capabilities*; no competitor copy, wording, or branding is
reused anywhere in this tool.

## Tools reviewed

| # | Tool | What it does |
|---|------|--------------|
| 1 | ByteJSON — JSON to OpenAPI Spec Generator (`bytejson.com/tools/api/json-to-openapi/`) | Paste one JSON sample → OpenAPI 3.0 schema; extracts nested objects into reusable component schemas, detects string formats, browser-local. |
| 2 | JSON Utils — JSON to OpenAPI Generator (`jsonutils.org/json-to-openapi.html`) | JSON sample + API metadata + an endpoint list (method picker) → full spec with components/`$ref`, security schemes, error responses. |
| 3 | DevOven — OpenAPI Schema Generator (`devoven.com/tools/openapi-schema`) | Minimal single-textarea generator → OpenAPI 3.0 schema fragment with inferred types, required fields, nested objects/arrays. JSON output only. |

## Table stakes observed → our decision

| Capability | Seen in | In model? | Our decision |
|---|---|---|---|
| Type inference from a JSON sample (string/integer/number/boolean/array/object) | 1,2,3 | yes | Core inference; integers distinguished from floats. |
| Nested object + array-of-object inference | 1,2,3 | yes | Recursive; array items are the **merge** of all observed elements (union of keys, `required` = keys present in every element). |
| String `format` detection (email, uri, date-time, date, uuid, ipv4) | 1,2 | yes | `detect_formats` (default on). |
| `required` list | 2,3 | yes | `required_props` (default on). |
| Extract nested objects into reusable `components.schemas` + `$ref` | 1,2 | yes | `extract_nested` (default off; implies component schemas). |
| Request/response schemas as components vs inline | 1,2 | yes | `components` (default on) — the whole point of a *stub* you paste into a spec. |
| Example values carried into the spec | 1,2 | yes | `include_examples` (default on) — emitted as operation-level `example` next to the schema. |
| Nullable handling | 1 | yes | OpenAPI 3.1 = JSON Schema 2020-12, so nulls become `"null"` in a type array (no 3.0 `nullable:` keyword). |
| YAML **and** JSON output | 1,2 | yes | `format` (yaml default). |
| API metadata: title, version, server URL, description | 1,2 | yes | `title`, `api_version`, `server_url`. |
| Endpoint definition: HTTP method + path | 2 | yes | `method` (enum), `path`; `{braced}` path segments become `parameters` entries automatically. |
| Query parameters | 2 | yes | `query` — paste a sample query string, get typed query parameters with examples. |
| Security schemes (bearer / apiKey / basic) | 2 | yes | `security` enum (default `none`). |
| Error response schemas (4xx/5xx) | 2 | yes | `include_error_responses` (default off) → 400 + 500 wired to a shared `Error` schema. |
| Operation id / tags | 2 | yes | `operation_id` (blank = derived from method + path), `tag` (blank = first path segment). |
| Status code of the sample response | 2 | yes | `status` (default 200) with the standard reason phrase as the response description. |
| Preset "Load Sample" button | 1,2 | yes | Five `[[example]]` preset chips on the page. |
| Runs fully client-side, nothing uploaded | 1 | yes | Same here — the block is pure Rust compiled to WASM. |
| Copy / download output, deep-linkable URL params | 1,3 | yes | Provided generically by the page generator (`format = "text"` gives copy + download; every param is a query param). |
| Swagger-UI live preview of the generated spec | 2 | **no** | Out of model — this repo renders generic static tool pages and does not embed third-party spec renderers. Listed, not built. |
| Multi-endpoint spec assembly (add many endpoints in one doc) | 2 | **no** | Out of model for a single-operation *stub* tool; `har-to-openapi` already covers multi-endpoint inference from a capture. Listed, not built. |
| Fetch the JSON sample from a live URL | 2 | **no** | Out of model: this is a pure (no-network) block by design. Listed, not built. |
| Dark mode toggle, QR-code/share buttons | 2,3 | **no** | Site-level chrome, owned by the private site repo — not a tool parameter. |

## Where we go beyond the three

- **OpenAPI 3.1** (they all emit 3.0) — proper `"null"` in type arrays instead of the removed
  `nullable:` keyword, and 2020-12 JSON Schema semantics.
- **Request *and* response in one run**, wired into a complete path-and-operation stub
  (`paths` → method → `requestBody` + `responses`), not just a bare schema fragment.
- **Array-of-object merge**: keys missing from some elements drop out of `required` and mixed
  scalar types become a type array, instead of only reading element 0.
- **Query-string sampling** into typed `parameters` entries.
- Deterministic derivation of `operationId`, `tags`, and response descriptions, so re-running
  the same input always yields a byte-identical stub.

## Stated limits (documented on the page, not silently mis-generated)

- One operation per run; a spec with many endpoints means many runs (or `har-to-openapi`).
- A sample can only reveal what it contains: a `null` value cannot reveal the underlying type,
  an empty array cannot reveal its item type, and no sample can reveal enums, constraints
  (`minimum`, `pattern`, …), auth scopes, or headers.
- `format` detection is heuristic and pattern-based; review before shipping the spec.
- The output is a *stub* meant to be edited — descriptions, examples for error cases, and
  operation summaries are placeholders derived from the inputs.
