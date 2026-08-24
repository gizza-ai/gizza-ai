# openapi-to-fetch-client — competitor analysis (2026-08-21)

Scan run BEFORE implementation. One WebSearch ("generate typed fetch client TypeScript from
OpenAPI spec tool") + WebFetch of the top reachable tools/docs. All notes are paraphrased
observations of *capabilities*; no competitor copy, wording, or branding is reused anywhere in
this tool. (npmjs.com 403s the fetcher, so the `openapi-typescript-fetch` notes come from its
GitHub README instead — an unreachable source was replaced, not dropped.)

## Tools reviewed

| # | Tool | What it does |
|---|------|--------------|
| 1 | RootUtils — OpenAPI to TypeScript Client Generator (`rootutils.com/tools/openapi-client-generator`) | Browser tool: drop a JSON/YAML spec, set base URL + client class name, get a lightweight fetch wrapper. Client-side only; live endpoint count, copy/download, reset. |
| 2 | OpenAPI Generator — `typescript-fetch` generator (`OpenAPITools/openapi-generator`) | The reference CLI generator. Emits `*Api` classes over `fetch` plus model files and a runtime module; ~20 config options (naming conventions, single-request-parameter, string enums, date library, interfaces, import extension, npm packaging). |
| 3 | openapi-fetch (`openapi-ts.dev/openapi-fetch`) | Runtime library, not a codegen: `createClient({ baseUrl })` typed by `openapi-typescript`'s `paths`. Params passed as `{ params: { path, query }, body }`; every call returns `{ data, error, response }`. |
| 4 | openapi-typescript-fetch (`sadams/openapi-typescript-fetch`) | Runtime library: `Fetcher.for<paths>()` + `configure({ baseUrl, init, use })`; path/query/body merged into ONE argument object; non-2xx **throws** `ApiError`; OpenAPI 3.0 + Swagger 2.0. |

## Table stakes observed → our decision

| Capability | Seen in | In model? | Our decision |
|---|---|---|---|
| Read OpenAPI 3.x **and** Swagger 2.0 | 1,2,4 | yes | Both. 3.x `requestBody`/`content`, 2.0 `in: body` + `produces`/`consumes`, `host`+`basePath`+`schemes`. |
| JSON **and** YAML input | 1,2,4 | yes | `input_format` = `auto` (default) / `json` / `yaml`. |
| One typed function per operation | 1,3 | yes | `style = "functions"` (default). |
| A single client **class** with methods | 1,2 | yes | `style = "class"` + `client_name` (default `ApiClient`), constructor takes shared options. |
| Configurable base URL baked into the client | 1,2,3,4 | yes | `base_url`; blank falls back to the spec's `servers[0].url` (3.x) or `schemes/host/basePath` (2.0), then `""`. |
| Configurable client/class name | 1,2 | yes | `client_name`. |
| Single request-parameter object per call (`useSingleRequestParameter`) | 2,3,4 | yes | `param_style = "object"` (default) — one typed request interface per operation. |
| Positional arguments instead | 2 (`useSingleRequestParameter=false`) | yes | `param_style = "positional"` — path params in path order, then body, then a query/header object. |
| Throw `ApiError` on non-2xx | 2,4 | yes | `error_handling = "throw"` (default) — an `ApiError` class carrying `status`, `response`, parsed `data`. |
| Return `{ data, error, response }` instead of throwing | 3 | yes | `error_handling = "result"` — an `ApiResult<T>` discriminated union. |
| Path params substituted + URL-encoded automatically | 1,2,3,4 | yes | Template literals with `encodeURIComponent`. |
| Query params serialized (incl. repeated array values) | 2,3,4 | yes | `URLSearchParams` helper; `undefined`/`null` skipped, arrays repeated. |
| Header params per operation | 2,4 | yes | Typed into the request interface and merged into the request headers. |
| Typed request/response bodies from `$ref`ed schemas | 1,2,3,4 | yes | `types_module` (default `./types`) → `import type { … }`; blank instead emits local `type X = unknown;` aliases so the file still compiles standalone. |
| Custom `fetch` implementation / init options | 2,3,4 | yes | `RequestOptions.fetch`, `.headers`, `.signal`, `.baseUrl` per call and per client. |
| JSDoc from `summary`/`description`, `@deprecated` | 2 | yes | `jsdoc` (default on) on functions and request-interface fields. |
| Function naming from `operationId` | 1,2,3,4 | yes | `naming = "operation_id"` (default), camelCased and de-duplicated; falls back to method+path when absent. |
| Naming derived from method + path | 2 (fallback) | yes | `naming = "path"` — always derive, ignoring `operationId`. |
| Generate only a subset of the API (per-tag) | 2 (`apis=` global property) | yes | `tags` — comma-separated tag filter, blank = every operation. |
| Indent / formatting control | 2 (via templates) | yes | `indent` 0-8 spaces, default 2. |
| Copy / download output; sharable deep links | 1 | yes | Provided generically by the page generator (`format = "text"`), every param is a URL query param. |
| Sample-spec preset buttons | — (1 has Reset only) | yes | Four `[[example]]` preset chips — we go past all four here. |
| Runs fully client-side, spec never uploaded | 1,3 | yes | Same: pure Rust compiled to WASM, no network. |
| Model/interface files for `components.schemas` | 2 | **no** | Out of model here *by design*: `openapi-to-typescript-types` already emits those, and this tool imports them via `types_module`. Listed, not built. |
| Multi-file output (`apis/`, `models/`, `runtime.ts`, `package.json`, npm publishing) | 2 | **no** | Out of model: a gizza tool returns one text result, not a file tree. Listed, not built. |
| Property/param/enum **naming conventions** (camelCase / PascalCase / snake_case / original) | 2 | **no** | Out of model at this scope: renaming wire fields requires a full model layer + request/response mappers, which is the model-generation job we deliberately delegate. Wire names are preserved verbatim. Listed, not built. |
| `date` vs `string` date library mapping | 2 | **no** | Listed, not built — `format: date-time` stays `string` (what `fetch` + `JSON.parse` actually give you); a Date mapping needs revivers in the model layer. |
| Middleware / interceptor chain (`use: [...]`) | 3,4 | **no** | Out of model: that is a runtime library feature. The generated `RequestOptions.fetch` hook covers the same need in one line. Listed, not built. |
| Auth helpers (bearer/apiKey wiring from `securitySchemes`) | 2 | **no** | Listed, not built — `headers` on `RequestOptions` is the escape hatch; the page documents it. |
| Resolving **external/remote** `$ref` documents | 2 | **no** | Needs network; this is a pure block. Local `#/components/schemas/...` refs become named type references. Listed, not built. |
| Live Swagger-UI style spec preview | 1 | **no** | Site chrome; this repo renders generic static tool pages. Listed, not built. |

## Where we go beyond the four

- **Both error idioms from one input**: the throw-`ApiError` convention (2,4) and the
  `{ data, error, response }` convention (3) are a single enum flip, so the same spec can feed
  either house style.
- **Both call conventions**: request-object *and* positional args, which #2 only exposes as a
  build-time generator property and #3/#4 don't offer at all.
- **No runtime dependency and no file tree** — one self-contained `.ts` file with an inlined
  `apiFetch`/`encodeQuery` helper, so it drops into any project (or a Deno/Bun script) as-is.
- **Composes with the sibling types generator**: `types_module` points at whatever file holds
  `components.schemas` types, so this tool stays focused on the operations layer.
- Deterministic output: same spec + same options always yields byte-identical source
  (sorted imports, stable de-duplicated names, key order preserved).

## Stated limits (documented on the page, not silently mis-generated)

- Only **local** `$ref`s (`#/components/schemas/X`, `#/definitions/X`) become named types;
  external/remote refs fall back to `unknown`.
- Bodies: `application/json` is preferred, then the first declared content type. `multipart/form-data`,
  `application/x-www-form-urlencoded` and Swagger 2.0 `in: formData` params are **not** assembled
  for you — the body is passed through and flagged in a comment.
- `in: cookie` parameters are noted in a comment and not sent (browsers do not allow scripts to
  set the `Cookie` header).
- The success type is taken from the lowest 2xx response (then `default`); other status codes are
  runtime `unknown` — in `result` mode they arrive as `error`.
- No auth wiring, no retries, no middleware chain: use `RequestOptions.headers` / `.fetch`.
- Schema validation is not performed; a spec that parses but is semantically wrong generates
  code that mirrors it.
