# postman-collection-converter — competitor analysis (2026-07-19)

Scan done BEFORE implementation (create-next-tool step: competitor scan). WebSearches:
"convert Postman collection to curl commands online tool", "Insomnia collection export to
fetch axios code converter tool", and a paste-JSON converter query; top 3 reachable real
tools skimmed. All notes are paraphrased — no competitor copy, branding, or trademarks
reproduced.

## Competitors skimmed

1. **DevToolBox Postman-to-cURL converter** (devtoolbox.harshasuraweera.com/blog/postman-to-curl)
   — the closest whole-collection competitor. Upload a Postman Collection v2.1 JSON export plus an
   OPTIONAL environment file; the tool substitutes `{{baseUrl}}`-style variables from the
   environment, walks every request, and renders one card per request (request name + generated
   curl command + a per-card copy button). Covers method, headers, and raw body.
2. **Postman's built-in code generator** (learning.postman.com code-snippet docs) — per-request,
   not per-collection: a code panel with a language dropdown (curl, JavaScript fetch, axios, and
   many more), copy to clipboard, variables resolved from the active environment. Output includes
   method, URL, headers, auth, and body. Limitation noted in third-party guides: pre-request /
   test scripts and dynamic values aren't reflected.
3. **curlconverter.com family** (curlconverter.com, smartformatter.com, tools.postly.ai) — the
   adjacent paste-based converters (curl → fetch/axios/etc.). UX table stakes: paste into a big
   textarea, target picker, instant conversion in the browser (nothing uploaded), copy button,
   sample/preset inputs. They convert single commands, not collections — our tool's differentiator
   is whole-collection batch conversion.

Insomnia side (Kong docs + GitHub issue #2110): Insomnia exports a JSON file
(`__export_format: 4`, `resources[]` with requests/request groups/environments) and its own
per-request code generation covers curl/fetch and friends; axios was a requested gap. Accepting
the Insomnia export JSON alongside Postman is in the backlog description and in-model.

## Table-stakes → decision (every item lands in the descriptor or the out-of-model list)

| Table-stake (seen at) | Tag | Where it landed |
| --- | --- | --- |
| Whole-collection batch conversion (DevToolBox) | in-model | core walks every request incl. nested folders; one labeled snippet per request |
| Postman Collection v2.0/v2.1 JSON input (DevToolBox, Postman) | in-model | `collection` param (multiline paste); format auto-detected |
| Insomnia export (format 4) input (backlog row, Kong docs) | in-model | same `collection` param; auto-detected via `_type: export` / `resources[]` |
| Target choice curl / fetch / axios (Postman dropdown, curlconverter family) | in-model | `target` enum `curl\|fetch\|axios`, default `curl`, friendly labels + preset chips |
| Environment-variable substitution (DevToolBox env upload, Postman active env) | in-model | optional `variables` param: Postman environment export JSON, plain JSON object, or KEY=VALUE lines; collection-level variables and Insomnia environment data applied automatically; unresolved `{{placeholders}}` left verbatim |
| Method, URL, headers, body in output (all 3) | in-model | full request extraction; disabled headers/params skipped |
| Body modes: raw JSON/text, urlencoded, form-data, file, GraphQL (Postman/Insomnia editors) | in-model | curl `--data-raw` / `--data-urlencode` / `-F` (+`@file`); fetch/axios `JSON.stringify`, `URLSearchParams`, `FormData`; GraphQL wrapped as a JSON body |
| Auth: basic, bearer, API key header/query (Postman auth tab, Insomnia auth) | in-model | curl `-u` / `Authorization` header / query append; fetch Basic → base64 header; axios native `auth:` option; collection-level auth inherited |
| Multi-line curl with `\` continuations (Postman codegen default) | in-model | `multiline` boolean, default true (fetch/axios are inherently multi-line) |
| Request name shown with each snippet (DevToolBox cards) | in-model | `# Folder / Name` (curl) or `// Folder / Name` (fetch/axios) comment above each snippet |
| Browser-local processing, nothing uploaded (curlconverter family) | in-model (copy) | inherent to the wasm page; stated in the copy |
| Sample input / presets (curlconverter family) | in-model | `[[example]]` chips: sample Postman collection, Insomnia export, variables demo |
| More language targets (Python, Go, PHP, Java, HTTPie — Postman/curlconverter) | out-of-model | backlog row scopes this tool to curl/fetch/axios; other targets listed, not built |
| Per-request cards with individual copy buttons (DevToolBox) | out-of-model | the generic page renders one combined output with a single Copy/Download; snippets are separated by labeled comments instead |
| Pre-request/test scripts, dynamic vars (`{{$guid}}`) resolution (Postman runtime) | out-of-model | scripts can't run here; dynamic placeholders stay verbatim (documented on the page) |
| OpenAPI/Swagger input (medium.com pipeline) | out-of-model | different input format; a candidate separate tool |
| .json file-upload widget (DevToolBox) | out-of-model | pure-tool pages are paste-based (multiline textarea); CLI takes the file via shell substitution |
| Insomnia v5 YAML export | out-of-model | JSON formats only (Postman v2.x, Insomnia format 4); stated in the limits |

## Design conclusions

- Pure tool (JSON text in → code text out) despite the backlog's `network` hint — nothing is
  fetched; classified `pure`, so it gets a page + CLI + chat.
- Caps: 200 requests per collection (clear error naming the count and the limit); tested at the
  boundary (200 ok, 201 rejected).
- Not a dup: `curl-command-parser` goes the other direction (curl → structured JSON);
  `http-request-builder` builds one raw HTTP/1.1 message from parts; neither reads
  Postman/Insomnia exports or emits fetch/axios code.
- Substitution precedence: user `variables` > collection/environment values; both `{{var}}` and
  Insomnia's `{{ _.var }}` template forms are handled.
