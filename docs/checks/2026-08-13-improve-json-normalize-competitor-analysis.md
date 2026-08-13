# json-normalize — competitor analysis (2026-08-13)

Scan run **before** implementation, per the create-next-tool recipe. One web search
("normalizr normalize nested JSON into entities keyed by id schema") plus targeted reads of the
top results. Everything below is paraphrased from public documentation — no competitor copy,
branding, or trademarked wording is reproduced in the block, its page, or its tests.

## Tool function

Take a deeply nested JSON document plus a small entity schema, pull every nested entity out into
per-type lookup tables keyed by id, and replace each nested occurrence with just its id — the
"normalized store" shape front-end and ETL code wants (one copy of each record, references by id).

## Competitors reviewed

### 1. normalizr (paularmstrong/normalizr) — the reference implementation

The de-facto library for this operation. `normalize(data, schema)` returns
`{ entities: { <entityKey>: { <id>: entity } }, result: <ids> }`; `result` is a single id for an
object input and an array of ids for an array input.

Schema pieces:

- `schema.Entity(key, definition, options)` — `key` is the table name in `entities`, `definition`
  maps a field name onto another schema (nested entity); undefined fields are copied through.
- `schema.Array` (also written as a one-element array literal) — a list of entities.
- `schema.Object` — a plain wrapper object that itself is not an entity.
- `schema.Union` / `schema.Values` — polymorphic fields, keyed by a type discriminator.

Entity options:

- `idAttribute` — string field name (default `id`) **or a function** `(value, parent, key)`.
- `mergeStrategy` — what happens when the same id is seen twice; default is a shallow merge with
  the later occurrence winning per key.
- `processStrategy` — arbitrary JS callback that pre-processes each entity.
- `fallbackStrategy` — only used on the reverse (`denormalize`) path.

Worked example from its docs (paraphrased shape): a tweet whose id lives in `id_str` and whose
`user` is an entity with the same custom id field normalizes to
`entities.tweets["123"] = { id_str: "123", user: "456" }`,
`entities.users["456"] = { id_str: "456", name: … }`, `result = "123"`.

### 2. json-api-normalizer (yury-dymov)

Schema-free, but only because it hard-codes one input shape: a JSON:API payload
(`data` / `included` / `relationships`). Output is a map of `type -> { id -> record }`, i.e. the
same normalized-store shape without a user-supplied schema. Options: `camelizeKeys` (default on,
snake_case → camelCase for keys and type names), `camelizeTypeValues`, `endpoint` +
`filterEndpoint` (record which request produced which ids, for Redux caching).

Takeaway: its differentiator is *inferring* the schema from a spec-defined envelope; for arbitrary
JSON there is nothing to infer, so a schema param is unavoidable for a general tool.

### 3. normalizr forks / small clones (normalizr-plus, simple-normalizr, and the various
   `restlessdesign` / `mulesoft-labs` forks surfaced by the search)

All restate the same core contract — nested JSON + schema in, `{ entities, result }` out — with
smaller option surfaces. None adds a capability normalizr lacks. Confirms
`{ entities, result }` + `idAttribute` + a same-id merge rule are the table stakes, not
implementation details of one library.

### 4. Online JSON flatteners (convertjson.com JSON flattener, onlinejsontools/onlinetools
   flatten-json)

Reachability: the convertjson page 404'd and the onlinejsontools page 301-redirects off-host, so
these were only skimmed at the level the search results describe. Their operation is a **different
one** — collapsing nesting into `a.b.c` dotted keys, with controls for the separator, array-index
handling, and indentation. That is path-flattening, not entity normalization: no id table, no
deduplication, no reference replacement. gizza already covers that direction
(`csv-json-convert` flattens nested keys for CSV output; `json-transform-rules` does dotted-path
reshaping). Their UX patterns worth copying are generic and already in the gizza page model:
paste-in textarea, sample/preset data buttons, pretty/indent control, copy + download output.

## Table stakes → in-model / out-of-model decisions

