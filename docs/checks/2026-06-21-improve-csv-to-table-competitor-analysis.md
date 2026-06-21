# csv-to-table — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/csv-to-table` — convert CSV data into a Markdown or HTML table.
Pure-Rust (`csv`). Pure-text input → text output: chat + CLI + a page. Joins the
csv-* family with the "render to a table" output none of the others produce.

## What competitors do

- **Online "CSV to Markdown/HTML table" sites** — paste CSV, get a table. Common
  and handy, but the data is uploaded and they're ad-supported.
- **`csvlook` (csvkit) / pandas `to_markdown()`/`to_html()`** — local + correct, but
  need a Python/CLI environment.
- **Editor plugins** (Markdown table tools) — help format tables but don't ingest
  raw CSV in one step.

## How this tool competes / improves

1. **Runs locally + everywhere.** Pure-Rust compiled to wasm: chat, CLI, and an
   in-browser page. The CSV never leaves the device.
2. **Markdown *and* HTML.** GitHub-style pipe tables (with a header separator row)
   or a clean `<table>` with `<thead>`/`<tbody>`.
3. **Correct escaping.** Markdown cells escape `|`, backslashes and newlines so the
   table doesn't break; HTML cells escape `&`, `<`, `>`. A real CSV parser handles
   quoted fields containing the delimiter.
4. **Header-aware.** Use the first row as the header, or auto-generate `Column N`
   headers; works with `,` / tab / `;` / `|`.
5. **Agent-friendly.** One call to turn a spreadsheet selection into a table for a
   README/PR/issue (Markdown) or a web page (HTML).

## Honest scope

- **CSV → table** rendering only — not the reverse (parsing a Markdown/HTML table
  back to CSV), and no column alignment markers or styling.

## Tests

7 core unit tests: a correct Markdown table; Markdown escapes `|`; an HTML table
with `<thead>`/`<tbody>` and escaped cells; no-header synthesises `Column N`; tab
delimiter; and errors (empty input, bad format). Plus the block drift-guard schema
test. **CLI verified** end-to-end for both formats. **Page** verified with
Playwright (Markdown + HTML). `wafer build` instantiates the chat block (327 KiB).
