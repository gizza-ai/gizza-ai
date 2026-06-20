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
