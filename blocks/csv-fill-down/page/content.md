## CSV fill down

Fill empty cells in a CSV with the last non-empty value **above** them — the
spreadsheet "fill down" you use to unflatten grouped or merged-cell exports.
A cell counts as empty when it is blank or contains only whitespace. Switch
**Direction** to *up* to back-fill from the next value below instead. It runs
in your browser; nothing is uploaded.

### Worked example

Input (grouped export where the repeated region was left blank):

```
region,rep
West,Ann
,
,Bob
East,
```

Fill **down**, all columns → output:

```
region,rep
West,Ann
West,Ann
West,Bob
East,Bob
```

Each blank inherits the nearest value above it in its own column.

### Options

- **Columns to fill** — names (when there's a header) or 1-based indices, comma-
  separated (`region` or `1,3`). Blank fills every column.
- **Direction** — *down* carries the last value above into blanks below it; *up*
  carries the next value below into blanks above it.
- **First row is a header** — kept verbatim (never filled) and lets you name
  columns.
- **Delimiter** — comma, tab, semicolon, pipe, or any single character.

### Limits & edge cases

- A blank at the very top of a column (fill down) or bottom (fill up) has no value
  to carry, so it stays empty.
- A completely empty line (no delimiters at all) is dropped by the CSV parser; a
  row of empty *cells* (e.g. `,,`) is kept and filled.
- Values are copied exactly, including quotes and inner commas — the tool never
  reformats non-empty cells.

### FAQ

<details>
<summary>What counts as an "empty" cell?</summary>

Any cell that is blank or contains only whitespace. Whitespace-only cells are
trimmed and treated as empty, so a stray space still gets filled.

</details>

<details>
<summary>Can I fill only some columns?</summary>

Yes. Put the columns in **Columns to fill** as header names (`region,rep`) or
1-based indices (`1,3`), comma-separated. Any column you don't list keeps its
blanks. Leave the field empty to fill every column.

</details>

<details>
<summary>What's the difference between fill down and fill up?</summary>

**Down** copies the last non-empty value *above* a blank into it — the usual
spreadsheet fill-down for un-flattening grouped rows. **Up** copies the next
non-empty value *below* a blank into it, handy when a summary value sits at the
bottom of a group. Pick one with the **Direction** control.

</details>

<details>
<summary>Does this fill blanks with a fixed value or an average?</summary>

No — it only carries forward (or backward) a neighbouring cell's own value. To
drop a constant like `N/A` into every blank, use a CSV cleaner instead; average
imputation is a separate numeric task and isn't done here.

</details>

<details>
<summary>Is my data uploaded?</summary>

No — it's processed locally with WebAssembly. Nothing leaves your browser.

</details>
