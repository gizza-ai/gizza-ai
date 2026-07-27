# json-sort — competitor analysis (2026-07-26)

Tool function: parse a JSON document, recursively sort every object's keys into a stable,
diff-friendly order, and re-serialize. Validates the input first so malformed JSON is rejected
with a line/column. Pure-Rust; runs locally (chat block + browser page + `gizza` CLI).

## Competitors surveyed

1. **JSONLint — JSON Sorter** (jsonlint.com/json-sort) — ascending (A→Z) / descending (Z→A);
   recursive through nested objects; optional case-insensitive; optional sort of primitive arrays.
2. **CodeShack — JSON Sorter** (codeshack.io/json-sorter) — "deep" recursive sort of both objects
   and arrays; pretty/minify output.
3. **HTML Code Generator — JSON Key Sorter** (html-code-generator.com/tools/json-key-sorter) —
   asc/desc, recursive nested objects, indentation options incl. minified.
4. **DataFormatterPro — JSON Sorter** (dataformatterpro.com/json-sorter) — sort by key name OR by
   value; asc/desc; case-sensitive/insensitive; recursive; optional arrays.
5. **DevToolbox — JSON Sorter** (devtoolbox.dedyn.io/tools/json-sorter) — recursive key sort,
   asc/desc, case-sensitive/insensitive toggle.

## Table-stakes params → our descriptor

| Capability                              | Competitors | In model? | Our param            |
|-----------------------------------------|-------------|-----------|----------------------|
| Recursive object-key sort               | all         | yes       | always on (core)     |
| Ascending / descending                  | all         | yes       | `order` = asc/desc   |
| Case-insensitive key comparison         | 1,4,5       | yes       | `case_insensitive`   |
| Also sort array elements                | 1,2,4       | yes       | `sort_arrays`        |
| Indentation / minify output             | 2,3         | yes       | `indent` (0–8; 0 minifies) |
| Input validation w/ error location      | JSONLint    | yes       | serde_json line/col  |

Every table-stake lands in the descriptor. Defaults match the common case: `order=asc`,
`sort_arrays=false` (preserve meaningful array order), `case_insensitive=false` (codepoint order),
`indent=2`.

## Out-of-model / deliberately not built

- **Sort by value** (DataFormatterPro): sorting object *keys by their values* is ill-defined for
  heterogeneous/nested values and rarely what users of a key-sorter want; a stable key order is the
  diff-friendly goal. Listed, not built.
- **Sort arrays by a chosen key path** (thetexttool's array-sorter): a different tool (sort an array
  of records by a field); out of scope for a whole-document key sorter. Not built.
- **File upload / drag-drop of `.json` files, live editor panes, share links**: site/UX chrome that
  belongs to the branded site repo, not this generic toolkit page.

## UX patterns adopted

- `order` renders as a `<select>` (asc/desc) with friendly labels.
- `sort_arrays` / `case_insensitive` render as checkboxes (off by default).
- `indent` is a number field (0–8).
- Preset `[[example]]` chips (like competitors' sample buttons): a nested-object sort and a
  minify+sort-arrays preset.

No competitor copy, branding, or trademarks were used; all page copy is original and generic.
