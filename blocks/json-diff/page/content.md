## About this tool

**JSON Diff** compares two JSON documents and reports exactly what differs
between them — no more eyeballing two blobs side by side.

Paste your **first (left / old)** document and your **second (right / new)**
document, and the tool walks both structures recursively:

- **Objects** are compared **key by key**.
- **Arrays** are compared **index by index**.
- Every difference is reported with its **JSON path** (e.g. `$.user.name` or
  `$.items[2]`) and classified as **added**, **removed** or **changed**.

The output is a JSON report:

```json
{
  "equal": false,
  "added": 1,
  "removed": 0,
  "changed": 1,
  "changes": [
    { "path": "$.age", "kind": "changed", "old": 30, "new": 31 },
    { "path": "$.city", "kind": "added", "new": "NYC" }
  ]
}
```

Set **Indent** for the output (or 0 to minify). Everything runs **locally in
your browser** via WebAssembly — your data is never uploaded.

### Handy for

- Reviewing changes between two API responses or config versions.
- Spotting unexpected additions or removals in a JSON file.
- Generating a machine-readable change report for tests or audits.
