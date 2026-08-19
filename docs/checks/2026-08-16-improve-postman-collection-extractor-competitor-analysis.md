# postman-collection-extractor — competitor analysis (2026-08-16)

Scan run **before** implementation, per `/create-next-tool` step 4. One WebSearch
("postman collection JSON extract list of requests method URL headers body online tool"), then the
top reachable competitors were skimmed (two candidates were unreachable and were replaced — see
below). Everything here is **paraphrased**; no competitor copy, branding, or trademarks are
reproduced or reused anywhere in this tool.

## Scope check vs. existing gizza blocks (why this is not a duplicate)

Two neighbours were checked by reading their `core/src/lib.rs`, not their descriptions:

- `blocks/postman-collection-converter` — same input class (Postman v2.x / Insomnia export) but a
  **code generator**: its only outputs are `curl`, JavaScript `fetch()`, and `axios` snippets
  (`Target::{Curl,Fetch,Axios}`). It has no listing/inventory output, no per-request rows, no
  folder path, no CSV/JSON/Markdown, and no filters.
- `blocks/har-request-extract` — the same *shape* of deliverable (flat request inventory as
  table / CSV / JSON / URL list, with method + URL filters) but for **HAR captures**, a completely
  different input format and field set (status, timing, transferred size — response-side data a
  collection does not contain).

This tool is the Postman-input sibling of `har-request-extract`: a request **inventory** (what
endpoints exist, with their headers and bodies), not runnable code. The same split already ships in
this repo for curl (`curl-command-parser` parses to structure; `rest-file-to-curl` generates code),
so the precedent is established. Built, not skiplisted.

## Competitors reviewed

| # | Competitor | What it is | Reachable |
|---|---|---|---|
| 1 | `postmanparser` (appknox, Python library) | Programmatic Postman collection parser used to pull out requests/responses | yes |
| 2 | `postman2openapi` (kevinswiber; Rust + wasm, CLI + browser playground) | Converts a collection into an OpenAPI definition | yes (README) |
| 3 | `postman-to-openapi` (joolfe, Node CLI/library, archived Dec 2024) | Same conversion direction, options-file driven | yes |
| — | Postman Collection Viewer (openbrowsertools) | Browser viewer for collections: folders, methods, URLs, query params, headers, bodies | **no — HTTP 403**, described from the search-result snippet only, not counted as a skim |
| — | Konfig "Postman to OpenAPI" web tool | Browser converter | **no — HTTP 503**, replaced by #3 |

### 1. postmanparser (Python)

- Supports Postman Collection schema **v2.0.0 and v2.1.0**; input from a JSON file or a URL.
- `get_requests()` walks the collection **recursively through nested folders**; recursion can be
  turned off to list only root-level requests.
- `get_requests_map()` returns requests **keyed by folder path** (nested paths joined with `/`).
- `folder="path/to/folder"` restricts extraction to one folder subtree.
- Raises typed errors on schema violations (missing required field / invalid object).
- Documents unsupported areas (`protocolProfileBehavior`, full SDK parity).

### 2. postman2openapi (Rust/wasm)

- Input: collection JSON as a file argument **or stdin**; output YAML (default) or JSON via
  `-f/--output-format`.
- Ships as CLI, JS library, and **WebAssembly**, with a hosted browser playground — i.e. the
  privacy/offline story is "the conversion runs client-side".
- No documented request-listing mode; the deliverable is always a spec document.

### 3. postman-to-openapi (Node)

- CLI flags for output file (`-f`) and an **options JSON file** (`-o`); options include things like
  a default tag for grouping endpoints.
- Reads the standard request elements (method, URL, headers, body) as part of conversion.
- Archived and unmaintained as of Dec 2024.

### Cross-cutting observations

- Every serious competitor treats **nested folders** as the hard part and exposes either recursion
  control or folder-scoped extraction.
- Both conversion tools take the whole collection and emit **one document**; neither offers
  per-request filtering (method, URL substring) — that is the viewer's job, and the viewer is a
  browse-only UI with no exportable text output.
