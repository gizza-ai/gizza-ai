## About this tool

**JSON Key Sorter** parses your JSON, recursively reorders every object's keys, and re-serializes
the result — so two documents with the same data always produce the same text. That makes diffs
small and reviews easy: reordered keys stop showing up as spurious changes in version control.

Sorting is **recursive**: keys are reordered at every level of nesting, inside objects that live in
arrays too. By default only object keys move — array element order is left alone, because array
order is usually meaningful (a list of steps, a priority order). Turn on **Also sort array
elements** when the arrays are really unordered sets and you want them canonicalized as well.

The input is validated before anything is sorted, so invalid JSON is rejected with the parser's
exact line and column instead of producing garbled output.

### Worked example

Input:

```json
{ "b": 1, "a": { "z": 2, "y": 3 } }
```

With **Sort direction = Ascending** and **Indent = 2**, the output is:

```json
{
  "a": {
    "y": 3,
    "z": 2
  },
  "b": 1
}
```

The top-level keys become `a`, `b` and the nested `z`, `y` become `y`, `z`. Set **Indent** to `0`
to collapse the whole thing onto one compact line: `{"a":{"y":3,"z":2},"b":1}`.

### Options

- **Sort direction** — Ascending (A→Z, default) or Descending (Z→A).
- **Also sort array elements** — off by default; when on, array items are ordered by their
  serialized value so the ordering is total across mixed types.
- **Case-insensitive** — off by default (codepoint order, where uppercase sorts before lowercase);
  on groups `Name` next to `name`. Keys that differ only in case keep a stable case-sensitive
  tie-break so the result is deterministic.
- **Indent spaces** — 1–8 spaces per level (default 2), or `0` to minify.

## FAQ

<details>
<summary>Does it sort keys inside nested objects and arrays?</summary>

Yes. Sorting is recursive — every object's keys are reordered at every level, including objects
nested inside arrays. Array element *order* is left unchanged unless you enable **Also sort array
elements**.

</details>

<details>
<summary>Why are my array items not reordered?</summary>

By default only object keys are sorted; array order is preserved because it's usually meaningful
(a sequence of steps, a ranked list). Enable **Also sort array elements** to reorder array items
too — they're sorted by their serialized value.

</details>

<details>
<summary>How does case-insensitive sorting work?</summary>

With **Case-insensitive** off (the default), keys sort by Unicode codepoint, so all uppercase
letters come before lowercase ones (`Name` before `apple`). Turn it on to compare ignoring case, so
`Name` sorts next to `name`. Keys differing only in case still get a stable tie-break, so the output
is always deterministic.

</details>

<details>
<summary>What happens if my JSON is invalid?</summary>

The document is parsed and validated first. If it isn't valid JSON, you get an `invalid JSON` error
with the line and column of the problem, and nothing is sorted. Trailing commas and unquoted keys
are rejected — this tool sorts valid JSON, it doesn't repair broken JSON.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The sort runs entirely in your browser via WebAssembly — the JSON never leaves your device.

</details>
