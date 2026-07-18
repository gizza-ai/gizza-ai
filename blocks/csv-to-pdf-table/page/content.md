## CSV to PDF table

Turn a small or medium CSV-style table into a printable PDF without uploading the
data. Paste comma, tab, semicolon, or pipe-delimited rows, choose the page layout,
and download a PDF data URL that contains a formatted table.

The renderer repeats the header row on every page, sizes columns to the page,
right-aligns numeric columns, truncates very long cells with an ellipsis, and can
draw grid lines plus light row banding for readability.

### Worked example

Input:

```
name,age,team
Alice,30,East
Bob,25,West
Carol,28,East
```

With title `Team roster`, page size `letter`, orientation `portrait`, and font size
`10`, the output is a `data:application/pdf;base64,...` URL. Save or open that URL to
get a PDF headed “Team roster” with a bold `name / age / team` table header and one
row for each person.

### Limits & edge cases

- Input is parsed as delimited text with flexible row lengths; short rows get blank
  trailing cells so the table remains rectangular.
- Headers are optional. When enabled, the first row is bold and repeated on every
  page; when disabled, every row is treated as data.
- Fonts use built-in Helvetica/Helvetica-Bold. Latin-1 characters are preserved;
  characters outside Latin-1 are replaced with `?`.
- Columns shrink proportionally when the table is wider than the page, and long
  cell text is ellipsized rather than wrapped.
- Very large tables produce large PDFs; for huge datasets, sample or filter the CSV
  first.

### FAQ

<details>
<summary>Does this upload my spreadsheet or CSV?</summary>

No. The page runs the PDF writer locally in WebAssembly. The pasted rows are turned
into a PDF data URL in your browser.

</details>

<details>
<summary>Can I use tab-separated or pipe-separated data?</summary>

Yes. Set the delimiter control to tab, semicolon, or pipe. The parser uses that
same delimiter for every row before drawing the table.

</details>

<details>
<summary>What happens if my table has too many columns?</summary>

Columns are measured and then scaled down to fit the page. For wide data, switch to
landscape orientation, lower the font size, or remove columns before generating the
PDF.

</details>

<details>
<summary>Will the header appear after a page break?</summary>

Yes. If “first row is a header” is enabled, that row is drawn in bold at the top of
each page so multi-page tables stay readable.

</details>
