## About this tool

**Query String Codec** converts between URL query strings and structured JSON in both directions.
Use it to inspect a long URL's parameters, debug form-encoded API calls, or build a query string
from a JSON object without hand-escaping spaces, brackets, or unicode.

In **Query string → JSON** mode, the tool accepts a bare query string or one with a leading `?`.
It splits on `&` and `;`, percent-decodes keys and values, decodes `+` as a space by default, turns
repeated keys into arrays, and expands common bracket notation such as `tags[]`, `tags[0]`, and
`user[name]` into nested arrays and objects.

In **JSON → query string** mode, paste a JSON object and choose how arrays should be serialized:
`tags[]=a&tags[]=b`, `tags[0]=a&tags[1]=b`, `tags=a&tags=b`, or `tags=a,b`. You can also sort keys,
use `%20` instead of `+` for spaces, and add a leading `?`.

### Worked example

Input query string:

```
name=John+Doe&color=red&color=blue&user[age]=30
```

Output JSON:

```json
{
  "name": "John Doe",
  "color": [
    "red",
    "blue"
  ],
  "user": {
    "age": "30"
  }
}
```

Switch to **JSON → query string**, paste `{"tags":["a","b"]}`, and choose **repeat** to get
`tags=a&tags=b` or **indices** to get `tags[0]=a&tags[1]=b`.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions with a blank line inside each. -->

<details>
<summary>Does parsing a query string infer numbers and booleans?</summary>

No. Query strings carry text, so parsed values stay strings. `age=30` becomes `"30"`, not the number
`30`. When building from JSON, numbers and booleans are serialized to their normal text form.

</details>

<details>
<summary>Which array styles round-trip?</summary>

`brackets`, `indices`, and `repeat` round-trip through parse as arrays. `comma` is compact for APIs
that expect `tags=a,b`, but parse intentionally leaves it as the string `"a,b"` because commas are
valid data too.

</details>

<details>
<summary>What does “Use + for spaces” change?</summary>

With the option on (default), build mode writes spaces as `+` and parse mode decodes `+` back to a
space, matching HTML form encoding. Turn it off for stricter RFC 3986 style: build writes `%20`, and
parse keeps literal plus signs as `+`.

</details>

<details>
<summary>Is the query string uploaded or fetched?</summary>

No. The codec is pure WebAssembly running in your browser. It never fetches URLs; it only parses or
builds the text you paste.

</details>

## Limits & edge cases

- **Top-level build input must be a JSON object.** Arrays or scalars at the root are rejected so every
  value has a query parameter name.
- **Invalid percent escapes are lenient.** A malformed `%` sequence is left literal instead of aborting.
- **Nested arrays/objects are supported for bracket and index styles.** Comma style requires scalar
  array items.
- **Parse output is strings.** Use a downstream JSON tool if you need type inference.
- **Key ordering in build mode follows input JSON order** unless you enable **Sort keys when building**.
