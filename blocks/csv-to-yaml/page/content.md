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

## FAQ

<details>
<summary>How does type inference decide what becomes a number or boolean?</summary>

With inference on (the default), an empty cell becomes `null`, `true`/`false`
become booleans, and anything that parses as an integer or float becomes a
number. Values that would lose information — like the leading-zero code `007`
or a `+1` — deliberately stay strings. Switch inference off to keep **every**
cell a string.

</details>

<details>
<summary>What if my CSV has no header row?</summary>

Untick the header option and the tool generates `col1`, `col2`, … keys for
each column instead. The same fallback fills in any *blank* header cells when
the header option is on, so you never get an empty YAML key.

</details>

<details>
<summary>Can rows have different numbers of cells?</summary>

Yes — parsing is flexible. The object keys come from the widest row, and any
missing cells in shorter rows are emitted as empty values, so every YAML
object has the same set of keys.

</details>

<details>
<summary>Which delimiters are supported?</summary>

Comma (default), tab, semicolon, and pipe — pick one explicitly if your file
isn't comma-separated, since the delimiter is not auto-detected.

</details>
