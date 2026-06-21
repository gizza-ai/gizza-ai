# parse-uri — competitor analysis (2026-06-21)

Tool: **URI / URL Parser** — splits a URI/URL into RFC 3986 components (scheme,
userinfo→username/password, host, port, origin, path + segments + filename,
query→decoded key/value pairs, fragment) and returns JSON. Pure Rust, runs on
all three surfaces (chat/LLM API, CLI, browser page); nothing is uploaded.

## Surfaces verified

| Surface | Status | Notes |
| --- | --- | --- |
| Chat block (`wafer build`) | PASS | `gizza-ai/parse-uri v0.1.0`, block.wasm validated (312 KiB) |
| CLI (`gizza tool parse-uri uri=…`) | PASS | emits pretty JSON; verified scheme/host/port/origin/path_segments/filename/file_extension/query_params/fragment |
| Page (Playwright, 2 specs) | PASS | full https URL + relative reference; checks host lowercasing, origin, filename, decoded query params |
| Unit tests | PASS | 23 core + 1 drift-guard schema test |

## Competitors surveyed

1. **FreeFormatter — URL Parser / Query String Splitter** (URI.js based)
2. **iplocation.io — URL Parser / URL Splitter**
3. **Site24x7 — URL Splitter**
4. **Browserling — URL Parser**
5. **Python `urllib.parse`** (reference semantics)

## Feature diff (competitor → us)

| Capability | Competitors | parse-uri | Decision |
| --- | --- | --- | --- |
| scheme / protocol | yes | yes (lowercased) | covered |
| userinfo → username/password | yes | yes (percent-decoded) | covered |
| authority / host / port | yes | yes (host lowercased, IPv6 in brackets, numeric port) | covered |
| path | yes | yes (raw + percent-decoded view) | covered |
| **path segments** | partial | **yes** (decoded array) | **added in this pass** |
| **filename + extension** | yes (FreeFormatter) | **yes** | **added in this pass** |
| **origin** | partial | **yes** (scheme://host:port, userinfo dropped, host normalized) | **added in this pass** |
| query string + parsed params | yes (decoded table) | yes (ordered, decoded, duplicates kept, `+`→space, `;` separator, bare key = null) | covered (richer: keeps order + dups) |
| fragment / hash | yes | yes (percent-decoded) | covered |
| relative references | varies | yes (no scheme/host) | covered |
| mailto:/file:///IPv6 hosts | varies | yes | covered |
| runs locally / no upload | some | yes (pure wasm, all surfaces) | covered |

## Gaps intentionally NOT closed (out of model)

- **subdomain / domain / TLD split** (FreeFormatter, iplocation): correct splitting
  requires the Mozilla Public Suffix List (e.g. `co.uk`, `github.io` are not simple
  "last two labels"). Shipping a stale/partial PSL would give wrong answers; the full
  list is a large data dependency that does not fit gizza's pure-compute model. Left out
  rather than implemented incorrectly. The raw `host` is provided so users can apply
  their own PSL.
- No copy, branding, or layout was taken from any competitor.

## Changes made in this pass

Beyond the initial RFC 3986 split, this pass added three pure, high-value fields that
competitors expose: `origin` (normalized `scheme://host:port`), `path_segments`
(decoded array), and `filename` + `file_extension`. The chat schema (`uri` param) was
unchanged, so the drift-guard test still holds; the skill description and page copy were
updated to mention the new fields.
