## About this tool

JSON Array Pluck Field extracts the same key from every row in a JSON array and returns a flat list. It is useful when you have API results, export files, webhook payloads, or NDJSON logs and only need one column such as `id`, `email`, `user.name`, or `orders.*.total`.

Paste a top-level array, a wrapper object such as `{ "items": [...] }`, NDJSON lines, or a single object. Then enter a field name or dotted path. The output can be one value per line, CSV, TSV, a JSON array that preserves number and boolean types, or a custom-delimited list.

### Worked example

Input JSON:

```json
[{"user":{"name":"Ada"},"email":"ada@example.test"},{"user":{"name":"Grace"},"email":"grace@example.test"}]
```

Field:

```text
user.name
```

Output:

```text
Ada
Grace
```

Use `root` when an API response wraps rows under a property such as `data`, `items`, or `response.results`. Use `*` to fan out arrays (`orders.*.total`) and `**` or JSONPath-style `..` to find a key at any depth (`**.city`).

## Limits and edge cases

- Maximum input size is 5,000,000 bytes.
- Field and root paths are limited to 200 bytes each.
- At most 200,000 values are returned in one run.
- Missing fields are skipped by default; switch to empty, null, or error mode when row alignment matters.
- Object and array values are emitted as compact JSON by default; they can be labelled or skipped.
- This is not a general JSON query language. It deliberately focuses on the common "pluck one field from every row" workflow.

## FAQ

<details>
<summary>Can I extract nested values?</summary>

Yes. Use dotted paths such as `user.name`, indexes such as `tags.0` or `tags[0]`, wildcard paths such as `orders.*.total`, and recursive descent such as `**.city` or `$..city`.

</details>

<details>
<summary>What if my array is inside an API response object?</summary>

Set the root array path, for example `items`, `data.results`, or `response.records`. If root is blank, the tool uses a top-level array as-is or the first array-valued property in a wrapper object.

</details>

<details>
<summary>How are missing or null fields handled?</summary>

The default is to skip rows where the value is missing or null. Choose `empty` to keep row positions with blank values, `null` to emit the word `null`, or `error` to stop on the first missing row and report its index.

</details>

<details>
<summary>Can I produce CSV or a quoted list?</summary>

Yes. Choose CSV or TSV for delimiter-separated output with RFC-style quoting. Turn on "Quote every text value" for SQL-style or JavaScript-style quoted lists in lines or custom-delimited output.

</details>
