# Competitor analysis: graphql-introspect

Date: 2026-08-29
Tool: `graphql-introspect` — fetch a live GraphQL endpoint's introspection schema and render SDL/type-list/documentation output.

## Scan summary

Web search query: `GraphQL introspection tool online SDL schema type list`.

Reviewed the feature shape of the official GraphQL introspection documentation, GraphQL.js utilities, Apollo's schema representation guidance, and standalone GraphQL schema download tools such as `gql-sdl`. The repeated expectation is simple: send the standard introspection query to an endpoint, accept headers for protected APIs, and return a representation that downstream GraphQL tools can consume or humans can inspect. This implementation intentionally stays a chat/CLI network block: browser pages in this repo are for local wasm tools and do not provide a generic arbitrary-origin HTTP requester UI.

## Table-stakes found

| Capability / UX pattern | In-model decision | Implementation notes |
| --- | --- | --- |
| Send a standard GraphQL introspection query | Built | Conservative query with descriptions and deprecated members by default. |
| POST JSON request body | Built | `method=POST` default with JSON envelope and operation name. |
| GET query-string fallback | Built | `method=GET` appends/encodes `query=` for endpoints that require it. |
| Custom request headers | Built | `headers` string map for Authorization and API-specific headers; content negotiation defaults are added. |
| Return GraphQL SDL | Built | `format=sdl` default, including schema/root block when needed. |
| Return raw introspection JSON | Built | `format=json` returns the `__schema` object for other tooling. |
| Return a flat type list | Built | `format=types` lists type kind and member counts. |
| Return lightweight Markdown docs | Built | `format=markdown` emits per-type tables for documentation/review. |
| Include/exclude descriptions | Built | `descriptions` boolean controls query fields and printer output. |
| Include/exclude deprecated fields/enum values | Built | `include_deprecated` controls introspection arguments and local rendering. |
| Hide built-in scalars and introspection types by default | Built | `include_builtins=false` keeps output focused on API-owned types. |
| Stable alphabetical output | Built | `sort=true` sorts types/fields/args/enums for diffs. |
| Compatibility toggles for newer introspection fields | Built | `specified_by_url` and `repeatable_directives` are opt-in because older servers reject unknown fields. |
| Helpful disabled-introspection errors | Built | GraphQL `errors` arrays are reported before HTTP status fallback, with auth/disabled-introspection hints. |

## Out-of-model or deliberately rejected

| Feature | Reason |
| --- | --- |
| Standalone browser page that fetches arbitrary GraphQL endpoints | Rejected for this repo: arbitrary-origin network calls are a chat/CLI/network-block concern, and browser CORS would make many endpoints look broken. |
| Auth-token storage, login flows, or saved endpoint workspaces | Outside the no-account/no-storage public toolkit model. Pass headers per invocation instead. |
| Schema diff against a previous endpoint snapshot | Useful but a separate two-input comparison tool; this tool fetches and renders one live schema. |
| Running arbitrary GraphQL operations | Separate from introspection and riskier for side effects; this tool sends only the introspection query. |
| Full GraphQL IDE/playground | Outside scope; the CLI/chat surface returns schema artifacts for existing IDEs and code generators. |

## Resulting schema decisions

- `url` is the only required field.
- `headers` is a string map rather than a single header text blob so callers can pass structured auth headers.
- Fixed choices are enums: `method` (`POST`, `GET`) and `format` (`sdl`, `types`, `markdown`, `json`).
- Boolean toggles cover descriptions, deprecated members, built-ins, sorting, and newer GraphQL spec fields.
- No page or wasm-pack web target is included because this is a network-only GraphQL endpoint tool.
