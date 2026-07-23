## Reshape any structured data with one query

Paste a document in **JSON, YAML, CSV, or XML**, write a **JSONata** expression,
and get a brand-new structure back as JSON or YAML. It replaces the usual
"convert to JSON → run a query → template the output" dance with a single step,
across all four input formats. Everything runs locally in your browser with a
pure-Rust engine — your data is never uploaded to a server.

[JSONata](https://jsonata.org) is one small language for both **querying**
(navigate paths, filter with predicates, aggregate) and **templating**
(construct new objects and arrays). Set the input format, or leave it on
**Auto-detect** to sniff it from the content.

### Worked example — CSV in, summary object out

Input (CSV):

```
name,price
apple,2
banana,3
cherry,5
```

Query:

```
{ "total": $sum($.price), "items": $count($), "cheapest": $min($.price) }
```

Output (JSON):

```json
{
  "total": 10,
  "items": 3,
  "cheapest": 2
}
```

CSV rows arrive as an array of objects (here `$` is that array), and numeric
cells are coerced to numbers so `$sum`/`$min` work directly.

### More examples

- `orders.{ "id": id, "total": $sum(lines.amount) }` — reshape each object into a new shape.
- `items[price > 10].name` — filter with a predicate, then project a field.
- `$sum(order.item.price)` — aggregate values pulled from nested XML elements.
- `{ "name": user.first & " " & user.last }` — combine YAML fields into a new object.
- `$sort($, function($a, $b){ $a.price > $b.price })` — sort an array with a comparator.

### Notes and limits

- **Input formats:** JSON, YAML, CSV (with a header row), and XML. CSV becomes an
  array of row objects; XML elements map to nested objects (attributes are keyed
  with a leading `@`, repeated tags become arrays).
- **Output formats:** JSON or YAML. Pretty-print applies to JSON only; YAML is
  always block style.
- **Type coercion:** CSV and XML text that looks numeric or boolean is coerced to
  real numbers/booleans so arithmetic and aggregation work; empty CSV cells become `null`.
- A query that matches nothing returns `null` — this is normal in JSONata, not an error.
- Object key order in the output follows the engine's serializer.

## FAQ

<details>
<summary>What query language does this use?</summary>

It uses **JSONata**, a lightweight expression language for querying and
transforming structured data. A single expression can navigate paths
(`account.orders.total`), filter (`items[qty > 0]`), aggregate (`$sum`, `$count`,
`$max`, `$average`), and construct new objects and arrays
(`{ "id": id, "total": $sum(lines.amount) }`). Because the input is parsed into a
common model first, the same expression works whether you started from JSON,
YAML, CSV, or XML.

</details>

<details>
<summary>How is CSV turned into something I can query?</summary>

CSV is parsed with its first row as headers into an **array of objects** — one
object per data row, keyed by the header names. Reference the whole array as `$`
(for example `$sum($.price)`) or a single field as `price`. Cells that look like
numbers or `true`/`false` are converted to real numbers and booleans, and empty
cells become `null`, so aggregation and comparisons behave as expected.

</details>

<details>
<summary>How does XML map onto the query model?</summary>

Each XML element becomes an object keyed by its child element names; repeated
child tags collapse into an array; attributes are included under keys prefixed
with `@`; and text/number/boolean-looking content is coerced to JSON scalars. So
`<order><item><price>10</price></item></order>` lets you write
`$sum(order.item.price)`.

</details>

<details>
<summary>Why did my query return null?</summary>

In JSONata a path that matches nothing yields *undefined*, which this tool
normalizes to `null`. The usual causes are a key-name or case mismatch, or
querying an object where an array was expected (or vice versa). Genuine problems
— a syntax error in the expression or an unparseable input document — are
reported as errors, not `null`.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. Parsing, the JSONata evaluation, and serialization all run inside your
browser via WebAssembly. Nothing is sent to a server, so you can reshape private
or sensitive data safely.

</details>
