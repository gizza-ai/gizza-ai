# http-header-parser — competitor analysis (2026-07-23)

Function: paste a raw HTTP request/response header block, get a structured,
case-normalized JSON map of `name → value` with configurable name casing and
duplicate-folding. All observations paraphrased from public feature
descriptions — no competitor copy, branding, or trademarks reproduced.

## Competitors scanned

| # | Tool (paraphrased) | Notable features (paraphrased) |
|---|--------------------|--------------------------------|
| 1 | ToolDock HTTP Headers Parser | Raw headers → key/value table, copy-as-JSON, runs fully in-browser, no upload. |
| 2 | jsontool.net Header→JSON | Request/response headers → JSON; offers both key-value object and pure-array output forms. |
| 3 | toolharbor.dev HTTP Header Formatter | Normalizes header names to Title-Case; converts both directions raw↔JSON; in-browser only. |
| 4 | KeJson Headers Formatter/Converter | Accepts a full paste incl. request/status line and ignores it; handles values that contain colons; exports JSON/cURL/Python. |
| 5 | getjsontools.com Header→JSON | Request/response headers → JSON object for storing/organizing header data. |

Sources: [ToolDock](https://tooldock.org/tools/http-headers-parser),
[jsontool.net](https://jsontool.net/header2json/),
[toolharbor.dev](https://toolharbor.dev/tools/http-header-formatter),
[KeJson](https://www.kejson.com/en/format/header/),
[getjsontools.com](https://www.getjsontools.com/http-header-json/).

## Table-stakes → decision

| Capability (table-stake) | In competitors | Our decision | Fit |
|--------------------------|----------------|--------------|-----|
| Raw headers → JSON map | 1–5 | Core output: `headers` object in first-seen order | in-model ✅ |
| Ignore/strip leading request or status line | 1, 4 | Detected, reported in `kind`, returned in `start_line` (not dropped) | in-model ✅ |
| Values that contain colons (e.g. URLs, times) | 4 | Split on first `:` only; value preserved verbatim (unit-tested) | in-model ✅ |
| Case-insensitive name matching | 3 | Names folded case-insensitively before output | in-model ✅ |
| Title-Case / canonical name output | 3 | `case=canonical` (default), with ETag/WWW-Authenticate special cases | in-model ✅ |
| Alternate casings (lower/upper/original) | — (differentiator) | `case=lower|upper|original` | in-model ✅ |
| Duplicate-header handling | 2 (array form) | `duplicates=combine|list|first|last` — richer than array-only peers | in-model ✅ |
| Set-Cookie kept unjoined | implied by correctness | Never comma-joined under `combine` (RFC 6265) — stays an array | in-model ✅ |
| Runs fully in browser, no upload | 1, 3 | Pure wasm, nothing uploaded — stated in page copy | in-model ✅ |
| Reports counts / which names duplicated | partial | `count`, `line_count`, `duplicates[]` | in-model ✅ |
| Obsolete folded continuation lines | — | Continuation (leading WS) lines joined onto prior header | in-model ✅ |
| Export as cURL / Python snippet | 4 | **Out of model** — this tool emits a canonical JSON map; code-gen is a separate concern | out-of-model |
| Bidirectional JSON → raw headers | 3 | **Out of model** — reverse direction is a distinct tool; kept single-purpose | out-of-model |
| Copy-to-clipboard button | 1 | Out of scope here — provided by the generic page shell, not the block | platform |

## Differentiation vs. sibling gizza tools

- `parse-http-message` — parses a *full* HTTP message (start-line fields, HTTP
  version, headers as an ordered **list**, and body). This tool instead folds
  headers into a normalized **map** with casing + duplicate policies and no
  body. Distinct output shape and use case.
- `http-header-analyzer` — *explains* headers and flags missing security
  headers. This tool does structural normalization only, no analysis.
- `phishing-header-inspector` — email-header spoofing heuristics. Unrelated.

Not a duplicate: this is the "clean up headers into a comparable JSON object"
niche (casing control + duplicate folding), which no sibling covers.

## Decisions

Every table-stake ends in the descriptor or the out-of-model list above; none
dropped silently. In-model gaps closed at build time: canonical-casing default,
alternate casings, four duplicate policies, Set-Cookie safety, start-line
capture, colon-in-value preservation, continuation folding, and count/duplicate
reporting. Out-of-model: cURL/Python code-gen and reverse JSON→raw conversion
(separate single-purpose tools). No competitor text or branding was copied.
