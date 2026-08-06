## About this tool

JSON Transform Rules reshapes a JSON document with a small declarative rule list. Each rule names an output target path and where its value comes from: a JSONPath-style selector, a literal value, or a removal operation in merge mode. It is useful when you need to normalize an API response, build a smaller payload for another system, redact fields before sharing a sample, or debug how a mapping will behave before you put it in code.

The shorthand form is designed for quick work:

```
id = $.user.id
name = $.user.name
email = $.user.contact.email
source = "import"
```

For larger mappings, paste a JSON object of `target -> selector` pairs or a JSON array of rule objects with `target`, `source`, `value`, `default`, `transform`, `separator`, `when`, and `op`. Selectors support `$`, `.key`, `["key"]`, `[0]`, `[*]`, `.*`, and `..key`. Target paths use dot and bracket notation, create missing objects and arrays, and can append with `tags[]`.

Worked example — map a nested API response into a flatter payload:

Source JSON:

```
{
  "user": {"id": 7, "name": "Ada Lovelace", "email": "ada@example.com"},
  "orders": [{"total": 19.5}, {"total": 30.5}]
}
```

Rules:

```
id = $.user.id
name = $.user.name
email = $.user.email
total = $..total
source = "import"
```

Output:

```
{
  "id": 7,
  "name": "Ada Lovelace",
  "email": "ada@example.com",
  "total": [
    19.5,
    30.5
  ],
  "source": "import"
}
```

Use **Each selector** when one input array should become one output object per item. For example, set `each` to `$.automobiles[*]`, then rules like `title = $.model` and `year = $.year` run against every automobile and return an array of mapped objects. Use **Output → Rule match report** when a mapping comes out empty; it shows which rules matched, wrote, skipped, or used defaults.

## Limits and edge cases

- Source JSON is capped at **5 MB**, rules at **200 KB**, and one run at **500 rules**. A selector can match at most **100,000 values**, and `each` can fan out over at most **50,000 items**.
- This is a JSONPath subset, not a full query language. Filters such as `[?(@.price > 10)]`, slices, arithmetic expressions, and script callbacks are intentionally out of scope. Use `when` for simple truthy guards and transforms such as `sum`, `count`, `first`, `join`, `upper`, `trim`, `keys`, and `values` for common reshaping steps.
- `mode = build` starts with an empty object. `mode = merge` starts with a copy of the input, so `-path` shorthand or `op = "remove"` can redact fields.
- `on_missing = skip` omits missing targets, `null` writes `null`, and `error` stops on the first missing selector. A rule-level `default` overrides all three.
- `array_mode = auto` returns a scalar for one match and an array for many matches. Choose `always` for stable array shapes or `first` when only the first value should survive.
- Target array indexes are capped at **10,000** to catch accidental sparse-array writes like `items[999999]`.

## FAQ

<details>
<summary>Is this a full JSONPath implementation?</summary>

No. It supports the selector pieces that are safe and predictable inside this tool: root `$`, dotted keys, quoted keys, array indexes, wildcards, and recursive descent by key. It does not implement filter predicates, slices, unions, or embedded scripts. That keeps the mapping portable across the browser, CLI, and sandboxed chat runtime.

</details>

<details>
<summary>When should I use build mode versus merge mode?</summary>

Use **build** when you want a fresh output object containing only the fields named by the rules. Use **merge** when you want to patch or redact the original document: the output starts as a copy of the input, then rules overwrite paths and `-path` rules remove paths such as `-ssn` or `-debug.trace`.

</details>

<details>
<summary>How do I map every item in an array?</summary>

Put the array selector in **Each selector**, for example `$.items[*]`. The rules then run with each item as their local root, so `sku = $.sku` and `price = $.price` produce one output object per item. Leave **Each selector** blank when all rules should run once against the whole source document.

</details>

<details>
<summary>What happens when a selector matches more than one value?</summary>

With the default **auto** mode, one match becomes a scalar and multiple matches become an array. Select **Always arrays** when downstream code needs a stable array shape, or **First match only** when a selector can match many values but only the first should be kept. Aggregate transforms such as `sum`, `count`, `min`, `max`, `avg`, and `join` return one value after aggregating the matches.

</details>

<details>
<summary>Is my JSON uploaded anywhere?</summary>

No. The tool runs locally in WebAssembly in the browser page, and the same core runs in the command line and sandboxed chat block. Your pasted JSON and rules are not sent to a server by this page.

</details>
