## CSV formula eval

Add computed columns to a CSV using spreadsheet-style formulas. Reference your
existing columns by their header name — for example `total = price * qty` or
`margin = (price - cost) / price`. It runs in your browser; nothing is uploaded.

### Formulas

- Write `<column> = <expression>`. A new name **adds** a column; an existing name
  **replaces** it.
- Separate multiple formulas with `;` or newlines. They run left-to-right, so a
  later formula can use a column an earlier one created.
- Expressions support `+ - * / %`, `^` (power), parentheses, and functions like
  `sqrt`, `abs`, `min`, `max`, `round`, `floor`, `ceil`, `sin`, `ln`, …
- Column names used in expressions must be identifier-like (letters/digits/`_`,
  not starting with a digit). Cells that aren't numbers leave a referencing
  formula blank for that row.

### Example

`price,qty` + `total = price * qty` → adds a `total` column = price × qty.

### FAQ

<details>
<summary>Is my data uploaded?</summary>

No — it's processed locally with WebAssembly.

</details>

<details>
<summary>Does it work with semicolon- or tab-separated files?</summary>

Yes. The delimiter option accepts any single character, or the names
`comma`, `tab`, `semicolon` and `pipe`. The default is a comma, and the
output is written back with the same delimiter you chose.

</details>

<details>
<summary>What happens when a cell isn't a number?</summary>

Any row where a referenced cell fails to parse as a number gets a **blank**
result for that formula — the row itself is kept and all other rows still
compute normally. That's also why the first row must be a header: it's how
columns get the names your expressions refer to.

</details>

<details>
<summary>Can one formula build on another one's result?</summary>

Yes. Formulas run left-to-right, so `subtotal = price * qty; total =
subtotal * 1.2` works — the second formula sees the `subtotal` column the
first one just created. Re-using an existing header as the target replaces
that column in place instead of appending a new one.

</details>
