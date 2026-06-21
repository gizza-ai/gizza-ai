# json-beautify — competitor analysis & differentiation

**Tool:** `gizza-ai/json-beautify` — pretty-print minified/messy JSON with
configurable indentation, validating it.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| jsonlint.com / jsonformatter.org | Web | Ubiquitous, but most **send your JSON to a server**, are ad-heavy, and key order is sometimes reordered. |
| `jq .` / `python -m json.tool` | CLI | The references, but a terminal and (for jq) sorts/format quirks; not paste-and-go. |
| Editor format-on-save | App | Editor-specific; needs the file open in a configured editor. |
| Browser devtools | App | Only for responses already in the network tab. |

## How gizza's tool is better / different

1. **Local — your JSON never leaves the device.** Runs in WASM (chat SW + CLI +
   page). The right default for config/secrets/API payloads.
2. **Beautify *and* minify in one knob.** `indent` 1–8 pretty-prints; `indent=0`
   collapses to one compact line.
3. **Validates as it formats.** Invalid input returns the parser's exact
   `line/column` message instead of silent/broken output.
4. **Preserves key order.** Objects keep their original order (via serde_json
   `preserve_order`) — many formatters alphabetize and lose meaningful ordering.
5. **Three surfaces, one Rust core**, dependency-light.

## Verification

Six core unit tests: 2- and 4-space pretty output, key-order preservation,
`indent=0` minify, validation rejects `{bad}` / trailing commas / empty, and
scalar/nesting handling. **End-to-end CLI**: pretty (kept `b` before `a`), minify
(`{"a":1}`), and an invalid input → `invalid JSON: key must be a string at line 1
column 2`. Page Playwright covers all three.

## Scope / honest limitations

- Strict JSON (RFC 8259) — comments / trailing commas (JSON5/JSONC) are rejected
  by design (that's what "validate" means here). A lenient mode could be a future
  option.
- Does not sort keys (intentional). A `sort_keys` toggle could be added.

## Possible future enhancements

- Optional `sort_keys`.
- JSON5/JSONC lenient parse mode.
- Tab indentation option.
