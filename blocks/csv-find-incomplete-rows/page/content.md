## About this tool

**Find incomplete CSV rows** scans a CSV and flags every row that doesn't fit
the shape of the file. It reports three kinds of problem:

- **Too few fields** — a row has fewer columns than the header (or first row).
- **Too many fields** — a row has more columns than expected, usually an
  unescaped delimiter inside a value.
- **Blank required cell** — a row leaves a column you marked as *required* empty.

Paste a CSV (with or without a header), choose the delimiter (`,` / tab / `;` /
`|`), and optionally list the columns that must always be filled in — by name
(e.g. `email,phone`) or by 1-based position (e.g. `1,3`). Each flagged row comes
back with its line number, its field count, and which checks it failed.

### Privacy

Everything runs **in your browser** via WebAssembly — your CSV is never uploaded.
Also available from the [gizza CLI](/) and in chat, which return the full report
as structured JSON (expected field count, column names, and every flagged row).

### Common uses

- Catch rows broken by an unescaped comma before importing a dataset.
- Find records missing a mandatory field like an email or ID.
- Validate an export's shape after a delimiter or quoting change.

## FAQ

<details>
<summary>How does the tool know how many columns a row should have?</summary>

When **header** is on (the default) the header row sets the expected field count
and the column names. With header off, the first data row sets the expected width
instead. Every other row is compared against that count and flagged as "too few"
or "too many" fields.

</details>

<details>
<summary>Will a comma inside quotes be flagged as an extra field?</summary>

No. The parser understands standard CSV quoting, so `"Smith, John"` stays one
field. A "too many fields" flag almost always means an *unescaped* delimiter — a
raw comma (or your chosen delimiter) that was never wrapped in quotes.

</details>

<details>
<summary>How do I mark columns as required?</summary>

List them comma-separated in the required field — by header name (e.g.
`email,phone`) or by 1-based position (e.g. `1,3`). A name that isn't in the
header or a position past the expected width is reported as an error rather than
silently ignored, so a typo can't hide missing data.

</details>

<details>
<summary>Which delimiters can I use?</summary>

Any single character, or the words `comma`, `tab`, `semicolon`, `pipe`. Anything
else (like a two-character string) is rejected with an error so you know the
delimiter wasn't applied.

</details>