- `{{variable}}` placeholders are pervasive in real exports; conversion tools inline what they can
  and leave the rest, and users complain when a URL column is full of unresolved braces.

## Table stakes → our decision

| # | Capability seen | In-model? | Decision |
|---|---|---|---|
| 1 | Collection v2.0 **and** v2.1 exports | in-model | Supported; both shapes parse (v2.0 `url` string vs v2.1 `url` object are both handled). |
| 2 | Recursive traversal of nested folders | in-model | Always recursive; the folder path (`Users / Admin`) is a first-class column/field. |
| 3 | Folder-scoped extraction (`folder=`) | in-model | `folder` param — case-insensitive substring match on the folder path. |
| 4 | Per-request method, URL, headers, body | in-model | The core deliverable; `list`/`json`/`csv`/`markdown` all carry headers + body. |
| 5 | Machine-readable export (JSON) | in-model | `format = json` — array of request objects. |
| 6 | Spreadsheet export | in-model | `format = csv` (RFC-4180 quoting, so multi-line bodies survive). |
| 7 | Docs-ready export | in-model | `format = markdown` — a GFM table, for pasting into a README/wiki. |
| 8 | Just the endpoints | in-model | `format = urls` (one URL per line, deduped order preserved) and `format = table` (aligned overview). |
| 9 | Filter by method / URL substring | in-model | `method` + `url_contains` params (case-insensitive). |
| 10 | `{{variable}}` resolution | in-model | Collection-level `variable` array is applied by default, plus a user `variables` input (JSON object **or** `KEY=VALUE` lines, incl. a pasted Postman *environment* export). `resolve_variables = false` keeps placeholders verbatim. Unresolved placeholders are always left as-is, never blanked. |
| 11 | Clear errors on malformed input | in-model | Distinct messages for "not JSON", "not a Postman collection", "no requests found", and the request cap. Individual sloppy items stay forgiving (missing method → `GET`, missing URL → empty) so a partial export still lists. |
| 12 | Client-side / privacy | in-model | The page is wasm in the browser; nothing is uploaded. Stated on the page. |
| 13 | One-click sample data | in-model | Five `[[example]]` preset chips (overview table, headers+body detail, CSV, Markdown docs table, URL list). |
| 14 | Auth visibility | in-model (reduced) | An `auth` field reports the auth **type** (`bearer`, `basic`, `apikey`, …, or `inherited` when the request inherits collection auth). Secrets/tokens are deliberately **not** printed — see limits. |
| 15 | File upload of a `.json` export | out-of-model (page shape) | Pure tools use a paste field; the page has no file picker. Paste or `cat file.json` into the CLI. |
| 16 | Convert to an OpenAPI spec | out-of-model here | Different deliverable and already the job of a converter family; `blocks/har-to-openapi` covers the HAR direction. Not built. |
| 17 | Emit runnable curl/fetch/axios | out-of-model here | Already shipped as `blocks/postman-collection-converter`; linked in the FAQ instead of duplicated. |
| 18 | YAML output | out-of-model | JSON/CSV/Markdown cover the export need; adding a YAML serializer is not worth the dependency. |
| 19 | Fetch a collection by URL | out-of-model | Pure/offline block by design (no network); the CLI's `web-fetch` block covers URL retrieval separately. |
| 20 | Pre-request/test scripts, `protocolProfileBehavior` | out-of-model | Not extracted — this is a request inventory. Stated as a limit on the page. |

## Stated limits (on the page, not just in code)

- Postman **Collection v2.0/v2.1** exports only (a v1 export must be converted first).
- Up to 500 requests per run; each body is truncated at 2 000 characters in the output.
- Disabled headers are skipped; `{{placeholders}}` with no value are left verbatim.
- Scripts, tests, examples/saved responses, and cookies are not extracted.
- Auth is reported by type only — tokens, passwords, and API keys are never printed.
