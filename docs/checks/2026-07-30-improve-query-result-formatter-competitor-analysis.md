# query-result-formatter — competitor analysis (2026-07-30)

Function: render pasted database/query result rows as a shareable table. Inputs include JSON row arrays, CSV, TSV, and SQL shell tables; outputs are Markdown pipe tables or aligned ASCII grids. Pure string transformation; runs locally in browser, CLI, and chat.

## Competitors surveyed

| # | Tool | Input coverage | Output coverage | UX patterns | Notes |
|---|------|----------------|-----------------|-------------|-------|
| 1 | General online data table formatter | CSV, TSV, JSON, Markdown-ish tables | CSV, TSV, Markdown, JSON, HTML, ASCII | paste area, auto-detect, copy result | Broad converter rather than query-result focused |
| 2 | Browser data converter for JSON/CSV/TSV/SQL | JSON, CSV, TSV, SQL-shaped data | JSON, CSV, TSV, SQL-like forms | format selector, preview, download/copy | Strong multi-format table conversion baseline |
| 3 | SQL result conversion tool | pasted SQL query result text | JSON or CSV | paste SQL result, privacy/local copy | Focused on database-shell output, not Markdown/ASCII sharing |
| 4 | Table converter utilities | CSV, Markdown, HTML, JSON, SQL-style tables | CSV, Markdown, HTML, JSON, SQL | input/output selects, examples | Table-stakes conversion matrix; not optimized for chat/docs snippets |
| 5 | CSV/JSON/Markdown converter | CSV and JSON | Markdown, JSON, CSV | clean/normalize toggles, copy/export | Good Markdown-table baseline, less SQL-shell aware |

Paraphrased from public search results and tool pages; no competitor copy, branding language, or proprietary examples reused.

## Table-stakes → decision

| Capability | Decision |
|------------|----------|
| Paste JSON array of objects | **IN** — object keys become columns, unioned across rows. |
| Paste JSON arrays, a single object, or scalar arrays | **IN** — useful API/debug shapes; arrays can use first row as header. |
| Paste CSV | **IN** — common spreadsheet/export shape. |
| Paste TSV | **IN** — common `psql`, BigQuery, and spreadsheet-copy shape. |
| Paste SQL CLI result tables | **IN** — psql/MySQL/SQLite-style pipe tables and row-count footers are parsed. |
| Auto-detect input format | **IN** — JSON by leading bracket/brace, SQL rule rows, TSV tabs, otherwise CSV. |
| Markdown pipe-table output | **IN** — default output for READMEs, issues, PRs, and chat. |
| ASCII grid output | **IN** — for logs, plain-text comments, and places without Markdown rendering. |
| Copy/download result | **IN via generic page shell** — the generated tool page provides result copy behavior. |
| Header toggle | **IN** — CSV/TSV/array rows can synthesize `Column N` headers. |
| Alignment controls | **IN** — left, right, center. |
| Null/missing-value marker | **IN** — `null_text` fills JSON nulls and missing object keys, and empty SQL cells. |
| File upload / XLSX import | **OUT** — this block is pure pasted text; spreadsheet file parsing is covered by other file tools. |
| Database connection / query execution | **OUT** — out of model and unsafe for this local formatter; users paste already-produced rows. |
| HTML table rendering | **OUT** — not needed for docs/chat target; Markdown and ASCII cover the sharing use case. |

## UX / page controls shipped

- `data` textarea with a JSON example placeholder.
- `input_format` select with labels for Auto, JSON, CSV, TSV, and SQL CLI table.
- `format` select with Markdown and ASCII choices.
- `header` checkbox defaulting on, with a tested off path.
- `align` select for left/right/center.
- `null_text` text field for database-style `NULL` markers.
- Example chips for JSON rows, TSV to ASCII, CSV right-aligned, and psql output.

## Relationship to existing blocks

No existing block in `blocks/` specifically formats arbitrary query-result rows into Markdown or ASCII tables. Nearby CSV/JSON tools convert data formats or query CSV, but they do not accept the SQL shell table shape and do not target copy-paste-ready Markdown/ASCII table output in one tool.
