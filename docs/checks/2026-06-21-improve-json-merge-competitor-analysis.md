# json-merge — competitor analysis & differentiation

**Tool:** `gizza-ai/json-merge` — deep-merge two or more JSON objects with
configurable array and conflict handling.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `jq -s 'reduce .[] as $x ({}; . * $x)'` | CLI | Powerful but an arcane incantation; `*` deep-merges objects but **replaces** arrays with no concat option, and you must remember the slurp+reduce form. |
| lodash `merge` / npm deep-merge | Library | Need to write JS; behavior (array merge) varies by library. |
| Online "JSON merge" tools | Web | Common, but most **upload your JSON**, and many only do a shallow merge. |
| Manual editing | DIY | Error-prone for nested structures. |

## How gizza's tool is better / different

1. **Local — your JSON never leaves the device.** Runs in WASM (chat SW + CLI +
   page).
2. **True deep merge.** Objects combine recursively; on a scalar/type conflict
   the **later document wins** (a predictable last-wins rule).
3. **Array handling you choose.** Default **replace** (later array wins) or
   **concatenate** — the option `jq`'s `*` doesn't give you.
4. **2 or more in one call.** Paste any number of JSON values (whitespace/newline
   separated) — merged left-to-right.
5. **Formatted or minified output**, key order preserved, three surfaces, one
   Rust core.

## Verification

Seven core unit tests: deep object merge, last-wins scalar conflict, array
replace vs concat, three newline-separated docs, single-doc formatting, and error
cases (empty / invalid / second-doc invalid). **End-to-end CLI**: deep-merge of
`{"a":1,"nested":{"x":1}}` + `{"b":2,"nested":{"y":2}}` →
`{"a":1,"b":2,"nested":{"x":1,"y":2}}`; array concat → `{"l":[1,2,3]}`. Page
Playwright covers both.

## Scope / honest limitations

- Inputs are concatenated JSON values (the streaming-parse trick), merged
  left-to-right. It's not RFC 7386 JSON Merge Patch (which deletes keys on
  `null`) — here `null` is just a value that wins. A `merge-patch` mode could be
  a future option.

## Possible future enhancements

- RFC 7386 JSON Merge Patch mode (null deletes keys).
- "First wins" conflict option.
- Array merge by index or by a key field.
