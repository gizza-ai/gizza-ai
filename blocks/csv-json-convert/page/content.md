## CSV ⇄ JSON converter

Paste CSV or JSON and convert it to the other format — both directions, right in
your browser. Nothing is uploaded: the conversion runs locally in WebAssembly, so
your data never leaves your device.

### How it works

- **Direction** — leave it on **auto** and the tool detects the input (text that
  starts with `[` or `{` is treated as JSON and converted to CSV; anything else is
  parsed as CSV and converted to JSON). Force a direction if you need to.
- **CSV → JSON** — with *first row is headers* on (the default) you get an array
  of objects keyed by the header names; turn it off to get an array of arrays.
  Numbers, `true`/`false`, and empty cells are inferred as JSON numbers, booleans,
  and `null`. Values that wouldn't round-trip (leading zeros like `007`, `+1`)
  stay strings, so zip codes and phone numbers are preserved.
- **JSON → CSV** — pass an array of objects (the column order is the union of all
  keys, in first-seen order) or an array of arrays. Nested objects and arrays are
  written as compact JSON in the cell so nothing is dropped — or turn on
  **flatten** to expand them into dot-notation columns instead (`{"addr":{"city":
  "NYC"}}` becomes an `addr.city` column).
- **Delimiter** — use any single character, or the words `tab`, `comma`,
  `semicolon`, or `pipe`. Quoted fields and embedded commas/newlines are handled
  per RFC 4180.

### Examples

`name,age` / `Alice,30` → `[{"name":"Alice","age":30}]`

`[{"a":1,"b":2},{"a":3,"c":4}]` → a CSV with columns `a,b,c` and blank cells where
a row is missing a key.

### FAQ

**Is my data uploaded anywhere?** No. The converter is compiled to WebAssembly
and runs entirely in your browser tab.

**Does it support TSV?** Yes — set the delimiter to `tab`.

**Why did my number stay a string?** Values with leading zeros or a leading `+`
are kept as text so they survive the round trip unchanged.
