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
