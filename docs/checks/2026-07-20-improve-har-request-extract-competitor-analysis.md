# har-request-extract — competitor analysis (2026-07-20)

New-tool build. One WebSearch ("HAR file viewer online analyzer extract requests
list method URL status timing"), skimmed three reachable competitors
(paraphrased — no copy/branding taken):

1. **Jam.dev HAR File Viewer** — upload .har/.json; request list with headers,
   payload, response drill-down; per-request durations (DNS, connect, SSL,
   response); spot failed/slow requests; export for sharing.
2. **Google Admin Toolbox HAR Analyzer** — upload; per-request method, URL,
   status, size, timing; status-class filter chips (1xx–5xx); free-text +
   field-scoped search (`request.url:term`); grouped-by-page vs flat view;
   redaction before sharing; times in UTC.
3. **singhajit.com HAR Viewer** — drag-drop OR paste HAR JSON textarea; table
   columns method, status, URL, content type, size, duration; filters by
   status class (2xx/3xx/4xx/5xx/errors), method, content type, URL; summary
   stats (request count, bytes over wire, decoded size, elapsed, status
   distribution); per-request waterfall; redact-secrets toggle; export
   filtered set as HAR; client-side only.

## Table stakes → decision

| Capability | Tag | Where |
|---|---|---|
| Per-request method, URL, status, content type, size, total time | in-model | table/CSV/JSON columns |
| Paste JSON input (textarea), no upload | in-model | `har` multiline field |
| Status-class filter incl. "errors" with failed status-0 requests | in-model | `status` enum (all/2xx/3xx/4xx/5xx/errors) |
| Method filter | in-model | `method` string (case-insensitive exact) |
| URL substring filter | in-model | `url_contains` string |
| Sort: slowest / largest first | in-model | `sort` enum (order/slowest/largest) |
| Summary line (count + bytes transferred) | in-model | table header line |
| Export/machine formats | in-model | `format` enum: table/csv/json/urls |
| Safe-to-share output (no headers/cookies/bodies) | in-model | by construction; stated on page |
| Preset one-click flows | in-model | 5 `[[example]]` chips (full table, errors only, slowest, API→CSV, URL list) |
| Graphical waterfall / per-phase timing bars | out-of-model | needs a canvas viewer UI, not a text tool; page states limit |
| Header/cookie/body drill-down per request | out-of-model | that's a viewer; deliberately excluded (privacy is the feature) |
| Grouped-by-page view | out-of-model | flat list only; capture `#` keeps cross-reference |
| Field-scoped query language (`request.url:x & …`) | out-of-model | three orthogonal filters cover the common cases |
| Redaction/export of a sanitized HAR | out-of-model | we never emit sensitive fields at all |
| File-upload drag-drop | out-of-model (page) | pure-tool pages are paste/deep-link; CLI takes the JSON arg |

## Design notes

- Extractor is FORGIVING (unlike sibling `blocks/har-validator`, which is the
  spec-strict complement): missing per-entry fields render `-`/null; only
  non-JSON / missing `log.entries` errors.
- SIZE preference: `response.bodySize` ≥0 → Chrome `_transferSize` → decoded
  `content.size` (each competitor picks one; we document the chain on-page).
- `#` column keeps the ORIGINAL capture index across filter/sort so rows stay
  findable in DevTools.
- 1xx status class omitted (informational responses are vanishingly rare in
  captures; `all` covers them).

## Verified

- `cargo test --workspace` (6 core tests incl. exact table + drift guard).
- CLI matrix: all 6 `status` values, all 4 `format` values, all 3 `sort`
  values, method + url_contains filters, invalid-JSON error (exit 1),
  generated page CLI example verbatim (succeeds).
- Playwright ×9: exact table, exact CSV, urls+slowest, json+largest (typed
  row equality), 5-way status matrix, method/URL filters, non-HAR error,
  example-chip run, full deep-link. No boolean params → no checkbox case; no
  advertised cap → no boundary case.
