## About this tool

**CSV to YAML** turns a CSV table into a **YAML list of objects**, one object per
row, keyed by the header:

```yaml
- name: Ada
  age: 36
- name: Bo
  age: 40
```

- **Column order is preserved.**
- **Type inference** (on by default) turns cell text into numbers, booleans, and
  null — while leaving leading-zero codes like `007` as strings. Turn it off to
  keep every value a string.
- Toggle the header row and pick the delimiter (`,` / tab / `;` / `|`).

### Privacy

Everything runs **in your browser** via WebAssembly — your CSV is never uploaded.
Also available from the [gizza CLI](/) and in chat.

### Common uses

- Turn a spreadsheet into a YAML config or fixtures file.
- Convert tabular data for a tool that expects YAML.
