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

## FAQ

<details>
<summary>The page has several tables — how do I pick the right one?</summary>

Use **Table number**: it's 0-based, so `0` is the first `<table>` in the HTML,
`1` the second, and so on. If the index is past the end you get an explicit
error telling you how many tables were actually found, so you can adjust rather
than silently getting the wrong data.

</details>

<details>
<summary>How are merged cells (colspan/rowspan) handled?</summary>

They are **not expanded**: a cell that spans three columns comes out as a single
value, so rows under a merged header can look shifted. If your source table uses
heavy merging, expect to realign those columns after export — the extractor
reads cells strictly in document order.

</details>

<details>
<summary>What does the header toggle change in JSON output?</summary>

With the toggle on (the default), the first row becomes the keys and you get an
array of objects — `[{"Name":"Alice","Age":"30"}, …]`. Off, you get a plain
array of arrays including the first row. CSV output always contains every row
either way.

</details>

<details>
<summary>Do I need a full HTML page, or just the table snippet?</summary>

Either works — paste a whole page source or just the `<table>…</table>`
fragment. Cell text is extracted with whitespace collapsed, and CSV values are
quoted and escaped correctly when they contain commas or quotes. Nothing is
uploaded; parsing happens in your browser.

</details>
