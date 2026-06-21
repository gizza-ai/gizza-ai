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
