## About this tool

**Extract Numbers From Text** scans any block of text — prose, a log file, a
pasted spreadsheet column, an invoice — and pulls out every number into a clean
list. It recognises:

- **Integers and decimals** — `42`, `3.14`, `.5`
- **Scientific notation** — `6.022e23`, `1.6E-19`
- **Signed numbers** — `-7`, `+5` (a hyphen glued to a preceding digit, as in a
  date like `2024-01-15`, is treated as a separator, not a minus sign)
- **Thousands-separated numbers** — `1,000`, `1,299.99`, `1,000,000`

Then it lets you shape the result:

- **Keep** — return *all* numbers, *only integers* (no decimal point), or *only
  decimals*.
- **Remove duplicate values** — `1,000` and `1000` and `+5`/`5` collapse to one
  (first-seen wins).
- **Sort order** — leave the numbers in the order they appear, or sort ascending
  or descending by value.
- **Join output with** — put one number per line, or separate them with a comma,
  space, tab, or semicolon (handy for pasting into a spreadsheet or CSV).
- **Append summary** — add a block with the **count, sum, min, max, and
  average** of the extracted numbers.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Handy for

- Totalling the amounts scattered through an invoice, receipt, or email.
- Grabbing every metric out of a log line or monitoring dump.
- Turning a paragraph of figures into a clean comma-separated list for a
  spreadsheet.

### Example (command line)

The same engine is available through the `gizza` CLI:

```
gizza tool extract-numbers-from-text text='Order 42 shipped for $1,299.99; weight 3.5kg' stats=true
```

returns the joined list (`42`, `1,299.99`, `3.5`) plus a summary with the count,
sum, min, max, and average.

### Limitations

- Numbers are matched by their **textual form**, so spelled-out numbers
  (`"forty-two"`), Roman numerals, and fractions written as `3/4` are **not**
  extracted.
- A `%`, `$`, `kg`, or other unit next to a number is **not** captured — only the
  numeric part is returned.
- De-duplication compares a **normalized token** (commas removed, leading `+`
  removed, case-normalized), so `1,000` and `1000` collapse to one value. Tokens
  with different notation, such as `5`, `5e0`, and `5.0`, are kept separate.

## FAQ

<details>
<summary>Does it understand numbers with commas like 1,000,000?</summary>

Yes. Thousands-separated numbers such as `1,000`, `1,299.99`, and `1,000,000`
are recognised as single values, and the commas are kept in the extracted token
exactly as they appeared. When you turn on **Remove duplicate values** or
**Append summary**, the commas are stripped internally so `1,000` counts as the
same value as `1000` and totals add up correctly.

</details>

<details>
<summary>How are dates and negative numbers told apart?</summary>

A `-` or `+` only counts as a sign when the character before it is **not** a
letter or digit. So in `-7` and `+5` the sign is kept, but in a date like
`2024-01-15` the hyphens are read as separators and you get `2024`, `01`, and
`15` — not negatives. The same rule means `a-5` and `5-3` yield plain positive
numbers.

</details>

<details>
<summary>Can I get only whole numbers or only decimals?</summary>

Yes — use the **Keep** option. *Integers only* returns tokens with no decimal
point (this includes scientific notation without a dot, like `5e2`), *Decimals
only* returns tokens that contain a decimal point (like `2.5` or `4.0`), and
*All numbers* returns everything.

</details>

<details>
<summary>What does the summary block include?</summary>

With **Append summary** enabled, the output ends with the **count** of numbers
returned, plus the **sum**, **min**, **max**, and **average** of their values.
Whole-number results are shown without a trailing `.0`, and the statistics are
computed after any filtering and de-duplication you applied.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The extraction runs entirely in your browser through WebAssembly — the text
you paste never leaves your device, so it is safe to use on logs, invoices, or
other private content.

</details>
