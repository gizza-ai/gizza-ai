## About this tool

**Markdown Table Formatter** takes a messy GitHub-flavored Markdown table and
re-renders it with **even column widths** and a tidy delimiter row, so it's easy
to read in a plain-text editor and diffs stay clean. Paste a whole document — any
text outside tables is passed through untouched, so you can reformat every table
in one pass.

### What it does

- **Even columns** — each column is padded to the width of its widest cell, so the
  `|` separators line up perfectly in a monospace font.
- **Normalized delimiter row** — the `|---|---|` row is rebuilt to match the new
  column widths and carry the alignment markers.
- **Alignment preserved or forced** — `:---` (left), `---:` (right) and `:---:`
  (center) markers are read from the source and re-emitted. Pick **keep** to
  preserve them, or **left / center / right** to force every column.
- **Ragged rows fixed** — a row with too few cells is padded out to the full
  column count instead of breaking the grid.
- **Unicode-aware** — East-Asian wide characters (CJK, fullwidth) are counted at
  their true display width, so mixed-script tables still line up. Escaped `\|`
  pipes stay inside their cell.
- **Code blocks respected** — a `|---|` line inside a fenced code block
  (` ``` ` or `~~~`) is left exactly as written, so example tables in your docs
  aren't reformatted.

### Alignment modes

- **keep** — preserve each column's alignment from the source delimiter row.
- **left** — force every column to left-align (`:---`).
- **center** — force every column to center (`:---:`).
- **right** — force every column to right-align (`---:`).

### Style

- **pretty** — pad every column to its widest cell so the grid lines up in a
  monospace font (the most readable layout).
- **compact** — single-space padding with no width alignment, for the smallest,
  most diff-friendly output.

### Handy for

- Tidying tables in a README or docs file before committing.
- Cleaning up tables pasted from a spreadsheet or another tool.
- Keeping Markdown diffs small and reviewable by enforcing one consistent layout.

Everything runs **locally in your browser** via WebAssembly — your Markdown is
never uploaded.

## FAQ

<details>
<summary>Can I paste a whole README, or only the table itself?</summary>

Paste the whole document. Only pipe tables (a header row, a `|---|` delimiter
row, and body rows) are reformatted — every other line passes through
byte-for-byte, and tables shown inside fenced code blocks (``` or ~~~) are
deliberately left alone so your examples stay as written.

</details>

<details>
<summary>Why do my columns still look crooked with CJK text or emoji?</summary>

They shouldn't here: cell widths are measured in display columns, counting
East-Asian wide and fullwidth characters (and most emoji) as 2 and
zero-width/combining marks as 0. If a table looks misaligned after
formatting, check that your editor is using a monospace font — the padding is
computed for monospace rendering.

</details>

<details>
<summary>When should I pick compact instead of pretty?</summary>

**pretty** pads every column to its widest cell — the most readable in an
editor. **compact** uses a single space of padding and no width alignment,
which produces the smallest output and the least diff churn when cells change
length. Both styles normalize the delimiter row and alignment markers.

</details>

<details>
<summary>How do I put a literal pipe character inside a cell?</summary>

Escape it as `\|`, exactly as GFM requires. The formatter honors the escape —
it won't split the cell there, and the backslash is preserved in the output.

</details>
