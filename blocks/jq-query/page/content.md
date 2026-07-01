## Run jq on JSON in your browser

Paste a JSON document, type a **jq** filter, and see the transformed output
instantly. Everything runs locally in your browser with a pure-Rust jq engine
(jaq) — your data is never uploaded to a server.

### Examples

- `.` — pretty-print / echo the whole document.
- `.users | map(.name)` — pull one field from every element.
- `.items | map(.price) | add` — sum a field.
- `.[] | select(.active)` — keep only matching elements.
- `group_by(.kind) | map({kind: .[0].kind, n: length})` — group and count.

### Notes

- A jq filter can produce **zero, one, or many** values; each output value is
  printed on its own line.
- The jq standard library (`map`, `select`, `group_by`, `sort_by`, `add`,
  `unique`, …) is available.
- Object keys are emitted in sorted order. Tick **Pretty-print** for indented
  output.

## FAQ

<details>
<summary>Is this the real jq, and does it behave identically?</summary>

The engine is **jaq**, a pure-Rust implementation of the jq language, loaded with
the jq standard library (`map`, `select`, `group_by`, `sort_by`, `add`, `unique`,
and friends). Filters that work in jq almost always work here. One visible
difference: jaq emits object keys in sorted order rather than insertion order.

</details>

<details>
<summary>Why did my filter print several lines instead of one result?</summary>

A jq filter produces a *stream* of values — zero, one, or many — and each value is
shown on its own line. `.[]` over a three-element array yields three outputs. To
collapse a stream into a single value, wrap it in `[...]` (e.g. `[.[] | .name]`)
or use `map(...)`.

</details>

<details>
<summary>What kinds of errors will the tool report?</summary>

Four distinct cases: invalid JSON input (with the parser's position message), a jq
parse error (e.g. a trailing `|`), a compile error (an unknown function name), and
runtime errors such as adding a number to a string. An empty filter is also
rejected, so a blank query never silently returns nothing.

</details>

<details>
<summary>Does my JSON leave the browser when I run a query?</summary>

No. jaq is compiled to WebAssembly and evaluates the filter entirely in your tab —
there is no server-side jq process and nothing is transmitted or logged.

</details>
