## About this tool

This is a MongoDB query runner for plain JSON. Paste a JSON array of documents, paste the same
find filter you would hand to `db.collection.find(...)`, and see exactly which documents come
back — without a server, a connection string, or a scratch collection. It is built for testing a
filter before it ships, explaining a query to a teammate, and slicing a JSON export down to the
records you actually care about.

The query language is MongoDB's query-and-projection language. Supported operators:

- **Comparison** — `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte`, `$in`, `$nin`
- **Logical** — `$and`, `$or`, `$nor`, `$not`
- **Element** — `$exists`, `$type` (`"string"`, `"number"`, `"int"`, `"long"`, `"double"`,
  `"decimal"`, `"object"`, `"array"`, `"bool"`, `"null"`, or the BSON type code)
- **Evaluation** — `$regex` with `$options`, `$mod`
- **Array** — `$all`, `$size`, `$elemMatch`

MongoDB's matching rules are followed, not approximated: dotted paths such as `team.name` reach
into sub-documents, a predicate on an array field matches when **any** element matches, and a
missing field behaves the way the server behaves — `{"age": null}`, `$ne`, `$nin` and `$not` all
match documents that do not have the field at all.

### Worked example

Documents:

```json
[
  {"name":"Ada","age":36,"team":{"name":"core"},"tags":["math","code"]},
  {"name":"Bo","age":24,"team":{"name":"infra"},"tags":["ops"]},
  {"name":"Cy","age":41,"team":{"name":"core"},"tags":["code","ops"]}
]
```

Query `{"tags": {"$in": ["code"]}}`, projection `name, age`, sort `age:desc`:

```json
[
  {
    "name": "Cy",
    "age": 41
  },
  {
    "name": "Ada",
    "age": 36
  }
]
```

Both documents match because `tags` is an array and at least one element is `"code"`. Switching the
output format to **Count only** returns `2` instead.

### Projection, sort, skip and limit

The four boxes below the filter reproduce a full cursor chain — `find(query, projection).sort(...)
.skip(...).limit(...)`:

- **Projection** — `{"name":1,"age":1,"_id":0}` in Mongo form, or the short form `name, age` to
  keep and `-password` to drop. `_id` is kept by a keep-projection unless you set it to `0`,
  exactly as in MongoDB, and mixing kept and dropped fields (other than `_id`) is an error.
- **Sort** — `{"age":-1,"name":1}` or the short form `age:desc, name`. Values sort in MongoDB's
  cross-type order: missing/null first, then numbers, strings, objects, arrays, booleans.
- **Skip / limit** — page through a big result set. The **Count only** format always reports the
  full match count, ignoring skip and limit.

Output is a JSON array (indented by default), NDJSON for piping line by line, CSV for a
spreadsheet, or just the count.

### Pasting a query straight from the shell or Compass

Shell-flavoured syntax is accepted, so a filter copied out of `mongosh`, Compass or an application
log usually runs unmodified: unquoted keys (`{ age: { $gt: 21 } }`), single quotes, `//` and
`/* */` comments, trailing commas, `/pattern/flags` regex literals, and the `ObjectId()`,
`ISODate()`, `NumberLong()`, `NumberInt()` and `NumberDecimal()` helpers. Strict JSON is a subset,
so it works too. Because the input is JSON rather than BSON, `ObjectId("…")` and `ISODate("…")`
compare as the string inside them, and the numeric helpers compare as plain numbers.

### Limits and edge cases

- Documents input: **5,000,000 bytes** and **50,000 documents**. The query, projection and sort
  boxes accept **20,000 bytes** each, nested at most **64** levels deep.
- Everything runs locally in your browser via WebAssembly — no document ever leaves the page.
- Comparisons only relate values of the same JSON type, matching MongoDB: `{"age":{"$gt":"30"}}`
  (a string) will not match the number `36`.
