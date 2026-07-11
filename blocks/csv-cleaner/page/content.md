## Clean a messy CSV in one pass

Paste a CSV and this tool tidies it locally in your browser — **nothing is
uploaded**. Every cleaning step is an independent toggle, so you can run just the
ones you need:

- **Trim whitespace** — strip leading/trailing spaces from every cell.
- **Remove duplicate rows** — drop rows identical to an earlier one (the first is
  kept).
- **Drop empty rows** — remove rows whose every cell is blank.
- **Fill empty cells** — replace blank cells with a value like `N/A` or `0`.
- **Normalize the delimiter** — re-emit with a comma, tab, semicolon, pipe, or any
  single character.
- **Normalize line endings** — output LF (`\n`) or CRLF (`\r\n`, for Windows/Excel).

A first-row **header** is kept and exempted from dedup, empty-row drop, and cell
fill, so your column names stay intact.

### Worked example

With **trim**, **remove duplicate rows**, and **drop empty rows** all on, this
input:

```
name , age
 Alice ,30
Bob,25
,,
Bob,25
Carol, 40
```

becomes:

```
name,age
Alice,30
Bob,25
Carol,40
```

The spaces around `name`, `Alice`, `40`, etc. are trimmed, the all-empty `,,` row
is dropped, and the repeated `Bob,25` row is removed — the header row stays.

### FAQ

<details>
<summary>Which duplicate row is kept?</summary>

The first occurrence in the file; later identical rows are dropped. Duplicate
matching happens **after** trimming and filling, so ` Bob ` and `Bob` count as the
same row when trim is on.

</details>

<details>
<summary>Is the cleaning case-sensitive?</summary>

Yes. Cells are compared exactly, so `Alice` and `alice` are treated as different
values and won't be deduped together. The tool doesn't change letter case — it only
trims surrounding whitespace when you enable **Trim**.

</details>

<details>
<summary>What counts as an "empty row" or "empty cell"?</summary>

A cell is empty when it's blank or only whitespace. An **empty row** is a row where
*every* cell is empty (for example a stray `,,,` line). Enable **Drop fully-empty
rows** to remove them, or **Fill** to replace individual empty cells with a value.

</details>

<details>
<summary>Can I change the delimiter, or read a tab/semicolon file?</summary>

Yes. Set the **input delimiter** to match your file (a single character, or
`comma`/`tab`/`semicolon`/`pipe`) and pick an **output delimiter** to re-emit with.
Leave the output on *Same as input* to keep the original separator.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The whole clean runs locally with WebAssembly; your CSV never leaves your
browser.

</details>

### Limits & edge cases

- Rows may have different column counts — cleaning is per-row and never pads to a
  fixed width. Missing trailing cells are treated as empty.
- Quoted fields are parsed as CSV, so a value like `"a, b"` stays one cell and is
  re-quoted only when the output needs it.
- Turn off **First row is a header** for files with no header row; then row 1 also
  participates in dedup, empty-row drop, and fill.
- Everything is processed in memory, so extremely large files are bounded by your
  browser's available memory.
