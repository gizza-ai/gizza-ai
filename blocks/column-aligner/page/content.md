## About this tool

Column Aligner turns pasted rows into neat fixed-width plain text columns. It is
for the common `column -t` workflow: take command output, logs, a quick report,
CSV-ish data, or a copied table and line up each field so the result stays easy
to read in a terminal, Markdown code block, issue comment, or plain text file.

The output is deliberately not a bordered table. It does not add rules, headers,
or Markdown pipes unless you ask for a separator; it only pads cells to the
widest value in each column. Padding uses Unicode display width, so East Asian
characters and emoji align correctly in a monospace font.

### Worked example

Input:

```
name age city
alice 30 Berlin
bo 7 SF
```

Default settings (`delimiter = whitespace`, `align = left`, `gap = 2`) produce:

```
name   age  city
alice  30   Berlin
bo     7    SF
```

For report-like data, switch **Alignment** to `auto` so columns whose non-empty
cells are all numeric are right-aligned while text columns stay left-aligned.
Use **Per-column alignment** for explicit control: `lrr` means left-align the
first column and right-align the next two; `l,r,c` is the same idea with words.

### Inputs

- **Input rows** — one record per line. Blank lines are preserved.
- **Input delimiter** — whitespace runs by default, or tab, comma, semicolon,
  colon, pipe, or a single space.
- **Alignment** — left, right, center, or auto numeric alignment.
- **Per-column alignment** — optional compact overrides such as `lrr`, `lr`, or
  `left,right,center`; `-` inherits the main alignment for that column.
- **Gap spaces** — 0 to 16 spaces between columns. Default 2.
- **Separator** — optional text drawn between columns, such as `|`. The gap is
  applied on both sides of the separator.
- **Trim fields** — remove padding around fields after splitting on literal
  delimiters. On by default.

### Limits and edge cases

- Maximum **20,000 lines** and **512 columns** per run. Errors name the limit and
  the value that exceeded it.
- Trailing spaces are never emitted. Ragged rows are padded only up to their last
  populated cell, so short rows do not grow invisible filler at the end.
- CRLF input is normalized to LF and a single trailing newline is not reproduced.
- `delimiter = whitespace` treats runs of spaces and tabs as one separator; use
  `delimiter = space` only when each individual space is meaningful.
- `align = auto` treats values like `-3.5`, `1,200`, `$19.99`, and `12%` as
  numeric. A header cell such as `qty` makes that column text unless you override
  it with `column_align`.

## FAQ

<details>
<summary>How is this different from a Markdown table formatter?</summary>

A Markdown table formatter adds pipes, a header separator row, and table syntax.
Column Aligner keeps the original rows as plain text and only inserts padding so
columns line up. Use it when the destination is a terminal, a code block, a log,
or any place where `column -t` output is the right shape.

</details>

<details>
<summary>Which delimiter should I choose?</summary>

Use **Whitespace runs** for normal command output and TSV-like text where spaces
or tabs separate fields. Use **Comma**, **Pipe**, **Semicolon**, or **Colon** for
literal-delimited rows. Literal delimiters can preserve empty fields between two
adjacent delimiters; whitespace mode cannot, because repeated spaces count as a
single separator.

</details>

<details>
<summary>Why are there no trailing spaces?</summary>

Trailing spaces are hard to see and often get stripped by editors, chat clients,
and version control reviews. This tool pads interior columns enough to keep the
visible layout aligned, then removes invisible trailing padding at the end of
each line. Ragged rows also stop at their final populated field.

</details>

<details>
<summary>What does auto alignment do?</summary>

Auto alignment inspects each column. If every non-empty cell in that column looks
numeric, the column is right-aligned; otherwise it is left-aligned. This works
well for simple reports with quantities, prices, percentages, and totals. If a
header row prevents numeric detection, set **Per-column alignment** explicitly,
for example `lrr`.

</details>

<details>
<summary>Does Unicode text line up correctly?</summary>

Yes. Padding uses display width rather than byte count or Unicode scalar count,
so CJK text that occupies two terminal columns is handled correctly. The final
look still depends on reading the result in a monospace font with ordinary
terminal-width glyphs.

</details>
