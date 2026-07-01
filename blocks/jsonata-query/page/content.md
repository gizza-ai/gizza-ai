## Query and transform JSON with JSONata

Paste a JSON document, type a **JSONata** expression, and see the result
instantly. Everything runs locally in your browser with a pure-Rust JSONata
engine — your data is never uploaded to a server.

[JSONata](https://jsonata.org) is a lightweight query and transformation
language for JSON: navigate paths, filter with predicates, aggregate with
built-in functions, and build any output structure you like.

### Examples

- `Account.Order.Product.Price` — navigate a path into nested objects.
- `items[price > 10].name` — filter elements with a predicate, then project a field.
- `$sum(items.price)` — aggregate with a built-in (`$sum`, `$count`, `$max`, `$average`, …).
- `orders.{ "id": orderId, "total": $sum(lines.amount) }` — reshape into a new object.
- `$sort(products, function($a, $b) { $a.price > $b.price })` — sort with a comparator.

### Notes

- The result is returned as JSON. A query that matches nothing returns `null`.
- Object key order follows the JSONata engine's serializer. Tick **Pretty-print** for indented output.
- String functions (`$uppercase`, `$substring`, `$split`, `$join`), numeric
  functions (`$round`, `$floor`, `$abs`), and higher-order functions (`$map`,
  `$filter`, `$reduce`) are all available.

## FAQ

<details>
<summary>My expression returns null — did something go wrong?</summary>

Not necessarily. In JSONata a path that matches nothing yields *undefined*, which
this tool normalizes to JSON `null`. The usual culprits are a case mismatch in a
key name or querying an object where an array was expected. Genuine problems — a
syntax error in the expression or invalid JSON input — are reported as errors,
not `null`.

</details>

<details>
<summary>Which JSONata functions can I use?</summary>

The engine covers the standard library you'd expect: aggregation (`$sum`,
`$count`, `$max`, `$average`), strings (`$uppercase`, `$substring`, `$split`,
`$join`), numbers (`$round`, `$floor`, `$abs`), higher-order functions (`$map`,
`$filter`, `$reduce`, `$sort` with a comparator), plus predicates, wildcards, and
object/array construction for reshaping output.

</details>

<details>
<summary>Can a runaway expression hang the page?</summary>

No — evaluation is guarded by a recursion-depth cap of 1024 and a roughly
one-second evaluation budget, so an accidentally infinite recursive function ends
with an error instead of freezing the tab.

</details>

<details>
<summary>Is this the same engine as jsonata.org?</summary>

It implements the same language, but it's a pure-Rust engine compiled to
WebAssembly rather than the reference JavaScript library — that's what lets your
data stay in the browser. Results agree for standard expressions; cosmetic
details like object key order follow this engine's serializer.

</details>
