## Query JSON with JSONPath in your browser

Paste a JSON document, type a **JSONPath** expression, and see the matched nodes
instantly. Everything runs locally in your browser with a pure-Rust, RFC 9535
engine — your data is never uploaded to a server.

### Examples

- `$` — select the whole document (root).
- `$.store.book[*].author` — the author of every book.
- `$.store.book[0]` — the first book.
- `$.store.book[1:3]` — a slice of the books array.
- `$..price` — every `price` anywhere in the document (recursive descent).
- `$.store.book[?(@.price < 10)].title` — titles of books cheaper than 10
  (filter selector).

### JSONPath syntax

- `$` is the root; `.name` or `['name']` selects a child.
- `[*]` is a wildcard over array elements or object values.
- `[n]` indexes an array (negative indices count from the end); `[start:end:step]`
  is a slice.
- `..` is recursive descent — search every level below.
- `[?(expression)]` is a filter; `@` refers to the current element, e.g.
  `[?(@.active && @.age >= 18)]`.

### Notes

- A JSONPath query can select **zero, one, or many** nodes; each matched value is
  printed on its own line.
- Tick **Wrap matches in a JSON array** to get a single result list `[ ... ]`
  instead of one value per line.
- Tick **Pretty-print** for indented output.
- This follows the **RFC 9535** standard for JSONPath.

## FAQ

<details>
<summary>Why did my query return nothing — is that an error?</summary>

No. A JSONPath query legitimately selects zero, one, or many nodes, so "no
match" simply produces empty output (or `[]` when "Wrap matches in a JSON
array" is ticked). You only get an error for an empty/invalid expression or
JSON that doesn't parse.

</details>

<details>
<summary>What does the "Wrap matches in a JSON array" option change?</summary>

By default each matched value is printed on its own line — easy to eyeball,
but the output as a whole isn't valid JSON when there are multiple matches.
Wrapping returns one JSON array containing all matches, which you can feed
straight into another tool or script.

</details>

<details>
<summary>Which JSONPath dialect does this follow?</summary>

The IETF standard, **RFC 9535**. The familiar constructs all work — `$..price`
recursive descent, `[*]` wildcards, `[start:end:step]` slices, negative
indices, and `[?(@.price < 10)]` filters — but non-standard extensions from
older "Goessner-style" implementations may be rejected with a parse error.

</details>

<details>
<summary>Can I filter on more than one condition?</summary>

Yes — filter expressions support logical operators, so
`$[?(@.active && @.age >= 18)]` selects elements matching both conditions,
with `@` referring to the element currently being tested.

</details>
