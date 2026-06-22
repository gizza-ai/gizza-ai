# jsonpath-query — competitor analysis (2026-06-22)

New tool. Evaluates a JSONPath expression (RFC 9535) against a JSON document, in the
browser, with a pure-Rust engine (`serde_json_path`). Three surfaces verified:
chat block (wafer fixture), CLI (`gizza tool jsonpath-query`), and the standalone page
(Playwright incl. deep-link query params + the wrap option).

## Top competitors surveyed (paraphrased — no copy/branding reused)

1. **jsonpath.com** — single text area for JSON + a JSONPath box; shows matches as a
   JSON array. Goessner-style syntax (`$.store.book[*].author`). No filter-result-list
   toggle, no offline guarantee stated.
2. **jsonpathfinder / jsonpath.herokuapp.com (Stefan Goessner reference impl)** — the
   classic `$..` / `[?(@.x)]` Goessner dialect; emits a flat result list. Reference for
   the syntax most users expect.
3. **jsonquerytool.com** — multi-language playground (JSONPath, jq, JMESPath, JSON
   Pointer). For JSONPath it offers expression + document and prints matched values.
4. **VS Code / browser-extension JSONPath evaluators** — inline evaluation, error
   surfacing on a bad path, copyable output.
5. **online RFC 9535 validators (e.g. jsonpath playgrounds tracking the IETF spec)** —
   emphasize standards conformance (normalized paths, filter spec, slices, function
   extensions like `length()`/`count()`).

## Gap diff vs our tool

| Capability | Competitors | Ours | Status |
|---|---|---|---|
| Child access `.a` / `['a']` | yes | yes | covered |
| Wildcard `[*]` | yes | yes | covered |
| Array index / negative index | yes | yes | covered |
| Array slice `[start:end:step]` | partial | yes | covered |
| Recursive descent `$..` | yes | yes | covered |
| Filter selector `[?(@.x < n)]` | yes | yes | covered |
| RFC 9535 conformance | partial (most are Goessner-era) | yes (serde_json_path) | **advantage** |
| Result-list (single JSON array) vs per-node | array-only or node-only | both (`wrap` toggle) | **advantage** |
| Pretty / compact output | mixed | yes (`pretty` toggle) | covered |
| Clear error on invalid path / JSON | yes | yes (`invalid JSONPath: …`) | covered |
| Fully offline, no upload, no account | rarely guaranteed | yes (wasm, browser-local) | **advantage** |
| Deep-link / shareable query (URL params) | rare | yes (page `?path=&json=`) | **advantage** |

## In-model gaps — all closed at build time

The build already ships every JSONPath capability a browser-local tool can offer:
the full RFC 9535 grammar (via `serde_json_path`), both output shapes (`wrap`), and
formatting (`pretty`). No additional in-model capability gap remained after the first
pass, so no follow-up capability edits were required.

## Out-of-model features considered, not built

- **Multi-language playground** (jq / JMESPath / JSON Pointer in one page) —
  gizza ships these as **separate** tools (`jq-query` already exists); a combined
  playground is a UX/product choice, not a capability gap, and out of scope for one tool.
- **Server-side batch / file-upload of huge documents** — needs a backend; gizza is
  browser-local by design.
- **Saved/shared snippets with accounts** — needs auth + storage; out of model. (The
  URL deep-link already gives stateless sharing.)

## Notes

- JSONPath ≠ jq: distinct query language and syntax (`$.store.book[*]` vs `.store.book[]`),
  so this is **not** a duplicate of the existing `jq-query` block — it serves users who
  know/need the RFC 9535 JSONPath dialect.
- `serde_json_path` instantiates cleanly in the wafer runtime (wasm32-wasip1) and the
  browser (wasm32-unknown-unknown) — pure, no WASI/host imports.
