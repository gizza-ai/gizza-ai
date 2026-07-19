# har-validator — competitor analysis (2026-07-17)

Scan done before finalizing the descriptor and page controls. Findings are paraphrased; no competitor copy, branding, or trademarks are reused. HAR captures may include URLs, cookies and headers, so browser-local validation is a core privacy requirement.

## Competitors and references surveyed

1. **HAR 1.2 specification** — defines the required top-level `log` object, `version`, `creator`, optional `browser` and `pages`, required `entries`, and the required fields inside each request/response/timings object.
2. **Google HAR Analyzer** — accepts a pasted/uploaded HAR and reports load/timing details; useful baseline for timing-summary UX but not a strict structural linter.
3. **HTTP Toolkit / HAR viewer tools** — emphasize local inspection of request/response data, searchable entries, and safe handling of sensitive headers.
4. **`har-validator` / schema-validator libraries** — validate required HAR object shapes and report schema paths for missing or wrong-typed fields.
5. **Browser DevTools HAR export workflows** — common real-world source of HAR files; exporters sometimes include extension fields and rounded timing values.

## Table stakes → decision

| Capability | Decision | Where |
|---|---|---|
| Parse HAR as JSON and reject non-JSON | **in-model** | core `validate` returns `invalid JSON` errors |
| Require top-level `log` object | **in-model** | `log` object check |
| Check required `log.version`, `creator`, and `entries` | **in-model** | core structural checks |
| Check required per-entry `request`, `response`, `cache`, and `timings` fields | **in-model** | entry validator with JSON paths |
| Check required request fields (`method`, `url`, `httpVersion`, `cookies`, `headers`, `queryString`, sizes) | **in-model** | request validator |
| Check required response fields (`status`, `statusText`, `content`, `redirectURL`, sizes) | **in-model** | response validator |
| Report all problems, not just the first | **in-model** | errors collected in one report |
| Include exact JSON paths | **in-model** | paths such as `log.entries[0].request.method` |
| Timing-total consistency (`entry.time` vs phase sum) | **in-model** | default-on `check_timings`, warnings only |
| Allow timing check to be disabled | **in-model** | boolean checkbox / CLI param |
| Ignore vendor extension fields | **in-model** | unknown JSON keys are not flagged |
| Browser-local validation, no upload | **in-model** | wasm page, no network |
| Full HAR waterfall/performance charts | **out-of-model (visual scope)** | needs viewer UI, not a validator report |
| Import HAR from a URL or browser extension | **out-of-model (input model/privacy)** | paste JSON only; no network fetch |
| Schema draft validation against a bundled official JSON Schema | **considered, not built** | hand-coded checks give clearer paths and avoid pulling a large schema dependency into wasm |
| Redaction of cookies/headers before sharing | **out-of-model (separate tool)** | validation only; redaction belongs in a privacy/scrubber tool |

## Result

Descriptor ships two controls: `har` (the pasted JSON) and `check_timings` (default true). The report states valid/invalid status, version, creator, page/entry counts, all structural errors, and timing warnings. This is not a duplicate of existing HTTP parsers: it validates complete HAR archive structure rather than individual HTTP messages.

Sources: HAR 1.2 spec, Google HAR Analyzer, HTTP Toolkit HAR documentation, schema-validator library behavior, and browser DevTools export workflows.
