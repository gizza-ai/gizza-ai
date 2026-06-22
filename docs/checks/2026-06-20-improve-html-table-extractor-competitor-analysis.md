# html-table-extractor — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/html-table-extractor` — extract a `<table>` from pasted HTML
and output it as CSV or JSON. Chat + CLI + page (pure-text field inputs;
`scraper`/html5ever parser).

## What competitors do

- **Online "HTML table to CSV/JSON" sites** (convertcsv, tableconvert,
  htmltabletocsv) — paste HTML, get CSV/JSON. Strengths: convenient, some
  interactive. Weaknesses: the HTML is often **uploaded/processed server-side**,
  ads, and several mishandle quoting/commas or only grab the first table.
- **Browser devtools / copy-paste** — copy a rendered table into a spreadsheet;
  fragile and loses structure on complex pages.
- **Python (pandas `read_html`)** — robust but needs a runtime + code, and pulls
  in lxml/bs4.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`scraper`, html5ever — the
   same parser engine browsers use) compiled to wasm: the page parses in-browser
   and the CLI runs headless. The HTML never leaves the device.
2. **Real HTML parsing**, not regex. html5ever handles messy/unclosed markup,
   nested elements, and entities the way a browser does, so cell text is correct.
3. **CSV *and* JSON, with proper escaping.** CSV goes through the `csv` crate
   (RFC-4180 quoting of commas/quotes/newlines); JSON can be an **array of
   objects keyed by the header row** (`header=true`) or a plain array of arrays.
4. **Pick the table.** `table_index` (0-based) selects which table on a
   multi-table page, instead of always taking the first.
5. **Clean cells.** Whitespace (newlines, runs of spaces) is collapsed to single
   spaces so the output is tidy.
6. **Three surfaces** — chat tool, CLI, and a shareable page with query-param
   deep-links.

## Honest scope

- Reads cells in document order; **merged cells** (`colspan`/`rowspan`) are not
  expanded into repeated values.
- Nested tables: a `tr` selector under a table also matches inner-table rows
  (rare in practice).

## Build note (reusable)

`scraper` pulls `getrandom 0.3` (via `ahash`), which on the page's
**wasm32-unknown-unknown** target needs the `wasm_js` backend. Fixed in the web
crate with `getrandom = { features = ["wasm_js"] }` plus a scoped
`web/.cargo/config.toml` setting `--cfg getrandom_backend="wasm_js"`. The block's
wasm32-wasip1 target is unaffected (native WASI RNG). Pattern to reuse for any
future `scraper`/`ahash`-based page tool.

## Tests

8 core unit tests: first table → CSV; JSON array-of-objects (header=true) and
array-of-arrays (header=false); select the 2nd table by index; whitespace
collapse; CSV escaping of commas/quotes; error cases (empty / no tables / index
out of range); format parsing. Plus the block drift-guard schema test. CLI
verified for CSV and JSON. Playwright: CSV via fill and JSON via query-param
deep-link both pass.
