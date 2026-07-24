## About this tool

JSONL (also called NDJSON) stores one JSON value per line — the format exported by log
pipelines, database dumps, LLM datasets, and streaming APIs. Concatenating or replaying such
streams often duplicates records, and most JSON tools only understand a single big array, not
line-delimited data.

This deduplicator works on JSONL natively: paste your lines and it removes duplicates while
keeping the surviving records in their original order. Leave the **key fields** blank to match
on the exact text of each line, or name one or more fields — for example `id`, or
`user.id,email` — to treat two records as the same when those fields match, even if the rest of
the record (or the key order) differs. Nested fields use dot paths (`user.id`), and a numeric
segment indexes into an array (`tags.0`).

Choose whether to keep the **first** or the **last** occurrence of each duplicate, turn on
case-insensitive matching for things like emails, and decide what happens to a line that isn't
valid JSON: stop with an error that names the line, skip it, or keep it matched by its raw text.
Everything runs locally in your browser through WebAssembly — no upload, no account, nothing
leaves your machine.

## FAQ

<details>
<summary>What's the difference between whole-line matching and key-field matching?</summary>

Leave **key fields** blank and two records are duplicates only when their lines are byte-for-byte
identical (after trimming surrounding whitespace) — so `{"a":1,"b":2}` and `{"b":2,"a":1}` are
treated as *different*. Enter one or more key fields and records are compared only on those
fields' values, which are normalised first, so field order no longer matters and
`{"id":1,"x":2}` collapses with `{"x":9,"id":1}` when you dedupe on `id`.

</details>

<details>
<summary>How do I deduplicate on a nested or array field?</summary>

Use a dot path. `user.id` reads the `id` inside the `user` object, and a numeric segment indexes
an array — `tags.0` is the first element of `tags`. You can combine several in the comma-separated
list, e.g. `user.id,email`, to match on a composite key. A record missing a named field is grouped
with other records that also lack it, which stays distinct from a record whose field is an
explicit `null`.

</details>

<details>
<summary>Does it keep the original order of my records?</summary>

Yes. Whether you keep the **first** or the **last** occurrence, the surviving records are emitted
in the order they appeared in your input. Keeping the last occurrence places each kept record at
the position of its final appearance — unlike sort-based deduplicators, the stream is never
reordered.

</details>

<details>
<summary>What happens to blank lines and invalid JSON?</summary>

Blank or whitespace-only lines are ignored — never counted and never emitted. When you dedupe by
key fields, a line that isn't valid JSON is handled by the **Line that isn't valid JSON** setting:
**Error** stops and names the offending line number, **Skip** drops it, and **Keep** keeps it and
matches it by its raw text. In whole-line mode no JSON is parsed at all, so any text is just a
line.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The tool is compiled to WebAssembly and runs entirely inside your browser tab. Your JSONL
never leaves your device, so it's safe to paste exports that contain personal or proprietary data.

</details>
