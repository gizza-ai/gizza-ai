## About this tool

**CSV structure validator** lints the raw text of a CSV and reports every
structural fault with a 1-based line number and a severity — without changing
your data. Unlike most CSV parsers, which quietly "repair" a broken file, this
tool scans the text directly, so it can show you exactly what an importer will
choke on.

It reports **errors** (the file breaks RFC 4180 parsing):

- **ragged_row** — a row's field count differs from the header/first row.
- **unclosed_quote** — a quoted field is opened but never closed (reported at
  the line where the quote opens).
- **stray_quote** — a bare quote inside an unquoted field, or text after a
  closing quote.

And **warnings** (parseable, but usually unintended):

- **blank_row** — an empty or whitespace-only line.
- **empty_row** — a row whose every field is empty (e.g. `,,`).
- **whitespace** — leading/trailing spaces around fields or quotes.
- **empty_header** / **duplicate_header** — blank or repeated header names.
- **mixed_line_endings** — the file mixes CRLF, LF, or CR.

The file is *valid* when it has zero errors — warnings alone don't fail it.

### Worked example

Input:

```
name,email,city
Ada,ada@example.com,Paris,extra
Bo,bo"x,Rome
Cy,"cy@example.com
```

Report:

```
INVALID CSV — 3 error(s), 0 warning(s).
Delimiter: comma (auto-detected) · Quote: double · Expected 3 field(s) per row · 3 data row(s)
Line 2 [error] ragged_row — expected 3 field(s), found 4
Line 3 [error] stray_quote — unquoted field 2 contains a bare quote character — wrap the field in quotes and double any embedded quote
Line 4 [error] unclosed_quote — quoted field 2 is opened here but never closed
```

### Limits and edge cases

- The expected column count comes from the **first row** (the header when
  "first row is a header" is on). Quoted fields may legally contain the
  delimiter and even newlines — they are parsed, not flagged.
- A row swallowed by an unclosed quote skips the field-count check (its count
  would be meaningless), so one mistake doesn't cascade into dozens of errors.
- A leading byte-order mark (BOM) is stripped before scanning; character
  encoding itself isn't checked — the input is already text when it arrives.
- Browser text areas normalize CRLF to LF when you paste, so the
  **mixed_line_endings** check is most useful from the CLI or chat, where the
  file's original line endings survive.
- `max_issues` (1–1000, default 50) caps the listed issues; the error and
  warning totals always cover the whole file and the report says when the list
  is truncated.
- This tool **reports only** — it never rewrites your CSV. To fix findings,
  companion tools such as [csv-cleaner](/tools/csv-cleaner/) (trim whitespace,
  drop empty rows, normalize delimiters/line endings) and
  [csv-find-incomplete-rows](/tools/csv-find-incomplete-rows/) (blank required
  cells) pick up from this report.

### Privacy

Everything runs **in your browser** via WebAssembly — your CSV is never
uploaded. The same validator is available from the CLI and in chat, which
return the full report as structured JSON (verdict, dialect, counts, and every
issue with line, severity, code, and message).

### Common uses

- Find the exact line that makes a spreadsheet import or ETL job fail.
- Check an export for unescaped quotes or delimiters before shipping it.
- Confirm a hand-edited CSV still has a consistent column count everywhere.
- Spot header problems (duplicate or blank names) before loading into a database.

## FAQ

<details>
<summary>What's the difference between an error and a warning?</summary>

Errors are faults that change how the file parses: a wrong field count, an
unclosed quote, or a stray quote character. Warnings are things a parser will
accept but that usually signal a problem: blank or all-empty rows, whitespace
around fields, header-name issues, and mixed line endings. The verdict is
**valid** whenever there are zero errors, even if warnings remain.

</details>

<details>
<summary>Will commas or line breaks inside quotes be flagged?</summary>

No. The scanner understands standard CSV quoting, so `"Smith, John"` stays one
field and a quoted field may even span multiple lines. Doubled quotes (`""`)
inside a quoted field are the standard escape and are accepted. Only *broken*
quoting is reported: a quote that never closes, a bare quote in an unquoted
field, or text after a closing quote.

</details>

<details>
<summary>How does delimiter auto-detection work?</summary>

With the delimiter set to auto-detect, the first non-blank, non-comment line is
scanned and the most frequent candidate separator outside quotes — comma, tab,
semicolon, or pipe — is chosen (comma wins ties). The report states which
delimiter was used, so a surprising result usually means the first line is not
what you thought. Pick the delimiter explicitly to override.

</details>

<details>
<summary>Which line number does a multi-line quoted field get?</summary>

Line numbers are physical lines of the pasted text. A row that contains a
quoted field with embedded newlines starts on one line and ends on a later
one; its issues are reported at the line where the row starts, and an
unclosed-quote error points at the line where the offending quote opens.

</details>

<details>
<summary>Can this tool fix my CSV?</summary>

No — it's deliberately report-only, so you can trust that the input is never
altered. Use the line numbers to fix the file in your editor, or run a fixing
tool afterwards: [csv-cleaner](/tools/csv-cleaner/) trims whitespace, drops
empty rows, and normalizes delimiters and line endings.

</details>
