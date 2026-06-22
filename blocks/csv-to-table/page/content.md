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
