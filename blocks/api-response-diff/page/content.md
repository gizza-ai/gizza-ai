## Compare two API responses without the noise

Two calls to the same endpoint almost never return byte-identical JSON: a request id, a
`generatedAt` stamp or a freshly minted UUID changes on every hit, and a plain text diff
buries the one field you actually care about. This tool compares the two responses
**structurally** and lets you drop the volatile parts first, so what is left is the real
behavioural change.

It is built for the everyday API jobs: checking a staging deploy against production,
reviewing a version bump (`/v1` vs `/v2`), confirming a cache or pagination fix changed
nothing else, and turning "these look different" into a precise list of paths.

Everything runs locally in your browser through WebAssembly — the two responses are never
uploaded anywhere.

## Worked example

**First response (baseline)**

```json
{"requestId":"req-8f21","generatedAt":"2026-08-15T09:14:02Z","data":{"total":2,"status":"ok"}}
```

**Second response (candidate)**

```json
{"requestId":"req-c07d","generatedAt":"2026-08-16T11:02:47Z","data":{"total":3,"status":"ok"}}
```

With **Ignore fields** set to `requestId, generatedAt`, the result is:

```json
{
  "equal": false,
  "counts": {
    "added": 0,
    "removed": 0,
    "changed": 1,
    "type_changed": 0,
    "ignored": 2
  },
  "truncated": false,
  "notes": [],
  "ignored_paths": [
    "$.requestId",
    "$.generatedAt"
  ],
  "changes": [
    {
      "path": "$.data.total",
      "kind": "changed",
      "old": 2,
      "new": 3
    }
  ]
}
```

One real change (`$.data.total` went from 2 to 3), two fields deliberately skipped — and
`equal` tells you at a glance whether anything meaningful moved.

Switch **Output** to *Readable summary* for a one-line-per-change view
(`~ $.data.total: 2 -> 3`), or to *RFC 6902 JSON Patch* to get
`[{"op":"replace","path":"/data/total","value":3}]` you can feed to a patch library.

## Ways to say "ignore this"

* **By name** — `updatedAt` skips that key wherever it appears, at any depth.
* **By pattern** — `*_at` skips `created_at`, `updated_at`, `expires_at`; `$.data.items[*].token`
  skips one token field per element; `**` matches any number of path segments.
* **By exact path** — `data.token` (or `$.data.token`) only matches at the root position.
* **By shape** — the *Ignore timestamp-shaped values* and *Ignore UUID-shaped values*
  checkboxes drop a change when **both** sides look like a timestamp or a UUID, which is
  handy when the volatile field names are not known up front.

Skipped locations are counted and listed under `ignored_paths`, so a filter can never hide
a change silently.

## Arrays: index, key, or set

Collections are where JSON diffs usually go wrong. Three matching modes cover the common
cases:

* **By position (index)** — the default, and the only mode that can produce a JSON Patch.
* **By key field** — pairs objects by `id` (or any field you name), so a list that came back
  in a different order still diffs field-by-field. Paths read `$.items[id=a1].price`.
* **As an unordered set** — elements are matched as a multiset; anything left over is
  reported as added or removed. Use it when order genuinely carries no meaning.

If an array cannot be matched by key (elements are not objects, or the key repeats), the
comparison falls back to index matching and says so in `notes` rather than failing.

## Limits and edge cases

* Each response is capped at **4 MiB**; larger payloads are rejected with a clear message.
* At most **2000 changes** and 2000 ignored paths are listed; `truncated` becomes `true`
  while the counts stay exact.
* Arrays longer than **2000 elements** fall back to index matching in key/set mode (noted in
  `notes`) because pairing cost grows quadratically.
* `output=patch` requires index array matching — JSON Pointer positions are not well
  defined once elements are paired by key or as a set.
* Deeply nested documents (beyond serde_json's ~128 level recursion limit) are rejected as
  invalid JSON rather than crashing.
* Both inputs must be valid JSON; errors name the side and the line/column, e.g.
  `left response is not valid JSON: expected value at line 1 column 2`.

## FAQ

<details>
<summary>How is this different from a plain JSON diff?</summary>

A plain diff reports every difference, including the request ids and timestamps that change
on every call. This tool is built around suppressing that churn: ignore lists by name, path
or glob, value-shape filters for timestamps and UUIDs, a numeric tolerance for rounding
drift, and array matching by key so a reordered list is not reported as a rewrite.

</details>

<details>
<summary>Do the ignored fields still show up somewhere?</summary>

Yes. Every skipped location is counted in `counts.ignored` and listed in `ignored_paths`
(and the summary output prints the ignored count). Nothing is dropped invisibly, so you can
double-check that a filter matched what you intended — and only what you intended.

</details>

<details>
<summary>Why did my `ignore` pattern not match?</summary>

A pattern that contains a dot or a bracket is treated as an **anchored path** from the root,
so `data.token` does not match `$.result.data.token`. Use a bare name (`token`), a
`**` prefix (`$.**.token`), or a wildcard segment (`$.*.data.token`). A bare name always
matches at any depth, which is usually what you want for volatile fields.

</details>

<details>
<summary>Can I compare responses whose numbers differ slightly?</summary>

Set a **numeric tolerance**. With `0.01`, a price of `10.00` and `10.005` compare equal
while `10.5` is still reported. This is the usual fix for floating-point rounding, currency
conversion and aggregation drift between two backends.

</details>

<details>
<summary>What do the change kinds mean?</summary>

`added` is present only in the second response, `removed` only in the first, `changed` is
present in both with a different value of the same type, and `type_changed` means the JSON
type itself moved (a number became a string, a value became `null`). Turn on
*Compare across types* if `"5"` and `5` should count as equal.

</details>

<details>
<summary>Is the JSON Patch output applicable to the first response?</summary>

It is a standard RFC 6902 array of `add` / `remove` / `replace` operations built from the
reported changes, so applying it to the first response reproduces the second — except for
anything you deliberately ignored, which is left untouched by design. Patch output requires
index array matching.

</details>
