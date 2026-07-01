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

### FAQ

<details>
<summary>Does the order of object keys matter?</summary>

No. `{"a":1,"b":2}` and `{"b":2,"a":1}` compare as equal — objects are matched key by key, not by position. Only the *report* follows the left document's key order (removed/changed first, then added keys).

</details>

<details>
<summary>How are arrays compared if an item was inserted in the middle?</summary>

Arrays are compared strictly index by index, so inserting one element near the front shifts everything after it and each shifted index is reported as "changed", plus one "added" at the end. It's a positional diff, not a move-detecting diff.

</details>

<details>
<summary>What does a type change (e.g. string to number) look like?</summary>

It's reported as a single "changed" entry at that path with the old and new values — e.g. `"1"` → `1`. The value's type is part of the comparison, so a string `"1"` never equals the number `1`.

</details>

<details>
<summary>Why do I get "the first (left) JSON is invalid"?</summary>

Both inputs must be well-formed JSON documents; the error tells you which side failed and where. Common culprits: trailing commas, single quotes, or unquoted keys — those are JavaScript-isms, not JSON.

</details>
