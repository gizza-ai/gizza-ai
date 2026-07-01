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
