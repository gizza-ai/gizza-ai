## About this tool

**CSV to XML** turns a CSV table into XML records. Each row becomes a record
element, and each field becomes `<tag>value</tag>` using the **header name** as the
tag:

```xml
<rows>
  <row>
    <name>Ada</name>
    <age>36</age>
  </row>
</rows>
```

- Header names are **sanitized to valid XML element names** (spaces and other
  invalid characters become `_`; a leading digit gets a `_` prefix).
- Values are **XML-escaped** (`&`, `<`, `>`).
- Customise the **root** and **record** tags, toggle the header row, and pick the
  delimiter (`,` / tab / `;` / `|`).

### Privacy

Everything runs **in your browser** via WebAssembly — your CSV is never uploaded.
Also available from the [gizza CLI](/) and in chat.

## FAQ

<details>
<summary>My CSV has no header row — what tags do the fields get?</summary>

Turn the **header** toggle off and the fields are emitted as `<col1>`, `<col2>`,
`<col3>`… in order. With the toggle on (the default), the first row supplies the
tag names and is not emitted as a record itself.

</details>

<details>
<summary>What happens to column names with spaces or leading digits?</summary>

They are sanitized into valid XML element names: spaces and other illegal
characters become `_`, and a name starting with a digit gets a `_` prefix — so a
header like `2024 sales` becomes `<_2024_sales>`. Cell values are separately
XML-escaped (`&`, `<`, `>`), so the output is always well-formed.

</details>

<details>
<summary>Can I rename the &lt;rows&gt; and &lt;row&gt; wrapper elements?</summary>

Yes — the **root** tag (default `rows`) wraps the whole document and the
**record** tag (default `row`) wraps each CSV line. Set them to e.g. `products`
and `product` to match the schema your importer expects.

</details>

<details>
<summary>Does it handle semicolon- or tab-separated files?</summary>

Yes. The delimiter accepts any single character or the words `comma`, `tab`,
`semicolon`, `pipe` — handy for European-style `;` exports or TSV clipboard
pastes.

</details>
