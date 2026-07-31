# Competitor analysis — sort-json-array (2026-07-31)

Tool function: sort a JSON **array of objects** by one or more object keys, ascending or
descending — the classic `_.orderBy` / SQL `ORDER BY` operation. Distinct from the existing
`json-sort` block, which reorders object **keys** alphabetically (canonicalization) and cannot
sort array *elements* by the value of a chosen field.

Method: one WebSearch for the function + skim of the real competitor tools. Two tool pages
(codeshack.io/json-sorter, codebeautify.org/json-sorter) returned HTTP 403 to the fetcher, so per
the recipe ("unreachable → replace it") I supplemented them with the canonical semantic reference
(lodash `_.orderBy`) and search-snippet feature descriptions. All copy below is paraphrased —
no competitor copy, branding, or trademarks reproduced.

## Competitors skimmed

1. **codeshack.io/json-sorter** (snippet-level; page 403'd the fetcher) — "sort arrays of objects
   by a specific property," flip to descending, browser-local (no upload), also sorts nested
   objects alphabetically / by key length.
2. **codebeautify.org/json-sorter** (snippet-level; 403'd) — rearrange a JSON object or array by
   explicit criteria: sort objects by a particular key, array elements ascending/descending; file
   upload supported (out-of-model — server/file-upload UX; we take pasted text + a URL is N/A for a
   pure tool).
3. **dataformatterpro.com/json-sorter** (snippet-level) — organise by key name **or value**,
   ascending/descending, all local in-browser.
4. **thetexttool.com json-array-sorter guide** — "sort JSON arrays by any key, nested path or
   number": nested paths via **dot notation** (`user.name`), numeric-aware sorting, and multiple
   keys with mixed direction expressed as a comma list with a `-` prefix for descending
   (`dept,-salary,name`).
5. **lodash `_.orderBy`** (canonical semantic reference) — keys as strings incl. dot-notation
   nested paths; **per-key** asc/desc; numbers sort numerically out of the box; null/undefined
   sort to the end by default; stable multi-key ordering.

## Table-stakes → decisions

| Table-stake | Competitors | Decision |
| --- | --- | --- |
| Sort array of objects by a chosen key | all | **in-model** → `keys` param (required) |
| Multiple keys, stable secondary sort | lodash, thetexttool | **in-model** → comma-separated `keys` |
| Nested path via dot notation (`a.b`, array index `a.0`) | thetexttool, lodash | **in-model** → path resolver in core |
| Per-key ascending/descending | lodash, thetexttool | **in-model** → `-`/`+` prefix per key overrides the global `order` |
| Global ascending/descending default | all | **in-model** → `order` enum (asc/desc) |
| Numeric-aware vs alphabetical (auto) | thetexttool, lodash, dataformatterpro | **in-model** → JSON number values compare numerically, strings lexicographically, mixed types get a total type-rank order |
| Case-insensitive string comparison | json-sort sibling, common | **in-model** → `case_insensitive` boolean |
| Null / missing key placement | lodash ("to the end") | **in-model** → `missing` enum (last default / first); JSON `null` treated as empty too |
| Pretty-print / minify output (indent) | codebeautify, formatters | **in-model** → `indent` integer 0–8 (0 = minify), mirrors json-sort |
| Sort **object keys** alphabetically | codeshack, dataformatterpro | **out of scope here** — already shipped as the separate `json-sort` tool; this tool is array-element sorting only (cross-linked in copy) |
| File upload / batch files | codebeautify | **out-of-model** — needs server/file-upload UX; this tool takes pasted JSON text, runs fully in-browser |
| Sort by value across a whole object tree | dataformatterpro | **out-of-model for this tool** — that's key/whole-doc canonicalization, covered by `json-sort` |

## UX control patterns matched

- `keys` — multiline text field (accepts a comma list; placeholder shows the `-desc` prefix form).
- `order` — `<select>` (asc/desc) via `Param::enumv` with friendly `[input.labels]`.
- `missing` — `<select>` (last/first) via `Param::enumv`.
- `case_insensitive` — checkbox (boolean).
- `indent` — number field (0 = minify), matching the json-sort sibling.
- `[[example]]` preset chips: sort by one numeric key; multi-key `dept,-salary`; nested dot-path
  with case-insensitive strings — the competitors' "flip order / by property" affordances as
  one-click presets.

Every table-stake above lands in the descriptor or is explicitly listed out-of-model. No feature
dropped silently.
