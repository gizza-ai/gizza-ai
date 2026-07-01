## About this tool

The mock data generator turns a compact shorthand **schema** — the field names
and types *you* define — into realistic-looking mock JSON. It is built for the
moment you need a fixture, a stubbed API response, or seed data shaped exactly
like your real model, without writing a generator by hand or exposing real
people's information. Every value is synthetic placeholder data.

### How the schema works

Write a comma- or newline-separated list of `field:type` pairs. You choose the
field names; each maps to a type that controls the generated value:

```
id:int(1..1000),
name:name,
email:email,
signup:date,
roles:enum(admin|editor|viewer)[2],
profile:{ city:city, country:country, lat:lat, lng:lng }
```

- **Array of values** — append `[n]` to any field, e.g. `tags:string[3]` or
  `scores:int(0..100)[5]`.
- **Nested objects** — wrap a sub-schema in braces, e.g.
  `user:{ name:name, email:email }`. Objects nest, and `{...}[n]` makes an array
  of objects.

### Supported types

- **Identifiers** — `uuid`, `int`, `float`, `bool`, `string`
- **Numeric ranges** — `int(lo..hi)`, `float(lo..hi)`
- **Choice** — `enum(a|b|c)` picks one of your options at random
- **People** — `name`, `first_name`, `last_name`, `username`, `email`, `phone`
- **Dates** — `date` (`YYYY-MM-DD`), `datetime` (ISO `…Z`)
- **Web / network** — `url`, `domain`, `ipv4`, `mac`, `color`
- **Place** — `street`, `city`, `state`, `zip`, `country`, `lat`, `lng`
- **Text** — `word`, `words`, `words(n)`, `sentence`, `paragraph`

### One object or many

Leave **Records** at `1` for a single JSON object. Set it higher (up to 1000)
and the output becomes a JSON array of that many records — perfect for seeding a
list endpoint or a table.

### Reproducible with a seed

Set the **seed** to any non-zero number and you'll get the exact same data every
time — handy for deterministic tests, fixtures, and code review. Leave it at `0`
to get fresh data on each run.

Everything runs locally in your browser via WebAssembly — your schema and the
generated data never leave your device.

## FAQ

<details>
<summary>How many records can I generate at once?</summary>

Up to **1000** per run (the **Records** field). Asking for more returns an
error rather than a partial result. With Records at `1` you get a single JSON
object; anything higher wraps the output in a JSON array.

</details>

<details>
<summary>How do I get the exact same data every time?</summary>

Set **Seed** to any non-zero number — the generator is fully deterministic for
a given schema + seed, so fixtures stay stable across runs and machines. Seed
`0` (the default) produces fresh data on every run.

</details>

<details>
<summary>What happens if I misspell a type in the schema?</summary>

You get an error that names the unknown type and lists every valid one — the
tool never silently guesses. Other schema mistakes are caught the same way:
an empty `enum()`, a field with no type, unbalanced `{` braces, or missing
`int(lo..hi)` bounds each produce a specific message.

</details>

<details>
<summary>How deeply can I nest objects?</summary>

Sub-schemas in braces (e.g. `user:{ profile:{ city:city } }`) can nest up to
**8 levels** deep, and `{...}[n]` makes an array of objects at any level.
Deeper nesting is rejected with an error.

</details>

<details>
<summary>Is any of the generated data real?</summary>

No — every name, email, address, and phone number is synthetic placeholder
data assembled from word lists, never sampled from real people. Generation
happens in your browser, so nothing you type is uploaded.

</details>
