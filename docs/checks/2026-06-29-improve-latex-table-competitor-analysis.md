# latex-table — competitor analysis & surface checks (2026-06-29)

**Tool:** `latex-table` — convert CSV/TSV data into a LaTeX `tabular` (optionally
wrapped in a centered `table` float). Pure-Rust (`csv`), runs on every backend
(chat block, CLI, in-browser page).

## Surface verification (all green)

| Surface | Check | Result |
| --- | --- | --- |
| Core unit tests | `cargo test --workspace` | ✅ 12 core + 1 drift-guard schema test pass |
| Chat block (wasm32-wasip1) | `wafer build` → validate `target/block.wasm` | ✅ OK, 336.8 KiB, instantiates |
| Page wasm (wasm32-unknown-unknown) | `wasm-pack build web --target web --release` | ✅ pkg built |
| CLI | `gizza tool latex-table data=… alignment=lr` and grid+caption+label+bold | ✅ correct LaTeX for booktabs/grid/table-float |
| Page generator | `cargo run -p generator -- .` | ✅ rendered `tools/latex-table/` |
| Page (Playwright) | `tool-page-latex-table.spec.ts` (6 tests: booktabs, grid, no-header, bold, caption+label, query-param deep-link) | ✅ 6 passed |

The chat schema is single-sourced from `descriptor()` and locked by the
`schema_json_matches_authored_chat_schema` drift test, so the LLM-facing schema,
the `manifest.json`, and the page inputs cannot silently diverge.

## Competitor landscape

Top CSV/TSV → LaTeX table generators users reach for:

1. **Tables Generator (tablesgenerator.com)** — the dominant free web tool. Grid
   spreadsheet UI, CSV/TSV import, booktabs toggle, per-column alignment, cell
   merge, borders, caption/label, "Generate" copy box.
2. **LaTeX Tables Editor (latexeditor.org / latex-tables.com)** — spreadsheet-style
   editor, import CSV, booktabs, alignment, export.
3. **Overleaf "Tables" help + ad-hoc snippets** — manual; no generator, but the
   reference standard for what compiles.
4. **tableconvert.com (CSV → LaTeX mode)** — multi-format converter (CSV, JSON,
   Markdown, HTML, LaTeX). Auto delimiter detect, header toggle, escaping.
5. **pandas `.to_latex()` / `csvtolatex` CLIs** — programmatic; booktabs via
   `\toprule`, column format string, escape toggle, caption/label.

## Capability diff (gizza vs competitors)

| Capability | Competitors | gizza latex-table |
| --- | --- | --- |
| CSV input | all | ✅ |
| TSV input | most | ✅ (`tab` / auto-detect) |
| Auto delimiter detect | tableconvert | ✅ (`auto`: tab > comma > semicolon) |
| Custom delimiter (`;`, `\|`, any char) | some | ✅ |
| booktabs style | all | ✅ default |
| Grid (bars + `\hline`) | all | ✅ |
| Plain (no rules) | some | ✅ |
| Per-column alignment (`lcr`) | all | ✅ + single-letter shorthand |
| Header row + separating rule | all | ✅ |
| Bold header (`\textbf`) | some | ✅ |
| LaTeX special-char escaping (& % $ # _ { } ~ ^ \\) | tableconvert/pandas | ✅ + toggle to keep raw LaTeX |
| Caption + label → `table` float | all | ✅ centered `\begin{table}[ht]` |
| Ragged rows padded to widest | varies | ✅ |
| Local / private (no upload) | varies | ✅ runs in browser & CLI, offline |

## In-model gaps closed / confirmed

All in-model capabilities the leaders offer for a *text-in → LaTeX-out* converter
are present: delimiter auto-detect + override, all three rule styles, per-column &
shorthand alignment, header handling, bold header, escaping toggle, and
caption/label float wrapping. Escaping is on by default (so pasted `%`/`&`/`_`
compile) with a documented escape-hatch for raw LaTeX cells — matching pandas and
beating the web tools that silently break on special chars.

## Out-of-model (intentionally not built)

- **Interactive spreadsheet grid / cell editing / cell merge (`\multicolumn`,
  `\multirow`)** — requires a stateful grid UI; gizza tools are stateless
  text-in/text-out. Out of scope for this converter surface.
- **Cell background colors / `\rowcolor` styling** — needs a visual editor and
  `xcolor`/`colortbl` preamble management; out of the pure-compute model.
- **Live LaTeX render preview** — would need a TeX engine in the browser; the page
  shows the source (the deliverable users paste), consistent with sibling tools.

No competitor copy, branding, or trademarks were used.
