## CSV filter

Keep only the rows of a CSV that match a condition. Write it as
`<column> <op> <value>` — for example `age > 28`, `city == NYC`, or
`name contains al`. It runs in your browser; nothing is uploaded.

### The condition language

- **Column** — a header name (when *first row is a header* is on) or a 1-based
  index.
- **Operators** — `==`, `!=`, `<`, `<=`, `>`, `>=`, and `contains`.
- **Comparison** — numeric when both the cell and the value are numbers (so
  `age > 9` works as you'd expect), otherwise a string comparison. `contains` is
  a case-insensitive substring match.
- Spaces are optional: `age>=30` works too.

### FAQ

<details>
<summary>Is my data uploaded?</summary>

No — it's processed locally with WebAssembly.

</details>

<details>
<summary>Can I combine conditions (AND/OR)?</summary>

Not yet — run the tool twice, or chain it
with the other CSV tools. (Multi-condition support is a planned addition.)

</details>

<details>
<summary>Does it work with semicolon- or tab-separated files?</summary>

Yes. Set the delimiter to `comma`, `tab`, `semicolon`, or `pipe` (or type any
single character). The filtered output is written back with the same
delimiter, and rows with differing field counts are tolerated.

</details>

<details>
<summary>What if my CSV has no header row?</summary>

Turn *first row is a header* off and refer to columns by **1-based index** —
e.g. `2 > 100` filters on the second column. With the header option on, the
header row itself is always kept in the output; note that `contains` needs
spaces around it (`name contains al`), while a bare `=` is accepted as
equality.

</details>
