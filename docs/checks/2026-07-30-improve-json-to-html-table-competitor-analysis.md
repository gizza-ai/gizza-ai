# json-to-html-table — competitor analysis (2026-07-30)

Function: convert JSON arrays or objects into copy-ready HTML or Markdown tables. Pure string transformation; runs locally in browser, CLI, and chat.

## Competitors surveyed

| # | Tool | HTML table | Markdown table | Nested values | UX patterns | Notes |
|---|------|------------|----------------|---------------|-------------|-------|
| 1 | JSON-to-HTML-table converter pages | yes | no | usually stringify or expand | paste JSON, convert button, copy result | Often emphasizes styled/sortable tables, beyond this pure markup block |
| 2 | Styled JSON table generators | yes | no | stringify or nested rendering | class/style controls, preview | Useful baseline for captions/classes and HTML escaping |
| 3 | JSON toolkit converters | yes | sometimes separate tools | stringify nested values | input/output panes, examples | Commonly suggests flattening nested JSON first |
| 4 | JSON-to-Markdown-table tools | no | yes | stringify nested values | paste JSON, copy Markdown | Documentation/README-focused baseline |
| 5 | Table conversion suites | yes | yes | varies | format selects, example chips, copy/download | Broad table converter rather than JSON-specific semantics |

Paraphrased from public search results and tool pages; no competitor copy or proprietary examples reused.

## Table-stakes → decision

| Capability | Decision |
|------------|----------|
| JSON array of objects → rows | **IN** — union object keys in first-seen order. |
| Single JSON object | **IN** — key/value table for config snippets and API objects. |
| JSON array of arrays | **IN** — first row can be header, or synthesize `Column N`. |
| JSON array of scalars | **IN** — single-column table. |
| HTML `<table>` output | **IN** — default, semantic `<thead>`/`<tbody>` markup. |
| Markdown table output | **IN** — copy-ready for docs and issues. |
| HTML entity escaping | **IN** — cells, headers, captions, and classes are escaped. |
| Markdown escaping | **IN** — pipes and newlines in cells are escaped/collapsed. |
| Nested object handling | **IN** — compact JSON, nested HTML tables, or flattened dotted keys. |
| Missing/null cell marker | **IN** — `null_text` controls missing object keys and JSON nulls. |
| Caption and CSS classes | **IN** — optional HTML polish without imposing styles. |
| Pretty vs compact HTML | **IN** — indented for reading, single-line for embeds/tests. |
| Sort/filter interactive table | **OUT** — generated runtime behavior is site/app-specific; this block emits static markup. |
| Inline CSS/responsive theme generation | **OUT** — styling belongs to the consuming document/site; this block stays generic. |
| File upload | **OUT** — this tool is pasted JSON; file-oriented table tools cover uploads. |

## UX / page controls shipped

- JSON textarea with an array-of-objects placeholder.
- Output format select: HTML table or Markdown table.
- Nested-value select: compact JSON, nested HTML tables, or flattened object keys.
- Header checkbox for array-of-arrays / scalar arrays.
- Null-text, caption, table-class, and pretty-print controls.
- Example chips for row objects, Markdown output, flattened nested objects, and compact HTML with caption/class.

## Relationship to existing blocks

`query-result-formatter` formats SQL/CSV/TSV/JSON query rows as Markdown or ASCII. `json-to-html-table` is not a semantic duplicate because it targets JSON-only input, emits semantic HTML tables, supports captions/classes/pretty HTML, and offers nested-object strategies including nested HTML tables and flattened dotted columns.
