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
