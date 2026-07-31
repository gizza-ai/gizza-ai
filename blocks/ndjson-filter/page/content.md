## About this tool

NDJSON / JSON Lines Filter keeps just the rows and fields you want from newline-delimited JSON — one JSON object per line, the format that log pipelines, data exports, and streaming APIs emit. Write a small predicate to keep matching records, list the fields to keep or rename, and export the result as JSONL, a pretty JSON array, or CSV. Everything runs in your browser through WebAssembly — nothing is uploaded, so it is safe for private logs and exports.

A predicate is one expression made of `path op value` clauses. Operators are `==`, `!=`, `>`, `>=`, `<`, `<=`, `contains`, `startswith`, `endswith`, and `~` (regex, also spelled `matches`). Join clauses with `and`, `or`, and `not`, and group them with parentheses. A bare path on its own (e.g. `active`) matches when that field exists and is truthy. Comparisons are numeric when both sides are numbers, otherwise string. Dotted paths (`user.name`) and array indexes (`items.0.id`) reach into nested data.

Worked example — filter rows, then keep two fields:

Input (NDJSON):

```
{"name":"Alice","age":30,"city":"NYC","active":true}
{"name":"Bob","age":25,"city":"LA","active":false}
{"name":"Carol","age":40,"city":"NYC","active":true}
```

Predicate: `city == NYC and age > 28` — Fields: `name, age` — Format: `ndjson`

Output:

```
{"name":"Alice","age":30}
{"name":"Carol","age":40}
```

Worked example — rename nested fields and export CSV:

Input:

```
{"user":{"name":"Al","id":7},"city":"NYC"}
{"user":{"name":"Bo","id":9},"city":"LA"}
```

Fields: `name=user.name, uid=user.id, city` — Format: `csv`

Output:

```
name,uid,city
Al,7,NYC
Bo,9,LA
```

## Limits and edge cases

- Input is pasted text held in memory, so it is bound by your browser's available RAM — it is not built for streaming multi-gigabyte files.
- Blank lines are always skipped. By default a line that is not valid JSON stops the run with a `line N: invalid JSON …` error; turn on **Skip invalid JSON lines** to silently drop malformed lines instead.
- A missing path is treated as absent: `==` matches only `null`, `!=` matches any non-null, and the ordering/`contains` operators are false. In **Fields**, a missing path yields `null`.
- `contains`, `startswith`, and `endswith` are case-sensitive substring tests; `~`/`matches` uses Rust `regex` syntax (no lookbehind/backreferences) and reports invalid patterns.
- `and` binds tighter than `or`; use parentheses to force other grouping. `!` is not a negation operator — use the word `not` (or the **Invert** toggle for the whole predicate).
- CSV columns are the first-seen union of the kept records' keys; cells containing a comma, quote, or newline are quoted RFC-4180 style, and `null` renders as an empty cell. Records that are not objects contribute empty cells.
- **Max rows** stops filtering once that many rows are kept (`0` = unlimited); **Invert** keeps the rows that do NOT match the predicate.

## FAQ

<details>
<summary>What is NDJSON / JSON Lines, and how is it different from a JSON array?</summary>

NDJSON (also called JSON Lines or JSONL) puts one complete JSON value — usually an object — on each line, with no enclosing brackets or commas between records. A JSON array wraps every element in `[ … ]` and separates them with commas. NDJSON streams well because each line is independent, which is why logs and exports use it. Set **Output format** to `array` to convert the kept rows back into a single pretty-printed JSON array, or `ndjson` to keep them line-delimited.

</details>

<details>
<summary>How do I filter on a nested or array field?</summary>

Use a dotted path. `user.name == Al` reads the `name` key inside a `user` object, and numeric segments index into arrays, so `items.0.id > 100` looks at the first element of an `items` array. The same paths work in the **Fields** box, so you can flatten nested data — `id=user.id, city` pulls `user.id` up to a top-level `id` column. A path that does not exist in a record resolves to absent (missing) for the predicate and to `null` for field selection.

</details>

<details>
<summary>Can I use a regular expression?</summary>

Yes. Use the `~` operator (or the word `matches`), for example `email ~ \.org$` to keep addresses ending in `.org`, or `name ~ ^A` for names starting with `A`. Patterns use the Rust `regex` engine, which supports the common syntax but not lookbehind or backreferences. An invalid pattern is reported as an `invalid regex` error rather than silently matching nothing.

</details>

<details>
<summary>How do I keep the rows that DON'T match?</summary>

Turn on **Invert** to keep the complement of your predicate — every record that the predicate rejects. For example, predicate `active` with **Invert** on keeps only the inactive rows. Within a predicate you can also negate a single clause with `not`, e.g. `not (city == NYC)`; **Invert** flips the whole expression at once.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The filtering runs entirely in your browser through WebAssembly — the text you paste never leaves your machine, so it is safe for private logs, database exports, and API payloads. There is no server round-trip and no sign-up.

</details>
