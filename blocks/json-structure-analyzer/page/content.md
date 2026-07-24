## About this tool

**JSON structure analyzer** parses a JSON document and reports its *shape* — without
transforming or reformatting the data. Paste any JSON value (object, array, or scalar)
and you get:

- **Max nesting depth** — the deepest level of objects/arrays.
- **Node counts** by type: objects, arrays, strings, numbers, booleans, and nulls,
  plus total values, total keys, unique keys, and empty values.
- **Key frequency** — how often each key name occurs across the whole document
  (recurring names counted each time), ranked most-common first.
- **Per-path type distribution** — each path (with array indices collapsed to `[]` so
  every element of an array shares one path) and the JSON types seen there. This makes
  mixed-type fields obvious.
- **Array stats** — count, min / max / average length, and total elements.
- **Byte size** — raw vs. minified, plus the compression potential of stripping
  whitespace.
- **Quality warnings** — deep nesting (more than 5 levels), recurring keys, and empty
  values.

### Worked example

For `{"users":[{"id":1,"name":"Ada"},{"id":2,"name":"Bo"}]}` the report shows root type
**object**, max depth **3** (`$` → `$.users` → `$.users[]` → `$.users[].name`), key
frequency `id: 2, name: 2, users: 1`, and the path `$.users[]` with **2** objects. The
`name` field lands at `$.users[].name` as a **string** seen twice.

### Options

- **Output format** — *JSON report* (structured, easy to diff or feed to another tool)
  or *Plain text* (a human-readable summary of the same data).
- **Max keys** and **max paths** — cap the key-frequency and per-path lists (set `0` to
  list everything). A truncation flag tells you when a list was cut.

### Privacy

Everything runs **in your browser** via WebAssembly — your JSON is never uploaded. The
tool is also available from the [gizza CLI](/) and in chat, which return the report as
structured JSON.

## FAQ

<details>
<summary>How is nesting depth counted?</summary>

The root value is depth **0**, and each step into an object or array adds one. So
`{"a":1}` is depth **1**, `{"a":{"b":1}}` is depth **2**, and a scalar like `42` on its
own is depth **0**. Depth over **5** levels raises a "deep nesting" warning, since deeply
nested data is harder to read and query.

</details>

<details>
<summary>What does the per-path type distribution show, and why collapse array indices?</summary>

Each path is written with `$` for the root, `.key` to descend an object, and `[]` to
descend an array — so all elements of an array share **one** path (`$.users[]` rather
than `$.users[0]`, `$.users[1]`, …). That collapse is what makes a mixed-type field
jump out: if `$.items[].price` reports `["number", "string"]`, some prices are strings.
Paths are ranked by how many nodes were seen at each one.

</details>

<details>
<summary>What counts as an "empty value"?</summary>

An empty value is a `null`, an empty string (`""`), or an empty object/array (`{}` /
`[]`). They're counted together in the `empty_values` total and surface a warning, which
is handy for spotting placeholder fields or optional data that never got filled in.

</details>

<details>
<summary>Can it handle very large JSON, and what about invalid JSON?</summary>

Large documents are fine — parsing and the single walk are linear, and the whole thing
runs locally so there's no upload size limit beyond your browser's memory. If the input
isn't valid JSON, you get a clear parse error (with the position) instead of a partial
report, so you know exactly what to fix.

</details>
