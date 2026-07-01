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
