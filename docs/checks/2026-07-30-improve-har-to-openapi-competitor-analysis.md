# har-to-openapi — competitor analysis (2026-07-30)

Tool: infers a draft OpenAPI 3.x spec from a HAR (HTTP Archive) capture by grouping
requests into paths + methods, inferring path/query parameters and request/response
JSON schemas. Pure Rust (serde_json + serde_yml), browser-local, no server, no account.

## Competitors scanned (paraphrased — no copy/branding reproduced)

1. **jonluca/har-to-openapi** (npm / CLI) — the reference generator.
   Options observed: `openapiVersion` (3.0.0 | 3.1.0), output `--format` json|yaml,
   `inferParameterTypes` (scalar type inference for query/path/form params, default off),
   `attemptToParameterizeUrl` + `minLengthForNumericPath` (collapse numeric/UUID path
   segments into `{param}`), `includeDomains`/`excludeDomains`/`urlFilter` (host/URL
   filtering), `dropPathsWithoutSuccessfulResponse`, `guessAuthenticationHeaders` +
   `securityHeaders`, `relaxedMethods`, `relaxedContentTypeJsonParse`,
   `includeNonJsonExampleResponses`, `tags` (URL→tag), `mimeTypes` filter, `pathReplace`
   normalization, `info{Title,Version,Description}` templates with `{domain}`/`{generatedAt}`.
   Input: HAR JSON (file or stdin). Output: OpenAPI YAML (default) or JSON.

2. **dcarr178/har2openapi** (CLI) — multi-file capture → docs pipeline.
   Path grouping via `pathReplace` search/replace normalization; collapses `/account/1`,
   `/account/2` → parameterized endpoint; JSON schema generation for request/response
   examples; `replace` section for redacting secrets anywhere in the spec; `tags` by path;
   emits JSON + YAML + helper method/path lists; merge-preserves manual edits.

3. **Mayhem HAR converter** — records API transactions as HAR, converts to an OpenAPI
   spec, infers the API base URL/server from the recording (positioning: fuzzing input).

4. **TheEagleByte/skylight-api** (CLI) — HAR→OpenAPI with schema inference from observed
   response bodies, path normalization, and auto-redaction of sensitive data; regenerates/
   updates Swagger UI + ReDoc docs.

5. **API Transformer / apievangelist workflow** — hosted conversion of proxy-captured HAR
   (Charles/etc.) into OpenAPI; positioned as a format-conversion service.

## Table-stakes → decision

| Capability | In/out of model | Decision |
| --- | --- | --- |
| OpenAPI version 3.0.x vs 3.1.0 | in | `openapi_version` enum (3.0.3, 3.1.0) |
| Output JSON vs YAML | in | `format` enum (yaml, json) — YAML default (competitor default) |
| Group requests → paths + methods | in | core: (path, method) grouping, always on |
| Response JSON schema inference per status | in | core: infer schema from captured body, always on |
| Request body JSON schema inference | in | core: infer from postData, always on |
| Query/header/path params collected | in | core: query + path params, always on |
| Parameterize numeric/UUID path segments | in | `parameterize_paths` boolean (default on) |
| Scalar type inference for params | in | `infer_types` boolean (default on) |
| Response/request example in spec | in | `include_examples` boolean (default on) |
| Host/domain filtering | in | `domain` substring filter param |
| Drop paths without a 2xx response | in | `drop_unsuccessful` boolean (default off) |
| Custom API title | in | `title` string (empty → inferred from first host) |
| Infer `servers` base URL from capture | in | core: derive `servers` from request origins, always on |
| Redact secrets in the spec | OUT (covered by sibling `har-redact`) | not built — pre-redact with `har-redact`, listed on page |
| `guessAuthenticationHeaders` → securitySchemes | considered, rejected | heuristic auth-scheme guessing is noisy + easily wrong; params/paths/schemas are the reliable core. Listed as a limit. |
| Per-URL `tags`, `pathReplace` rules, multi-file merge | OUT | needs config-file/callback surface that doesn't fit a single-input browser tool |
| Auto-update Swagger UI / ReDoc docs | OUT | needs a docs pipeline / server |

## Notes
- Never copy competitor copy/branding/trademarks; all page copy + code original.
- Redaction intentionally delegated to the existing `har-redact` block (single-responsibility);
  the page tells users to run that first if the HAR holds secrets.
