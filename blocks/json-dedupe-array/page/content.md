## What this tool does

**JSON Array Deduplicator** removes repeated **elements** from a JSON array. Paste an array
straight out of an API response, an export, or a merge of two datasets, and get back the same
array with the duplicates gone — original order intact, nothing uploaded.

By default two elements are duplicates when they are **structurally equal**: same values all
the way down through nested objects and arrays. Object **key order is ignored** when comparing
but preserved in the output, so `{"a":1,"b":2}` and `{"b":2,"a":1}` collapse into one while the
survivor keeps the shape you pasted. When "same record" means something narrower, name the
fields to compare — `id`, or `user.email,country` — and everything else is ignored.

You choose whether the **first or last** occurrence survives, whether matching **ignores letter
case**, and whether the result is the de-duplicated array, **only the duplicates that were
removed**, or a **report** of counts and duplicate groups. If the array is wrapped inside an
object (`{"data":{"items":[…]}}`), point **Path to the array** at it and the wrapper comes back
untouched.

## Worked example

**Input**

```json
[
  { "id": 1, "email": "ada@x.com" },
  { "id": 2, "email": "bo@x.com" },
  { "id": 1, "email": "ada@x.com" }
]
```

With **Compare only these fields** left blank (whole-element comparison) and **indent = 2**, the
output is:

```json
[
  {
    "id": 1,
    "email": "ada@x.com"
  },
  {
    "id": 2,
    "email": "bo@x.com"
  }
]
```

Switch **Show** to *Counts + duplicate groups* on the same input and you get the audit trail
instead — which positions collided, and which one was kept:

```json
{
  "total": 3,
  "unique": 2,
  "removed": 1,
  "duplicate_groups": [
    {
      "count": 2,
      "indexes": [0, 2],
      "kept_index": 0,
      "value": { "id": 1, "email": "ada@x.com" }
    }
  ]
}
```

## How to use it

1. Paste a **JSON array** — of objects, strings, numbers, or nested values.
2. Leave **Compare only these fields** blank for whole-element matching, or list fields
   comma-separated (`id`, `user.email,country`) to match on just those.
3. If the array sits inside a wrapper object, set **Path to the array** to its dot-path, e.g.
   `data.items`.
4. Pick the **occurrence to keep**, whether to **ignore letter case**, what to **show**, and the
   **indent** (`0` minifies).
5. Copy the result. Everything runs locally in your browser — the JSON is never uploaded.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions. Keep the blank line inside each. -->

<details>
<summary>How are two elements decided to be duplicates?</summary>

With **Compare only these fields** blank, elements are compared **structurally and deeply**:
every nested value must match. Object key **order** doesn't matter (`{"a":1,"b":2}` equals
`{"b":2,"a":1}`), but array **element order does** — `[1,2]` and `[2,1]` are different values.
Numbers compare by value, so `2` and `2.0` are duplicates, while the string `"2"` is not.
Fill the field in and only those fields are compared; two records with the same `id` but
different names are then duplicates.

</details>

<details>
<summary>Which copy is kept, and does the order change?</summary>

The **original order is always preserved** — this tool never sorts. With *Keep the first
occurrence* (the default) each duplicate group survives at its earliest position; with *Keep the
last occurrence* the survivor is the final copy, sitting at that final position. Keeping the
last one is the usual choice when later rows are fresher, e.g. an append-only log where the
newest record wins.

</details>

<details>
<summary>Can I compare nested fields, or an array element?</summary>

Yes — use **dot-notation**. `user.email` compares the `email` inside each row's `user` object,
and `tags.0` compares the first element of each row's `tags` array. List several paths
comma-separated (`user.email,country`) and elements must match on **all** of them to count as
duplicates. A path segment that doesn't exist for a row is treated as *absent*, which never
matches an explicit `null` — but all rows missing the same field do match each other.

</details>

<details>
<summary>How do I see what was removed instead of what survived?</summary>

Switch **Show**. *Only the removed duplicates* returns an array of exactly the elements that
were dropped, in their original order — handy for reviewing before you commit to the cleanup.
*Counts + duplicate groups* returns a JSON summary with `total`, `unique` and `removed` counts
plus, for each repeated value, its `count`, the 0-based `indexes` where it appeared, the
`kept_index`, and the surviving element. Both modes leave your input untouched.

</details>

<details>
<summary>My array is inside an object — do I have to extract it first?</summary>

No. Set **Path to the array** to the dot-path of the array, for example `data.items` for
`{"ok":true,"data":{"items":[…]}}`. The array is de-duplicated in place and the whole document
comes back with the wrapper intact. A numeric segment indexes an array, so `results.0.rows`
works too. If the path points at something that isn't an array, the error names what it found.

</details>

<details>
<summary>Is this the same as de-duplicating JSON Lines / NDJSON?</summary>

No — this tool takes one **JSON array**. If your data is NDJSON/JSONL (one JSON value per
*line*, with no enclosing brackets or commas), use a JSONL de-duplicator instead: pasted here,
the whole file would be either invalid JSON or a single element. Removing duplicate **keys**
inside one object is a third, separate job — that belongs to a JSON repair/format tool.

</details>

## Limits & edge cases

- The input must be **valid JSON**, and the target must be an **array** — a bare object is
  rejected with an error that points you at **Path to the array**.
- Up to **200,000 elements**; larger arrays are rejected with a named error rather than
  freezing the tab. Very large inputs are still bounded by your browser's memory.
- **Indent** is clamped to `0`–`8` spaces; `0` minifies to a single line.
- **Ignore letter case** folds case in string values **and in field names**, so `{"ID":1}` and
  `{"id":1}` match while it is on. Leave it off for case-exact data such as IDs and hashes.
- An **absent** field and an explicit `null` are different: `{}` and `{"id":null}` are not
  duplicates when comparing by `id`. All rows missing that field do group together.
- Elements are compared, never rewritten: key order, spacing choices inside each surviving
  element, and every non-compared field come back exactly as pasted.
- Everything runs in-browser via WebAssembly — the JSON you paste is never uploaded.
