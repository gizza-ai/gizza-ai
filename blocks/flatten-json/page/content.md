## About this tool

**Flatten JSON** collapses a nested JSON document into a single level of
**path → value** pairs, so `{"user":{"name":"Ada"}}` becomes `{"user.name":"Ada"}`.
It also runs the other way: point it at a flat map of paths and it rebuilds the
nested document. Both directions are lossless, so you can flatten, edit or diff
the flat form, and unflatten it back. Everything runs locally in your browser —
your JSON is never uploaded.

Paths use the familiar **lodash / `dot-object` / `flat`** style, not RFC 9535
JSONPath:

- **Object keys** are joined by the **separator** — `.` by default (`user.address.city`),
  but `_` or `/` or any 1–8 characters work.
- **Array elements** use the **array index notation** — `bracket` (default) writes
  `tags[0]`, `separator` writes `tags.0`.

### Worked example

Given this document:

```json
{"user":{"name":"Ada","tags":["admin","beta"]},"active":true}
```

flattening with the defaults gives:

```json
{
  "user.name": "Ada",
  "user.tags[0]": "admin",
  "user.tags[1]": "beta",
  "active": true
}
```

Paste that result back in with **direction = unflatten** and you get the original
nested document, byte for byte — key order is preserved, not alphabetised.

Change the output format to get the same data in a different shape. With
`separator = _`, `key case = upper` and `output = pairs`, the document
`{"db":{"host":"localhost","port":5432},"debug":false}` becomes an env-file:

```text
DB_HOST=localhost
DB_PORT=5432
DEBUG=false
```

and `output = csv` gives a two-column sheet you can paste straight into a
spreadsheet:

```text
key,value
order.id,A-17
order.items[0].sku,X1
order.items[0].qty,2
```

`output = paths` prints just the path list — handy for auditing which fields an
API actually returns.

### Options worth knowing

- **Direction** — `flatten` (default), `unflatten`, or `auto`. `auto` unflattens
  only when the input is a one-level object whose keys already look like paths,
  and flattens otherwise.
- **Max depth** — `0` (default) flattens everything. With `2`,
  `{"a":{"b":{"c":1}}}` becomes `{"a.b":{"c":1}}` and anything deeper stays as a
  nested JSON value.
- **Expand arrays** — turn it off to keep every array whole as one JSON value
  while still flattening objects, which is what you want when a list is a single
  logical cell.
- **Keep empty objects and arrays** — on by default, so `{}` and `[]` survive as
  leaf entries and round-trip. Turn it off to drop those keys.
- **Key case** — `upper` gives ENV-style keys, `lower` gives SQL-style column
  keys, `preserve` (default) keeps the source spelling.

### Limits & edge cases

- **Key collisions are an error, not a silent merge.** If a source key already
  contains the separator, two different paths can flatten to the same key — the
  run fails and names the key so you can pick another separator. The same applies
  when `upper`/`lower` casing merges two keys that differed only in case.
- **Unflatten refuses conflicting paths.** Supplying both `a` and `a.b` errors
  instead of overwriting whichever came second.
- **Bracket vs dotted indices change the round-trip.** With `separator` notation
  an all-digit segment rebuilds an **array**; with `bracket` notation it stays an
  object key literally named `"0"`. Use the same setting in both directions.
- **Upper/lower key case is lossy** — the original key spelling can't be
  recovered by unflattening.
- **Non-JSON output formats are flatten-only.** Unflattening always returns
  nested JSON, so `pairs`/`csv`/`paths` error rather than being ignored.
- Input is capped at **5 MB**, **100 levels** of nesting, **200,000** flattened
  keys, and array indices up to **100,000** (so a typo like `a[999999999]` errors
  instead of allocating a huge array).
- A top-level array flattens too: `[{"a":1}]` gives `{"[0].a":1}`.

## FAQ

<details>
<summary>What's the difference between bracket and dotted array indices?</summary>

`bracket` writes `tags[0]`, `separator` writes `tags.0` using whatever separator
you chose. Bracket is the default because it is unambiguous: `[0]` can only mean
an array element, so a bare numeric segment is free to mean an object key
literally named `"0"`. With `separator` notation the rule flips — an all-digit
segment rebuilds an array. Both round-trip cleanly as long as you flatten and
unflatten with the **same** setting.

</details>

<details>
<summary>Can I get the nested JSON back after flattening?</summary>

Yes — that's what `direction = unflatten` does. Paste the flat map of paths back
in and the nested document is rebuilt, with key order preserved. The round-trip
is lossless for every option except `key case = upper`/`lower` (which discards
the original spelling) and a `max depth` cap combined with dropping empty
containers.

</details>

<details>
<summary>Why did I get a "two different paths flatten to the same key" error?</summary>

Because a key in your document already contains the separator. For example
`{"a.b":1,"a":{"b":2}}` produces the path `a.b` twice — one from the literal key,
one from the nested object. Rather than silently dropping a value, the tool stops
and names the key. Pick a separator that doesn't appear in your keys (`/`, `::`
and `__` are common choices).

</details>

<details>
<summary>How do I turn a nested API response into a spreadsheet?</summary>

Set **output** to `csv`. You get a two-column `key,value` sheet with a header
row, correctly quoted for values containing commas, quotes or newlines — paste it
straight into a spreadsheet. If you'd rather have an env file or a `.properties`
style dump, use `pairs`, which prints `path=value` lines with strings unquoted.

</details>

<details>
<summary>What happens to empty objects, arrays, and nulls?</summary>

`null` is an ordinary value and always survives as a leaf. Empty objects and
arrays have nothing inside to make a path from, so they're controlled by **keep
empty objects and arrays**: on (the default) they're emitted as `{}` / `[]` leaf
entries so the key survives the round-trip; off, those keys are dropped entirely.

</details>

<details>
<summary>Is my JSON uploaded anywhere?</summary>

No. The whole tool is compiled to WebAssembly and runs inside your browser tab,
so the document never leaves your device. That makes it safe for config files,
API responses, and anything else you wouldn't paste into a server-side
converter.

</details>
