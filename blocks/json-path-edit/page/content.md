## About this tool

**JSON Path Editor** reads and edits a single value inside a JSON document by its
**path** — no need to hand-edit brackets and commas. Pick an operation, point at
a path, and get back either the value there (`get`) or the whole modified
document (`set` / `delete`). Everything runs locally in your browser: your JSON
is never uploaded.

Paths use the familiar **lodash / `dot-object`** style (not RFC 9535 JSONPath):

- **Dot segments** — `store.book.title`
- **Array indices** — `store.book[0]` or the equivalent dotted form `store.book.0`
- **Quoted keys** — for a key that itself contains a dot, bracket, or space, quote
  it inside brackets: `["first name"]` or `["a.b"].c`
- An optional leading `$` is accepted and ignored (`$.store.book[0]`), and an
  **empty path** selects the whole document.

### Worked example

Given this document:

```json
{"store":{"book":[{"title":"A","price":5},{"title":"B","price":12}]}}
```

- **get** `store.book[1].title` → `"B"`
- **set** `store.book[0].price` to `9` → `{"store":{"book":[{"title":"A","price":9},{"title":"B","price":12}]}}`
- **delete** `store.book[0]` → `{"store":{"book":[{"title":"B","price":12}]}}` (the array shifts down)

### Setting values

For `set`, the **value** field is parsed as JSON: `42` becomes a number, `true` a
boolean, `null` a null, and `{"k":1}` an object. If it isn't valid JSON it's
stored as a plain string, so a bare `hello` becomes `"hello"`. To force text that
looks like JSON to stay a string, wrap it in quotes — e.g. `"true"` sets the
string `true`, not the boolean.

`set` **creates missing intermediates**: setting `user.address.city` on `{}`
builds the nested objects for you, and a numeric segment such as `list[2]` creates
an array (padding earlier slots with `null`).

### Limits & edge cases

- **`get`** errors if the key or array index doesn't exist (rather than returning
  nothing), so a typo is obvious. **`delete`** likewise errors if there's nothing
  at the path.
- **`set` won't clobber a scalar**: trying to set `a.b` when `a` is already a
  string or number errors instead of silently overwriting your data.
- Array growth on `set` is capped at 100,000 elements, so a typo like
  `a[999999999]` errors instead of allocating a huge array.
- Object key order is preserved on `set`/`delete`.

## FAQ

<details>
<summary>What path syntax does this use — is it JSONPath?</summary>

No. It uses **lodash / `dot-object`** notation: dot segments (`a.b.c`), array
indices as brackets or dotted digits (`a[0]` is the same as `a.0`), and quoted
keys for keys containing a dot, bracket, or space (`["a.b"].c`). This targets one
value at a time. For RFC 9535 JSONPath queries with wildcards, slices, recursive
descent, and filters, use a dedicated JSONPath query tool instead.

</details>

<details>
<summary>How do I set a value that looks like JSON but should stay a string?</summary>

Wrap it in quotes. The **value** field is parsed as JSON first, so `true`, `42`,
and `null` become a boolean, number, and null. Typing `"true"` (with the quotes)
stores the string `true`. A bare word that isn't valid JSON,
like `hello`, is already treated as the string `"hello"`.

</details>

<details>
<summary>Will `set` create missing objects and arrays along the path?</summary>

Yes. Setting `user.address.city` on `{}` creates the `user` and `address` objects
automatically. A numeric segment creates an array — `items[2]` builds a
three-element array padded with `null`. What it will **not** do is overwrite an
existing scalar: if `a` is already `5`, setting `a.b` errors instead of destroying
the `5`.

</details>

<details>
<summary>What happens if the path doesn't exist?</summary>

For **get** and **delete**, a missing key or an out-of-range array index is an
error with a message that says which segment failed — so a typo surfaces
immediately rather than silently doing nothing. For **set**, a missing path is the
normal case: it's created.

</details>

<details>
<summary>Is my JSON uploaded anywhere?</summary>

No. The whole tool runs as WebAssembly inside your browser tab. Your JSON document
never leaves your device, so it's safe to use with private or sensitive data.

</details>
