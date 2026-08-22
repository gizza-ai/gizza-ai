# openapi-to-curl competitor analysis (2026-08-22)

## Scope

`openapi-to-curl` turns an OpenAPI 3.x or Swagger 2.0 spec into pasteable curl examples. It is an offline generator: no requests are sent, remote references are not fetched, and credentials default to placeholders.

## Competitor scan

| Competitor/tool shape | Table-stakes capabilities observed | Gizza fit decision |
| --- | --- | --- |
| Swagger UI / Redoc-style API docs | Shows each operation, expands parameters and request body schema, offers a curl snippet for the selected endpoint, and uses server/auth info from the spec. | In-model. This tool emits every operation at once and uses server URLs, parameters, request bodies and security schemes. Interactive “try it out” execution is out-of-model because gizza tools should not call the target API. |
| Postman / Insomnia OpenAPI import | Imports a spec into a collection, creates sample requests, supports auth variables, filters by folder/tag, and exports curl for a request. | In-model as deterministic text generation: auth placeholders, method/tag/path filters and shell variables. Workspace state, request history and live sending are out-of-model. |
| Browser OpenAPI-to-curl snippets / code generators | Paste/upload a spec, choose output language or curl, include sample bodies from schemas/examples, and copy snippets. | In-model for curl output and sample generation. Multi-language SDK/client generation is out-of-model for this slug and belongs to existing OpenAPI client/stub tools. |

## In-model table-stakes implemented

- JSON/YAML parsing with auto-detect and explicit format override.
- OpenAPI 3.x servers plus Swagger 2.0 scheme/host/basePath fallback.
- One command per operation under `paths`.
- Path, query, header and cookie parameters with examples/defaults/schema samples.
- JSON, form-urlencoded and multipart request body examples.
- Local `$ref` resolution with a bounded depth cap.
- Auth placeholders for bearer, basic and API-key schemes.
- Filters by HTTP method, tag and path substring.
- Output as shell script, bare commands, Markdown or JSON records.
- Page controls for optional fields, multiline output, comments, pretty bodies and depth.

## Out-of-model / intentionally not built

- Sending requests or validating responses against a live API.
- Fetching remote `$ref`s from the network.
- Generating SDKs or examples in languages other than curl.
- Persisted collections, environment management or request history.

## Verification targets

The verification matrix should cover:

- OpenAPI 3.x YAML with path/query params, request body and bearer auth.
- Swagger 2.0 base URL and body handling.
- Optional fields, filters and multiple output formats.
- Syntax errors, missing paths and unsupported option values.
- Browser page output plus a deep-link for non-default checkbox/select values.