| Capability (competitor) | Decision | Where it lands |
| --- | --- | --- |
| `{ entities, result }` output shape | **in** | default `output = normalized`; `entities` / `result` also selectable |
| Entity schema: entity key → nested field → entity | **in** | `schema` param, JSON object form `{"articles":{"author":"users"}}` |
| Array-valued relations (`[comments]`) | **in** | `["comments"]` in the JSON form, `[comments]` in the shorthand line form |
| Custom `idAttribute` per entity | **in** | `id_field`: one name, a comma-separated fallback list, or a per-entity JSON object |
| Default id field `id` | **in** | `id_field` default `id` |
| Same-id merge (later wins per key) | **in** | `on_conflict = merge` (default), plus `replace` / `keep_first` / `error` |
| Root schema is an entity or an array of entities | **in** | auto: array document → array `result`, object document → scalar `result` |
| Entity value already reduced to a bare id | **in** | a string/number where an entity is expected passes through as a reference (re-running is a no-op) |
| Undefined fields copied through untouched | **in** | traversal only rewrites fields named in the schema |
| Pretty / indent control (flatteners + every JSON tool) | **in** | `pretty` (default on) + `indent` slider 0–8 |
| Preset/sample buttons (flatteners, most online JSON tools) | **in** | four `[[example]]` chips on the page |
| Point at a nested payload (JSON:API `data`, `{"data":{"items":[…]}}`) | **in** | `path` param (dotted/indexed path, blank = whole document) |
| Entities missing their id field | **in**, and better than the reference lib (which warns and drops) | `on_missing_id = error \| index \| hash \| keep` |
| Debugging "why is my table empty" | **in** | `output = report` — per-type counts, conflicts merged, synthesized ids |
| `processStrategy` / `mergeStrategy` / function `idAttribute` (arbitrary JS callbacks) | **out** | needs a JS runtime; gizza blocks are sandboxed pure Rust/WASM with no eval. Field-level reshaping is already `json-transform-rules`; chain the two. |
| `schema.Union` / `schema.Values` (polymorphic type discriminators) | **out for v1** | needs a per-field discriminator mini-language; the common case (one field, one entity type) is covered. Documented as a limit. |
| `denormalize()` (put the tree back together) | **out for v1** | the inverse operation, a separate tool's worth of surface; documented as a limit. |
| `camelizeKeys` / JSON:API auto-schema | **out** | key-case rewriting is `change-case` / `json-transform-rules` territory; auto-schema only works for one spec-defined envelope and would not generalize. |
| Path flattening to `a.b.c` keys | **out (already covered)** | `csv-json-convert` (flatten for CSV) and `json-transform-rules` (dotted targets) already do it. |

Every table stake above is either in the descriptor or in this out-of-model list; none was dropped
silently.

## Duplicate check

`ls blocks/ | grep -iE 'json|flat|normal|entity|nest'` plus a read of each candidate's summary:

- `data-normalize` — scales **numeric CSV columns** (min-max / z-score / robust). Unrelated.
- `json-transform-rules` — declarative `target = selector` field mapping into one output object.
  No entity tables, no id keying, no cross-reference replacement.
- `json-structure-analyzer` — reports depth / key frequency / per-path types. Read-only stats.
- `jq-query`, `jsonata-query`, `jsonpath-query` — general query engines; extracting an entity
  store would be a hand-written program per schema, not a parameterized operation.
- `csv-json-convert` — CSV↔JSON with dotted-key flattening for the CSV direction.
- `json-to-sql-insert`, `json-merge`, `json-diff`, `json-mask`, `json-sort` — different operations.

Not a duplicate; built.

## What shipped

`schema` (JSON object **or** shorthand lines), `root`, `path`, `id_field`, `on_missing_id`
(`error`/`index`/`hash`/`keep`), `on_conflict` (`merge`/`replace`/`keep_first`/`error`), `output`
(`normalized`/`entities`/`result`/`report`), `pretty`, `indent`. Deterministic ordering
(`serde_json` `preserve_order` + first-seen insertion order in every table) so exact outputs are
testable from the CLI, the page, and the chat block.
