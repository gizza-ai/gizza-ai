# parse-query-string — competitor analysis (2026-06-22)

Tool: `blocks/parse-query-string`. Parses a URL query string into ordered
key/value pairs plus a structured object (repeated keys → arrays, PHP/Rails
bracket notation → nested arrays/objects, percent-decoding, optional `+`→space).

Surfaces verified: chat block (`wafer build` instantiates), CLI
(`gizza tool parse-query-string ...`), standalone page (Playwright, 3 specs).
Drift-guard schema test passes.

## Top competitors surveyed

1. **`qs` (npm, ~70M weekly dl)** — the de-facto Node parser. Parses nested
   bracket syntax `a[b][c]=v`, arrays `a[]=` / `a[0]=`, dot notation
   (`allowDots`), configurable depth/array limits, custom delimiters, charset
   handling, `ignoreQueryPrefix` (strip leading `?`).
2. **`query-string` (npm, Sindre Sorhus)** — simpler; flat parse with array
   format options (`bracket`, `index`, `comma`, `separator`), type coercion
   (`parseNumbers`/`parseBooleans`), sort, keeps order optionally.
3. **`URLSearchParams` (browser/Node built-in)** — flat, repeated keys via
   `getAll`, `+`→space, percent-decode. No nesting.
4. **PHP `parse_str` / Rails `Rack::Utils.parse_nested_query`** — the bracket
   nesting semantics our `structured` view mirrors.
5. **Online "query string parser" web tools** (e.g. FreeFormatter, onlinetool
   style pages) — paste a URL/query, show a key→value table; most are flat,
   some group duplicates. Ad-supported, send data implicitly via JS only.

## Capability diff (✓ have / + added this build / ✗ out of model / — declined)

| Capability | Status |
| --- | --- |
| Split on `&` and `;` | ✓ |
| Percent-decode keys + values (UTF-8, lenient on bad escapes) | ✓ |
| `+` → space, toggleable (form vs strict RFC 3986) | ✓ (`plus_as_space`) |
| Strip a leading `?` / whole-URL query part (`ignoreQueryPrefix`) | ✓ |
| Ordered pairs, duplicates preserved | ✓ (`pairs`) |
| Repeated keys → array (`URLSearchParams.getAll` / `qs`) | ✓ (`structured`) |
| Empty-bracket arrays `a[]=` | ✓ |
| Numeric-index arrays `a[0]=` (null-padded gaps, like `qs`) | ✓ |
| Named-bracket nested objects `user[name]=` (PHP/Rails/`qs`) | ✓ |
| Arbitrary-depth nesting `a[b][c]=`, arrays-of-objects `items[][id]=` | ✓ |
| Bare key → no value / empty value distinction | ✓ |
| Both a human table view and machine JSON | ✓ (page render + JSON) |
| Private / no-upload / offline | ✓ (runs locally, like the libraries; better than ad web tools) |

## Gaps considered, deliberately NOT built

- **Type coercion (`parseNumbers`/`parseBooleans`)** — `query-string` offers it,
  but it is lossy and surprising for IDs/leading-zeros/`"true"` strings; values
  stay as strings (matching `qs`/`URLSearchParams` default and our existing
  `parse-uri`). Declined for correctness, not a model limit.
- **`comma` array format (`a=1,2,3`)** — ambiguous (a value may legitimately
  contain commas); `qs` defaults it off too. Declined.
- **Custom delimiter / dot notation / depth & array limits config** — extra knobs
  that add schema surface for marginal benefit; `&`/`;` + bracket nesting covers
  the overwhelming majority of real query strings. Declined to keep the schema
  small for the chat surface.
- **Re-encoding / stringify (round-trip back to a query string)** — that is the
  inverse tool (the existing `url-encode` form mode); out of scope here.

## Conclusion

At parity with `qs`/`URLSearchParams`/PHP `parse_str` on the parsing semantics
that matter (nesting, arrays, repeated keys, percent + `+` decoding, prefix
stripping) and ahead of typical ad-supported web parsers by offering both a
readable view and structured JSON with full bracket-notation expansion, locally
and private. Remaining differences are config knobs / opinionated coercions
deliberately declined, not capability gaps. No out-of-model (ML/network)
features apply — this is a pure-compute tool.
