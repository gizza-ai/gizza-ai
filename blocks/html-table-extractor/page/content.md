## Extract tables from HTML in your browser

Paste HTML that contains one or more `<table>` elements, pick which table and the
output format, and get clean **CSV** or **JSON**. Everything runs locally in your
browser — your HTML is never uploaded to a server.

### Options

- **Output format** — `csv` (default) for spreadsheets, or `json`.
- **Table number** — 0-based. `0` is the first table on the page; bump it to grab
  a later one.
- **First row is a header** — on by default. For **JSON** this emits an array of
  objects keyed by the header cells (`[{"Name":"Alice","Age":"30"}, …]`); turn it
  off to get a plain array of arrays. (CSV always includes every row.)

### Notes

- Cell text is whitespace-collapsed (newlines and runs of spaces become a single
  space), and CSV values are properly quoted/escaped.
- Cells are read in document order; merged cells (`colspan`/`rowspan`) are not
  expanded.
