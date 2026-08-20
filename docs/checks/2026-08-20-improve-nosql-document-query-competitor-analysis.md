# nosql-document-query — competitor scan (2026-08-20) → SKIPLISTED as a duplicate

Backlog row: `tools-to-build.csv:1486` — `nosql-document-query`, "Queries an uploaded or pasted
JSON document collection with filters, projections, and JSONPath-style selectors.", use case
"from this JSON array, find all documents where status is active and project just name and id",
type_hint `pure`, feasibility note "serde_json parse plus a filter/JSONPath evaluator, all
in-memory like an embedded NoSQL store".

Outcome: **not built.** The scan was run BEFORE implementation (per the build procedure) and every
table-stake it surfaced is already shipped by `blocks/mongo-query` (filters + projection + cursor
chain over a pasted document collection) and `blocks/jsonpath-query` (RFC 9535 selectors), with
`blocks/jq-query` / `blocks/jsonata-query` / `blocks/ndjson-filter` covering the remaining query
dialects. Recorded in `docs/tool-skiplist.txt`. All competitor notes below are paraphrased
feature observations — no competitor copy, wording, or branding was reused.

## Competitors skimmed

Search: "online tool query JSON document collection filter projection JSONPath NoSQL playground".
Top 3 real tools that were reachable and functional:

### 1. JSONPath tester (jsoncraft.dev/jsonpath)
- Query surface: one JSONPath expression box over a pasted JSON document.
- Syntax advertised: root `$`, dot/bracket child access, array index, slice `[1:3]`, negative
  index, wildcard `*`, recursive descent `..`, filter predicates `[?(@.price > 10)]`, multi-key
  union `[title,author]`.
- Output: matched nodes as a JSON array **plus a match count**; explicit parse/expression errors
  instead of a silent empty result.
- UX: re-evaluates on keystroke, example dropdown of common patterns, all client-side.
- Limits: none documented beyond browser memory.

### 2. JSON Query Explorer (onlinewebdevtools.com/json-query)
- Query surface: JavaScript expressions over a `data` root — `.filter()`, `.map()`, `.reduce()`,
  optional chaining — pitched explicitly as the JSONPath-style-selector equivalent
  (`data.products.filter(p => p.price > 100)`).
- Input: Monaco editor with syntax highlighting, a "sample" button that loads a nested demo
  document, recent queries remembered locally for autocomplete.
- Output: pretty-printed, syntax-highlighted JSON; copy-to-clipboard; download as `.json`.
- Errors: JS exceptions shown as hints rather than breaking the pane.
- Limits: "several megabytes", bounded by tab memory.

### 3. Multi-dialect JSON query playgrounds (jsonfmt.dev/json-query-language and the
   JSONPath/JMESPath tool at jsonviewertool.com/json-path)
- Query surface: a dialect switch — JSONPath, JMESPath, and a jq playground — over the same
  pasted document, so the user picks the language instead of the tool.
- JMESPath is the one that markets "filtering and projection" as the headline pair (its
  multiselect-hash `[?status=='active'].{name: name, id: id}` is exactly the backlog use case).
- Output: evaluated result as formatted JSON, live as you type.
- Limits: client-side, browser-bound.

## Table stakes distilled → where gizza already has them

| Table stake | Competitor evidence | Already shipped |
| --- | --- | --- |
| Paste a JSON array of documents (a "collection") | all three | `mongo-query` `data` (JSON array, single object, or NDJSON; caps 5 MB / 50 000 docs) |
| Predicate filter over documents | all three | `mongo-query` `query` — `$eq $ne $gt $gte $lt $lte $in $nin $exists $type $regex/$options $mod $all $size $elemMatch $not $and $or $nor`, dotted paths, array/missing-field semantics, relaxed shell syntax |
| Projection (keep/drop a field subset) | JMESPath multiselect-hash; JS `.map()` | `mongo-query` `projection` — Mongo form `{"name":1,"_id":0}` and short form `name, email` / `-password`, dotted paths |
| JSONPath-style selectors (wildcards, slices, recursive descent, filter predicates, unions) | jsoncraft, jsonviewertool | `jsonpath-query` — RFC 9535 via `serde_json_path`, plus `wrap`/`pretty` |
| Match count | jsoncraft | `jsonpath-query` returns `count`; `mongo-query` has `format=count` |
| Sort / paginate results | cursor-style tools | `mongo-query` `sort` / `skip` / `limit` |
| Result export (copy/download, non-JSON shapes) | JSON Query Explorer download | `mongo-query` `format` = json / ndjson / csv / count; every generated page ships a Download link for `format = "text"` |
| Clear errors, not silent empties | jsoncraft, JSON Query Explorer | `mongo-query` rejects `$where`/`$expr`/`$jsonSchema`/`$text`/geo/bitwise with an explicit message; both blocks surface parse errors |
| Alternate query dialects (jq, JSONata, JMESPath-ish) | jsonfmt.dev, jsonviewertool | `jq-query`, `jsonata-query`; NDJSON collections via `ndjson-filter` |
| Presets / examples to learn from | example dropdowns, sample button | page `[[example]]` chips on the existing pages |

## In-model vs out-of-model (had it been built)

In-model (and therefore already delivered by the blocks above): serde_json parsing, Mongo-style
filter compilation, projection, sort/skip/limit, RFC 9535 selector evaluation, count, NDJSON/CSV
export — all pure Rust → WASM.

Out-of-model for this repo regardless of the dup: a JavaScript expression evaluator as a query
language (needs a JS engine in the block, which the pure-Rust model does not carry); an actual
embedded document store with indexes/persistence (unqlite's real value — the backlog row's own
"mirrors unqlite" framing is about the query surface, not storage); Monaco-grade editor affordances
(syntax highlighting, keystroke autocomplete) — the generated page uses plain textareas.

## Verification runs behind this conclusion

Run 2026-08-20 with the installed `gizza` CLI (not by reading descriptors):

```
gizza tool mongo-query data='[{"id":1,"name":"Ada","status":"active"},{"id":2,"name":"Bo","status":"disabled"},{"id":3,"name":"Cy","status":"active"}]' \
  query='{"status":"active"}' projection='name, id' format=json pretty=false
→ [{"name":"Ada","id":1},{"name":"Cy","id":3}]        # the backlog row's exact use case
```

```
gizza tool mongo-query data='[{"name":"Ada","age":36,"team":{"name":"core"},"tags":["math","code"]}, …]' \
  query='{"team.name":"core","age":{"$gt":30}}' projection='name, age' sort='age:desc' limit=5 format=json pretty=true
→ [ {"name":"Cy","age":41}, {"name":"Ada","age":36} ]  # dotted path + operator + sort + limit
```

```
gizza tool mongo-query data=$'{"id":1,"status":"active"}\n{"id":2,"status":"off"}' query='{"status":"active"}' format=count → 1
gizza tool mongo-query … projection='-status' format=csv → id,name / 1,Ada / 2,Bo
gizza tool jsonpath-query json='[{"id":1,"name":"Ada","status":"active"}, …]' path='$[?(@.status=="active")].name' wrap=true
→ {"count":1,"outputs":["[\"Ada\",\"Cy\"]"]}           # the JSONPath-selector half
```

## Recommendation

Skiplist (done). If any gap is felt later, it is an ENHANCEMENT to `blocks/mongo-query` — e.g.
accepting a JSONPath expression in the `projection`/`query` slot, or a JMESPath-style
multiselect-hash projection — not a second document-query block.
