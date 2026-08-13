# mongo-query competitor analysis — 2026-08-13

## Scope

Tool: `mongo-query` — run a MongoDB-style query document (`{ age: { $gt: 21 } }`) against a pasted
JSON array of documents and return the matching documents, in the browser and the CLI.

Model fit: pure Rust/WASM over pasted JSON. No database connection, no server-side JavaScript
evaluation, and no aggregation runtime.

Related blocks checked before building (not duplicates): `mongodb-query-to-sql` *translates* a Mongo
filter into SQL and never touches data; `ndjson-filter` and `faceted-filter` filter records with
their own `path op value` DSL, not with Mongo query documents; `jq-query`, `jsonata-query`,
`jsonpath-query` and `json-mask` are different query languages entirely.

## Competitor scan

| Competitor | Table-stakes capabilities observed | UX/control patterns | Fit decision |
| --- | --- | --- | --- |
| sift (crcn/sift.js) — the reference "Mongo queries in JS" filter | Operator set `$in $nin $exists $gte $gt $lte $lt $eq $ne $mod $all $and $or $nor $not $size $type $regex $where $elemMatch`; `$regex` accepts a pattern plus `$options` flags; array fields match if any element matches; nested/dot paths. | Library, so its "UX" is the query document itself: a query object is pasted verbatim and applied to an array. | In model: every operator above except `$where`. `$where` needs a JS interpreter → out of model, rejected with an explicit message. |
| mingo (kofrasa/mingo) — in-memory MongoDB query language | Same query operators plus a cursor API: `find()` then `sort()`, `skip()`, `limit()`, and field projection; `Query#test()` for a single document; strict-mode MongoDB compatibility as the default. | Cursor chaining (filter → project → sort → skip → limit) is the expected pipeline shape. | In model: projection, sort, skip and limit become first-class params so a single call reproduces the whole cursor chain. Out of model: aggregation pipeline stages and `$jsonSchema`. |
| MongoDB Compass / `mongosh` `db.c.find()` filter bar | Filter, Project, Sort, Skip, Limit input boxes side by side; relaxed shell syntax (unquoted keys, single quotes, `/regex/i` literals, `ObjectId()`/`ISODate()` helpers); a result count next to the documents. | Separate small boxes for filter/project/sort/skip/limit; result count always visible; documents shown pretty-printed by default. | In model: same five boxes, relaxed shell syntax accepted for the query, pretty output default on, and a `count` output mode. Out of model: connecting to a live cluster, explain plans, index hints. |
| jsonmongoquery / jsfilter (JSON-array Mongo-query utilities) | Apply a Mongo query object to a plain JSON array; accept a single object as a one-document collection; update operators as a separate concern. | Plain "data in, query in, matches out" shape. | In model: JSON array, single object, and NDJSON input are all accepted. Out of model: update operators (`$set`, `$inc`) — this tool reads, it does not mutate. |
| MongoDB query-operator manual (comparison / logical / element / evaluation / array pages) | Documented semantics that libraries are measured against: `{field: null}` also matches missing fields, `$ne`/`$nin` match missing fields, `$not` matches missing fields, `$type` aliases, `$mod: [divisor, remainder]`, `$elemMatch` requires one array element to satisfy every condition, BSON sort order across types. | Every operator page ships a worked example pair (documents + query + result). | In model: those exact semantics are implemented and pinned by unit tests; the page ships worked examples in the same shape. Out of model: `$geoWithin`/`$near`, `$text`, `$expr`, `$jsonSchema`, `$bitsAllSet` — all rejected with a named error. |

## Table-stakes → where each landed

| Table-stake | Decision |
| --- | --- |
| Comparison `$eq $ne $gt $gte $lt $lte $in $nin` | Built |
| Logical `$and $or $nor $not` | Built |
| Element `$exists $type` (with BSON type aliases) | Built |
| Evaluation `$regex` + `$options`, `$mod` | Built |
| Array `$all $size $elemMatch` | Built |
| Dotted paths + implicit array traversal | Built |
| `{field: null}` matches missing; `$ne`/`$nin`/`$not` match missing | Built (unit-tested) |
| Projection (inclusion / exclusion, `_id` rule) | Built — `projection` param |
| Sort, skip, limit (the cursor chain) | Built — `sort`, `skip`, `limit` params |
| Relaxed shell syntax (unquoted keys, single quotes, comments, trailing commas, `/re/i`, `ObjectId()`/`ISODate()`/`NumberLong()`) | Built — the query parser accepts them; strict JSON is a subset |
| Result count visible | Built — `format = "count"`, and `csv`/`ndjson`/`json` output modes |
| Pretty-printed documents by default | Built — `pretty` defaults to true |
| `$where` (JavaScript predicate) | Out of model — named error |
| `$expr`, `$jsonSchema`, `$text`, `$geoWithin`/`$near`, `$bitsAllSet`, `$rand` | Out of model — named error |
| Aggregation pipeline (`$group`, `$lookup`, `$unwind`) | Out of model — listed on the page, not built |
| Update operators (`$set`, `$inc`, `$push`) | Out of model — this tool reads only |
| Live cluster connection, explain plans, index hints | Out of model — no network in the sandbox |

## Decisions implemented

- Five Compass-shaped inputs — `query`, `projection`, `sort`, `skip`, `limit` — so one call reproduces
  a whole `find().sort().skip().limit()` cursor chain.
- `projection` and `sort` accept both the Mongo document form (`{"name":1,"_id":0}`, `{"age":-1}`) and a
  short comma form (`name,-password` / `age:desc,name`), because the short form is far easier to type
  into a URL deep-link.
- Mixing inclusion and exclusion in one projection is an error, matching MongoDB, instead of silently
  picking one.
- `format` is an enum (`json`, `ndjson`, `csv`, `count`) and `pretty` a checkbox, so the page renders a
  `<select>` and a checkbox rather than free-text boxes.
- Unsupported and unknown `$operators` fail with the operator name and the reason, so a pasted
  production query never silently returns the wrong rows.
- Preset example chips cover a comparison filter, `$in` + projection + sort, `$elemMatch` on an array
  of sub-documents, and a `count`.

## Out-of-model / intentionally not built

- `$where` and `$expr` (require a JavaScript / aggregation-expression evaluator).
- Aggregation pipeline stages, `$jsonSchema`, `$text`, geospatial operators, bitwise operators.
- Update operators — the tool filters, it does not modify documents.
- Connecting to a live MongoDB deployment, explain plans, index hints, collation.
- BSON-specific types beyond the shell helpers that unwrap to JSON (`ObjectId()`/`ISODate()` become
  strings, `NumberLong()`/`NumberInt()`/`NumberDecimal()` become JSON numbers), because the input is
  plain JSON, not BSON.

## Verification focus

- Exact CLI output for a comparison filter, `$in`+projection+sort, `$elemMatch`, and `count`.
- Every `format` enum value exercised end to end.
- Non-default `pretty` (unchecked) state.
- Relaxed shell syntax (unquoted keys, single quotes, `/re/i` literal) and strict JSON both run.
- Missing-field semantics (`{field: null}`, `$ne`, `$not`) pinned by unit tests.
- Page deep-link (`?data=…&query=…`) prefills and auto-runs.

## Sources

- <https://github.com/crcn/sift.js>
- <https://github.com/kofrasa/mingo>
- <https://www.npmjs.com/package/sift>
- <https://github.com/cgkineo/jsonmongoquery>
- <https://www.mongodb.com/docs/manual/reference/operator/query/>
