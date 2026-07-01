## About this tool

**CSV to table** turns CSV data into a ready-to-paste **Markdown** table (the
GitHub `| a | b |` style) or an **HTML** `<table>`.

- **Markdown:** a pipe table with a header separator row; `|` and newlines in cells
  are escaped so the table stays valid.
- **HTML:** a clean `<table>` with `<thead>`/`<tbody>`; cell text is HTML-escaped.
- Toggle whether the first row is a header (otherwise `Column N` headers are made),
  and pick the delimiter (`,` / tab / `;` / `|`).

### Privacy

Everything runs **in your browser** via WebAssembly — your CSV is never uploaded.
Also available from the [gizza CLI](/) and in chat.

### Common uses

- Drop a spreadsheet selection into a README, issue, or PR as a Markdown table.
- Generate an HTML table for a web page or email.

## FAQ

<details>
<summary>What if a cell contains a pipe character or a line break?</summary>

In Markdown output, `|` is escaped as `\|` and newlines inside a cell are
replaced with spaces, so the pipe table stays valid. In HTML output, `&`, `<`,
and `>` are escaped, so cell text can't inject markup.

</details>

<details>
<summary>My rows have different numbers of fields — will the table break?</summary>

No. The table is sized to the widest row, and shorter rows are padded with
empty cells, so a ragged CSV still produces a rectangular table instead of an
error.

</details>

<details>
<summary>Can I convert tab- or semicolon-separated data?</summary>

Yes — choose `tab`, `semicolon`, or `pipe` as the delimiter (comma is the
default), or supply any other single character. Pasting straight from a
spreadsheet usually means tab-separated.

</details>

<details>
<summary>What headers are used when my data has no header row?</summary>

Switch the header toggle off and the tool generates `Column 1`, `Column 2`, …
for you; every data row (including the first) then lands in the table body.

</details>
