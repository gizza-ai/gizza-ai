# json-path-edit — competitor analysis (2026-07-29)

Tool function: get, set, or delete a single value at a dotted/bracketed path in a
JSON document (lodash / dot-object style, not RFC 9535 JSONPath). All notes are
**paraphrased** — no competitor copy, branding, or trademarks reproduced.

## Competitors scanned (top results for the tool's function)

1. **lodash `_.get` / `_.set` / `_.unset`** (library, the de-facto path-editing
   semantics). Path grammar: dot + bracket + array index; `set` auto-creates
   missing intermediate objects, and creates an array when the next key is a
   numeric index; `unset` deletes a key/element and returns success/failure.
2. **`dot-object`** (npm library). Transforms objects using dot notation; supports
   `pick`/`set`/`delete` by dotted path, array indexing, and bracket keys for keys
   that contain a dot.
3. **JSON Editor Online** (jsoneditoronline.org). Full visual/tree JSON editor:
   view, edit, format/pretty, repair, compare, query (JMESPath/JSONPath),
   transform, validate. Add/move/remove/duplicate fields via a GUI tree.
4. **"JSON Remove Keys / Filter"** (jsonviewertool.com). Remove or *keep* keys by
   dot-notation path, supports nested paths; batch multiple keys; remove vs keep
   modes; paste-in / paste-out.
5. **JSONPath Finder / Evaluator** (jsonpathfinder.org, javainuse). Click a node
   in a rendered tree to generate its path (dot or bracket); evaluate a JSONPath
   expression against a document.

## Table-stakes params / behaviors and where they land

| Capability | In/out of model | Decision |
|---|---|---|
| Get value at a path | in-model | `operation=get` |
| Set value at a path | in-model | `operation=set` |
| Delete key / array element | in-model | `operation=delete` |
| Dot notation (`a.b.c`) | in-model | path parser |
| Bracket + array index (`a[0]`, `a.0`) | in-model | path parser |
| Quoted keys for dotted/spaced keys (`["a.b"]`) | in-model | path parser |
| Auto-create intermediate objects/arrays on set | in-model | `set_rec` (lodash-parity) |
| JSON value typing on set (number/bool/null/object, else string) | in-model | `parse_value` |
| Pretty-print / compact output | in-model | `pretty` checkbox (default on) |
| Preset examples (get/set/delete) | in-model | `[[example]]` chips |
| Clear errors for missing path / wrong container type | in-model | typed error messages |

## Out-of-model (considered, not built — listed, not implemented)

- **Interactive visual tree editor** with click-to-edit nodes (JSON Editor Online):
  a GUI paradigm, not our single-recompute page model.
- **Click-a-node-to-generate-path** (JSONPath Finder): needs a rendered interactive
  tree; out of the field-in/field-out model.
- **Batch multi-key removal + "keep only these keys" mode** (JSON Remove Keys): this
  tool edits one path per run; a multi-path/filter tool would be a separate tool.
- **Full RFC 9535 JSONPath queries** (wildcards, slices, `$..`, filters): already
  covered by the sibling `jsonpath-query` tool; this tool is the single-value
  *editor*, deliberately using the simpler lodash path grammar.
- **Diff/compare, repair, schema validation** (JSON Editor Online): separate tools
  in the toolkit; out of scope here.
- **100 MB file handling / server upload**: browser-local wasm, so very large docs
  are bounded by tab memory — no server tier to offload to.

## UX patterns matched

- Preset "Try:" chips for the three operations double as worked examples.
- `operation` renders as a `<select>` (fixed choice → `Param::enumv`); `pretty` as a
  checkbox (default checked); JSON as a multiline paste area.
- Deep-linkable via `?path=…&operation=…` query params.
- Errors state the failing segment and the expected container type rather than a
  bare "invalid input".
