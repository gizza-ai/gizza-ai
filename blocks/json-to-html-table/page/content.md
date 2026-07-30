## About this tool

**JSON to HTML Table** turns a pasted JSON array or object into a clean table you
can drop into docs, CMS fields, tickets, or Markdown notes. It accepts the shapes
you usually get from APIs: arrays of objects, arrays of arrays, arrays of scalar
values, or a single object.

- **HTML or Markdown:** choose a semantic `<table>` with `<thead>`/`<tbody>` or a
  GitHub-style Markdown pipe table.
- **Object rows:** an array of objects becomes one row per object, with the union
  of every key as the columns. Missing keys and JSON `null` use your null text.
- **Array rows:** arrays of arrays can treat the first row as the header, or turn
  the header toggle off to generate `Column 1`, `Column 2`, … labels.
- **Nested values:** keep nested objects/arrays as compact JSON, render nested HTML
  tables, or flatten nested objects into dotted columns like `user.id`.
- **HTML finishing:** add an optional `<caption>`, CSS class names, and choose
  pretty indented HTML or compact single-line HTML.

### Worked example

Input JSON:

```json
[{"id":1,"name":"Ada"},{"id":2,"name":"Linus"}]
```

HTML output:

```html
<table>
  <thead>
    <tr><th>id</th><th>name</th></tr>
  </thead>
  <tbody>
    <tr><td>1</td><td>Ada</td></tr>
    <tr><td>2</td><td>Linus</td></tr>
  </tbody>
</table>
```

### Privacy

Everything runs locally in your browser via WebAssembly; pasted JSON is never
uploaded. The same formatter is available from the gizza CLI and in chat.

### Limits and edge cases

Top-level JSON must be an array or object. Empty arrays/objects and scalar-only
JSON documents are rejected because there is no table shape to infer. Markdown
cannot contain nested Markdown tables, so the **Nested HTML tables** option falls
back to compact JSON when Markdown output is selected.

## FAQ

<details>
<summary>What JSON shapes can I paste?</summary>

Paste an array of objects, an array of arrays, an array of scalar values, or a
single object. Arrays of objects become row tables, arrays of arrays use either
the first row as the header or generated `Column N` headers, scalar arrays become
a single-column table, and a single object becomes a two-column key/value table.

</details>

<details>
<summary>How are different object keys handled?</summary>

For an array of objects, columns are the union of every key in first-seen order.
If one row is missing a key that another row has, the missing cell renders as the
null / missing text you provide (empty by default).

</details>

<details>
<summary>What should I choose for nested objects or arrays?</summary>

Use **Compact JSON in cells** for a safe default that works in both HTML and
Markdown. Use **Nested HTML tables** when you want nested `<table>` elements in
HTML output. Use **Flatten object keys** when nested objects should become columns
such as `user.id` and `user.name`; arrays still stay as compact JSON strings.

</details>

<details>
<summary>Can I style the generated HTML table?</summary>

Yes. Add a class list such as `table table-striped` in the HTML table class field,
and optionally add a caption. The tool only writes the table markup; your site or
document provides the CSS that styles those classes.

</details>
