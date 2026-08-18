## About this tool

Two JSON arrays, four questions: what is in both, what is in either, what is in A but not
B, and what is on exactly one side. This tool answers all four — union, intersection,
difference (A − B) and symmetric difference — over arrays you paste in, and reports the
counts alongside the result so you can sanity-check the answer at a glance.

Elements are compared as JSON **values**, not as text. Object key order never matters, and
`1`, `1.0` and `1e0` are the same number, so two exports that serialise differently still
line up. When the records differ in ways you do not care about — a changed `name`, a fresh
`updated_at` — set **Match on field** to `id` (or a dot-path like `meta.sku`) and elements
are paired on that field only, the way lodash's `differenceBy` family works.

It suits the everyday list jobs: finding the user ids present in an export but missing from
the database, deduplicating a merged tag list, checking which SKUs two feeds share, or
diffing two API result sets by primary key. Input can be a JSON array or NDJSON (one JSON
value per line), which is what most exports paste as. Everything runs locally in your
browser through WebAssembly — neither array is uploaded anywhere.

### Worked example

**Array A**

```json
[{"id":1,"name":"Ada"},{"id":2,"name":"Bo"},{"id":3,"name":"Cy"}]
```

**Array B**

```json
[{"id":2,"name":"CHANGED"}]
```

With **Operation** set to *Difference (A − B)* and **Match on field** set to `id`, the
result is:

```json
{
  "operation": "difference",
  "matched_by": "id",
  "counts": {
    "a": 3,
    "b": 1,
    "a_unique": 3,
    "b_unique": 1,
    "only_in_a": 2,
    "only_in_b": 0,
    "in_both": 1,
    "union": 3,
    "result": 2
  },
  "result": [
    {"id": 1, "name": "Ada"},
    {"id": 3, "name": "Cy"}
  ]
}
```

Ada and Cy are missing from B; Bo matched on `id` even though every other field changed.
The `counts` block always describes the two **sets** (distinct match keys), so it stays
meaningful whether or not repeats are collapsed in the result. Switch **Output** to *Result
array only* to get the bare `[{"id":1,…},{"id":3,…}]` you can paste straight into the next
step, and set **Indent** to `0` to minify it onto one line.

### Limits and edge cases

- Each array is capped at **50,000 elements**; anything larger is rejected with a clear message.
- Input must be a top-level JSON array, or NDJSON with **two or more** non-blank lines. A lone `{"id":1}` is treated as a mis-paste rather than a one-record export.
- **Difference is directional**: it is A − B. Swap the two boxes for B − A, or use symmetric difference to get both sides at once.
- When **Match on field** is set, *every* element of both arrays must carry that field — a missing one is an error naming the array and index, not a silent skip. Leave it blank to compare whole values.
- Result elements are always taken from **array A** wherever an operation could draw from either side (union and symmetric difference append B's leftovers afterwards), and the first occurrence of a repeated key wins.
- With **Collapse repeats** on (the default) the result is a true set. Turn it off to keep every matching occurrence, including duplicates within one array.
- Numbers are compared through 64-bit floats, so integers beyond 2^53 (about 9·10^15) can compare equal despite differing in their last digits. Match on a string key if you carry ids that large.
- **Ignore string case** folds case inside the matched value only; it does not change the elements that come back, which are returned exactly as pasted.
- Element order *inside* a compared value is significant: `[1,2]` and `[2,1]` are different elements, even though the arrays A and B themselves are treated as unordered sets.
- **Indent** accepts 0–8 spaces; larger values are clamped to 8.

## FAQ

<details>
<summary>When are two elements "the same"?</summary>

By default, when their whole JSON values are equal after canonicalisation: object keys are
sorted, so `{"a":1,"b":2}` and `{"b":2,"a":1}` match, and numbers are normalised, so `1`
and `1.0` match. Whitespace and key order in your paste are irrelevant. If you set **Match
on field**, only that field's value is compared and everything else is ignored.

</details>

<details>
<summary>How do I compare records that have changed fields, like lodash differenceBy?</summary>

Put the identifying field in **Match on field** — `id`, `email`, `sku`. Nested fields use a
dot-path: `meta.sku`, and array positions work too (`tags.0`). Elements are then paired on
that value alone, so a record whose `name` or `updated_at` changed still counts as present
on both sides.

</details>

<details>
<summary>Which array do the returned elements come from?</summary>

Array A, wherever the operation could draw from either side. Intersection returns A's
version of each shared element, union returns all of A and then only the elements of B that
A did not already have, and symmetric difference returns A-only elements first, then B-only
ones. That means the fields you see are A's fields — useful when B is a trimmed id list.

</details>

<details>
<summary>Can I paste NDJSON or a log-style export?</summary>

Yes. If the text does not start with `[`, it is read as NDJSON: one JSON value per line,
blank lines skipped, two or more lines required. Mixing the two formats between the boxes
is fine — array A can be NDJSON while array B is a plain array.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The set logic is compiled to WebAssembly and runs inside your browser tab, so both
arrays stay on your machine. Nothing is sent to a server, logged, or stored — you can load
the page, disconnect, and it still works.

</details>
