## About this tool

Faceted Search & Filter for JSON turns a pasted array of records into the two things a faceted-navigation UI needs: the records that match the current search, and a value → count breakdown ("Brand: Northwind 2, Summit 1, Trailhead 1") for every facet field. It is the query-time half of a search sidebar without an index, a server, or an account — the whole search runs in your browser through WebAssembly, so the data you paste never leaves your machine.

Paste a JSON array (a single object, or NDJSON / JSON Lines with one record per line, also work). Optionally narrow the set with **Search text** (all whitespace-separated terms must appear somewhere in the record) and a **Filter expression**, sort and page the survivors, and list the **Facet fields** you want counted — or leave that blank to auto-detect. Facet counts are always computed over the whole match set, not just the page you are looking at.

A filter expression is made of `path op value` clauses. Operators are `==`, `!=`, `>`, `>=`, `<`, `<=`, `contains`, `startswith`, `endswith`, `~` (regex, also spelled `matches`), and `in` (membership: `brand in [Northwind, Summit]`). Join clauses with `and`, `or`, `not`, and group them with parentheses; `and` binds tighter than `or`. A bare path on its own (`in_stock`) is true when that field exists and is truthy. Comparisons are numeric when both sides are numbers, otherwise string. Dotted paths (`meta.color`) and array indexes (`items.0.id`) reach into nested data, and a clause on an array field holds when **any** element satisfies it.

Worked example — count facet values across a small catalog:

Input (JSON dataset):

```
[
  {"name":"Aero Jacket","brand":"Northwind","price":180,"tags":["outdoor","sale"],"in_stock":true},
  {"name":"Basalt Boots","brand":"Northwind","price":120,"tags":["outdoor"],"in_stock":false},
  {"name":"Cirrus Cap","brand":"Trailhead","price":25,"tags":["sale"],"in_stock":true},
  {"name":"Delta Pack","brand":"Summit","price":95,"tags":["outdoor","travel"],"in_stock":true}
]
```

Facet fields: `brand, tags` — Output: `summary`

Output:

```
4 records matched — page 1 of 1 (4 shown, 10 per page)

brand (3 distinct values)
  Northwind  2
  Summit     1
  Trailhead  1

tags (3 distinct values)
  outdoor  3
  sale     2
  travel   1
```

`tags` counts to 6 across 4 records because each array element is counted on its own — that is what a collection facet means.

Worked example — filter, then read the recounted facet:

Filter expression: `tags contains outdoor and in_stock` — Facet fields: `brand` — Sort records: `price:desc` — Output: `facets`

Output:

```
[
  {
    "field": "brand",
    "distinct": 2,
    "values": [
      {
        "value": "Northwind",
        "count": 1
      },
      {
        "value": "Summit",
        "count": 1
      }
    ]
  }
]
```

Two of the four records survive, and the `brand` facet is recounted over just those two — Trailhead drops out entirely because nothing it contains matches.

Worked example — numeric facet stats:

Facet fields: `price` — Facet value order: `Value A→Z / low→high` — **Add min / max / avg / sum for numeric facets** on — Output: `summary`

Output:

```
4 records matched — page 1 of 1 (4 shown, 10 per page)

price (4 distinct values)
  25   1
  95   1
  120  1
  180  1
  min 25 · max 180 · avg 105 · sum 420
```

## Limits and edge cases

- Up to **50,000 records** per run, **1,000 rows per page**, and **1,000 values per facet** (`0` in *Values per facet* means unlimited). The dataset is held in memory, so very large inputs are bound by your browser's available RAM.
- With **Facet fields** blank, the first **20** top-level fields that hold a scalar or an array of scalars are auto-detected, in first-seen key order. Fields whose values are objects (or arrays of objects) are skipped — name a dotted path such as `meta.color` to facet those.
- Each facet reports `distinct`, the full number of values found, alongside the (possibly truncated) `values` list, so a *Values per facet* cap is always visible rather than silently hiding the tail.
- Facet counts are scoped to the **current result set**, and array elements count individually — so a facet's counts can legitimately exceed the match total. A field with a unique value per record (an id) makes a useless facet: as many buckets as rows.
- A page past the end is **not** an error: `items` comes back empty with the real `total` and `total_pages`. Page numbers are 1-based.
- `contains`, `startswith`, and `endswith` are case-sensitive substring tests, while **Search text** is case-insensitive. `~`/`matches` uses Rust `regex` syntax (no lookbehind or backreferences) and reports an invalid pattern instead of matching nothing.
- `!` on its own is not a negation operator — use the word `not`. `!=` on a record that lacks the field is true (the record does not hold that value).
- Sorting puts missing and `null` values last, and ties keep the input order. Mixed-type columns still order deterministically (booleans, then numbers, then strings).
- **Facet counting** changes only the counts, never the returned records: *multi-select friendly (disjunctive)* ignores a facet's own filter clauses when counting that one facet, so the values a user has not selected yet keep non-zero counts.

## FAQ

<details>
<summary>What is a facet, and how is this different from just filtering JSON?</summary>

A facet is a field broken down into its distinct values with a count for each — `brand: Northwind (2), Summit (1)` — computed from the records that currently match. A plain filter answers "which records match?"; faceting also answers "what else could I narrow by, and how many rows would each choice leave?" That is the data behind the checkbox sidebar on a shop or docs search page. This tool returns both halves together: `items` for the rows and `facets` for the counts, plus `total` / `page` / `per_page` / `total_pages`.

</details>

<details>
<summary>When should I use disjunctive instead of conjunctive counting?</summary>

Use **multi-select friendly (disjunctive)** when a facet's checkboxes should stay clickable after the first one is ticked. In conjunctive mode, filtering on `brand in [Northwind]` recounts the `brand` facet over only Northwind rows, so every other brand shows zero and the sidebar becomes a dead end. Disjunctive mode ignores that facet's **own** clauses when counting it, so Summit and Trailhead keep their real counts and a user can add a second brand. Other facets are still counted with the brand filter applied, and the returned records are identical in both modes.

</details>

<details>
<summary>How do I facet or filter on a nested or array field?</summary>

Use a dotted path. `meta.color` reaches the `color` key inside a `meta` object, and a numeric segment indexes an array, so `items.0.id` is the first element's id. A non-numeric segment applied to an array of objects maps over the elements, so `items.sku` reaches every sku. In a filter, a clause on an array field holds when **any** element satisfies it (`tags contains sale`), and when that path is used as a facet each element gets its own bucket.

</details>

<details>
<summary>Can I paste NDJSON / JSON Lines instead of an array?</summary>

Yes. If the input is not a valid JSON array it is retried as NDJSON — one JSON value per line, no enclosing brackets or commas — which is what log pipelines and database exports emit. Blank lines are skipped. A single JSON object is accepted as well and treated as a one-record dataset. If neither parse works you get the JSON error rather than an empty result.

</details>

<details>
<summary>How do I filter a numeric range, like a price slider?</summary>

Write two clauses joined with `and`: `price >= 50 and price < 150`. Comparisons are numeric whenever both sides are numbers, so no quoting or padding is needed, and the same works for ISO-8601 date strings as a string comparison (`date >= 2026-01-01`). To see the spread before choosing a range, facet the field and turn on **Add min / max / avg / sum for numeric facets**.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The search runs entirely in your browser through WebAssembly — the JSON you paste never leaves your machine, so it is safe for private catalogs, database exports, and API payloads. There is no server round-trip, no index to build, and no sign-up. The same tool is available offline through the command line.

</details>