- `$where` and `$expr` are rejected with an explicit message — they need a JavaScript /
  aggregation-expression evaluator. So are `$jsonSchema`, `$text`, the geospatial operators
  (`$near`, `$geoWithin`, `$geoIntersects`), the bitwise operators, `$rand` and `$sampleRate`.
- Aggregation pipeline stages (`$group`, `$lookup`, `$unwind`) and update operators (`$set`,
  `$inc`, `$push`) are out of scope: this tool reads documents, it never modifies them.
- `$all` with `$elemMatch` entries and projection operators such as `$slice` or `$` are not
  supported; an unknown operator always fails loudly rather than silently returning the wrong rows.
- Dates are compared as the strings they are in JSON, so ISO-8601 timestamps (`2026-08-13T12:00:00Z`)
  sort and range-compare correctly, while other date formats do not.

## FAQ

<details>
<summary>Does this connect to my MongoDB database?</summary>

No. It never opens a network connection. You paste the documents (a JSON array, a single object, or
NDJSON) and the query, and everything is evaluated locally in your browser by a WebAssembly module.
That is the point: you can test a filter against production-shaped sample documents without giving
anything access to your cluster. To get sample documents out of a real deployment, run
`mongoexport --jsonArray` or copy a result set out of Compass and paste it in.

</details>

<details>
<summary>Why does my query on an array field match more documents than I expected?</summary>

Because that is what MongoDB does. A predicate on an array field matches when **any** element
satisfies it, so `{"tags": "code"}` matches `{"tags": ["math", "code"]}`, and
`{"scores": {"$gt": 90}}` matches a document whose `scores` array holds a single value above 90 —
even if other elements are far below it. When you need one **element** to satisfy every condition
at once, use `$elemMatch`: `{"items": {"$elemMatch": {"sku": "a", "qty": {"$gt": 5}}}}` matches
only documents where a *single* item is both SKU `a` and quantity greater than 5.

</details>

<details>
<summary>Why does `$ne` return documents that don't have the field at all?</summary>

Again, matching the server. In MongoDB a missing field is treated as null for query purposes, so
`{"age": {"$ne": 24}}` matches documents with no `age` key, `{"age": null}` matches both an
explicit null and a missing key, and `{"age": {"$not": {"$gt": 30}}}` matches documents that lack
`age` too. If you want only documents that actually carry the field, add `$exists`:
`{"age": {"$exists": true, "$ne": 24}}`.

</details>

<details>
<summary>Can I paste a query with unquoted keys, single quotes or `ObjectId()`?</summary>

Yes. The query box reads relaxed shell syntax, so `{ status: 'active', _id: ObjectId("64b8f0…") }`
parses as written, along with `//` and `/* */` comments, trailing commas and `/^ada/i` regex
literals. Because the documents are plain JSON rather than BSON, `ObjectId("x")` and `ISODate("x")`
compare as the string `x`, and `NumberLong(5)` compares as the number `5` — which is normally what
you want when the documents came out of `mongoexport` or an API response.

</details>

<details>
<summary>Why is `$where` rejected?</summary>

`$where` runs arbitrary JavaScript against each document, which needs a JavaScript interpreter this
sandbox does not ship — and which MongoDB itself discourages for performance and security reasons.
`$expr` and `$jsonSchema` are rejected for the same class of reason: they need the
aggregation-expression evaluator and a JSON Schema validator. Rather than quietly ignoring them and
returning the wrong documents, the tool fails with the operator name and the reason. Most `$where`
filters can be rewritten with `$and`, `$or`, `$regex` and `$mod`.

</details>

<details>
<summary>How do I get a CSV of just a few fields?</summary>

Set the projection to the fields you want (short form is easiest: `name, age, team.name`) and
switch the output format to **CSV**. The columns are the first-seen union of the returned
documents' top-level keys, so every returned document contributes its keys in order, and a document
missing a column gets an empty cell. Nested objects and arrays are written into the cell as compact
JSON — flatten them with a dotted projection first if you want them in their own columns.

</details>
