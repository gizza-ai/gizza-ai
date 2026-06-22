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
