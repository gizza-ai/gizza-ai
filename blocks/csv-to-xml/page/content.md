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
