# parse-http-message — competitor analysis (2026-06-21)

Snapshot of the top tools in the "raw HTTP request/response parsing" space, the
gaps vs gizza's `parse-http-message`, and what was (and was not) built. Findings
are paraphrased — no competitor copy, branding, or assets were reused.

## Surfaces verified (Phase 1)

All three surfaces of `parse-http-message` are live and green:

- **Chat / LLM API** — `cargo test --workspace` passes (14 core tests + the
  drift-guard schema test); `descriptor()` single-sources the chat schema and the
  drift-guard `schema_json_matches_authored_chat_schema` test confirms no LLM-facing
  drift.
- **CLI** — `gizza tool parse-http-message message="<raw>"` returns structured JSON
  for both a request (method/target/path/query) and a response (status code / reason /
  class), verified live.
- **Page** — Playwright (`tool-page-parse-http-message.spec.ts`, 2 tests) drives
  `/tools/parse-http-message/` for a request and a response-with-body; both pass.

## Competitor landscape

Most "HTTP parser" results are actually **live header-fetch** tools (give a URL, they
send a real request) or **curl ↔ code converters**. Very few do the pure
"paste raw HTTP text → structured fields, no network" job that this tool does, and
none combine response support + parse-without-sending + browser-local.

| Tool | Parses | Local? | Notes (paraphrased) |
|------|--------|--------|---------------------|
| curl h2c (curl.se) | requests only | server-side | Raw request → curl command; option toggles; converts on curl's servers. |
| curlconverter (/http/) | requests (from curl) | client-side | curl → 30+ targets incl. raw HTTP; sample examples; copy button; fully in-browser. |
| askapache HTTP Headers Tool | request + response | server-side | Sends a live request to a URL; status + headers + body + hexdump; rich status-code/header reference content. |
| oxylabs cURL→HTTP | requests only | unstated | curl → HTTP request, part of a converter suite; copy button. |
| Beeceptor HTTP Echo | request only | server-side | Receives a request you send to their endpoint and echoes the parsed parts. |

## Gap analysis (fit-to-model)

`parse-http-message` is differentiated: auto request-vs-response detection, headers in
wire order with **duplicates preserved** (e.g. multiple `Set-Cookie`), obsolete
line-folding handling, CRLF/bare-LF tolerance, `Content-Type` / `Content-Length` /
chunked convenience fields, body + byte length, dual **JSON** (chat/CLI) and
**human-readable text** (page) output, all 100% browser-local Rust/wasm with no upload.
No surveyed competitor combines parse-without-sending + response support + local-only.

**In-model (could be added to a pure browser-local parser):**
- Copy-to-clipboard buttons (curlconverter, oxylabs) — presentational page chrome.
- One-click sample request/response data — the page `content.md` already ships worked
  request and response examples; the placeholder seeds a request sample.
- Status-code / common-header reference content — SEO/educational; partly covered by the
  page copy describing the status classes.
- Header table (name/value columns) — purely presentational alternative to the wire list.
- Hexdump / byte view of the body — feasible locally.
- Syntax highlighting of the raw message — client-side only.
- Convert-to-curl as a bonus output — pure transform, but a separate tool's job.

**Out-of-model (need a server / account / actually sending the request — intentionally
NOT built):**
- Live URL fetch / sending the request to capture a real response (askapache, beeceptor)
  — needs a backend and breaks the no-upload promise.
- Echo-server request inspection (beeceptor) — requires a hosted endpoint + account.
- Following redirects / configuring an outbound User-Agent or cookies for a real call
  (askapache) — a network feature, out of scope for a parser.
- Code generation into 30+ languages (curlconverter, oxylabs) — technically a pure
  transform but scope-creep; a dedicated converter, not this parser.

## Decision

The tool already covers every in-model capability that matters for *parsing* a raw HTTP
message and beats the field on the core job (local, response-aware, duplicate-preserving).
The remaining in-model items (copy buttons, header table, hexdump, syntax highlighting)
are presentational page enhancements that belong to a shared page-chrome pass, not the
descriptor/core logic, and the worked request+response examples are already in the page
content. No competitor copy, branding, or trademarks were used. Out-of-model network
features are listed above as considered-not-built.
