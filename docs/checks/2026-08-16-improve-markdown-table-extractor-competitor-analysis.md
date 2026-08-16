# markdown-table-extractor — competitor analysis (2026-08-16)

Scan run **before** implementation, per `/create-next-tool` step 4. One WebSearch
("markdown table extractor convert markdown tables to CSV JSON online tool"), then the top 3
reachable competitors were skimmed. Everything below is **paraphrased** — no competitor copy,
branding, or trademarks are reproduced or reused anywhere in this tool.

## Scope check vs. existing gizza blocks (why this is not a duplicate)

`blocks/markdown-table-to-csv` already converts **one** pasted pipe table to CSV. Verified against
its `core/src/lib.rs`: it keeps *every* pipe-bearing line in the input and folds them into a single
rectangular table, so a document containing two tables is silently **merged into one broken CSV**.
It also has no JSON output, no table discovery, and no fenced-code-block awareness.

This tool is the *document-level* sibling — the Markdown analogue of the existing
`blocks/html-table-extractor` (which likewise ships `format = csv|json` + a table selector next to
the single-table converters). Its job is **finding** every GFM table in a whole document and
exporting each one. Built, not skiplisted.

## Competitors reviewed

| # | Competitor | What it is |
|---|---|---|
| 1 | tableconvert.com (Markdown → JSON / CSV / JSONLines family) | Multi-format table converter with a spreadsheet-style editor |
| 2 | textavia.com (Markdown table → CSV) | Single-purpose in-browser converter with an options panel |
| 3 | md-to.com (Markdown table → CSV) | Minimal zero-config paste-and-copy converter |

### 1. tableconvert.com

- Output shapes for JSON: array-of-objects, 2-D array, column array, keyed array.
- Minify toggle; indentation choice (2 / 4 / 8 spaces or tabs); custom root object name.
- JSON Lines is a separate destination in the same family.
- Editor-side extras: dedupe rows, drop empty rows, transpose, case conversion, regex
  find-and-replace, live preview.
- Accepts `.md` / `.markdown` uploads by drag-and-drop; documents a ~10 MB working ceiling.
- Local processing, no account.

### 2. textavia.com

- **Table number selector** — it detects every pipe table in the pasted text and lets you choose
  which one to export; the preview follows the selection. This is the closest competitor to our
  brief.
- Delimiter dropdown (comma, semicolon, …), include-header toggle, trim-cells toggle,
  quote-all-fields toggle, LF vs CRLF line endings.
- Input area with character / word / line counters; Copy, Download and Clear buttons.
- States that GFM pipe tables need the header + `| --- |` separator row.
- Guide has a simple worked example and a second one covering cells containing commas and quotes,
  plus troubleshooting for "no table found" and delimiter mistakes.
- No stated size limit; runs in-browser.

### 3. md-to.com

- Zero configuration: paste → CSV, with Copy and Download.
- Paste / Clear buttons, live Markdown preview.
- Claims correct escaping of quotes and commas.
- FAQ covers formatted text inside cells, Excel/Sheets compatibility, and privacy.
- No table selection, no delimiter/quoting/line-ending controls, no JSON.

## Table stakes → our decision

Every table stake below lands in the descriptor or in the out-of-model list. Nothing dropped
silently.

| Table stake | Seen at | Decision | Where |
|---|---|---|---|
| Detect **all** tables in a document, choose which to export | 2 | **In model** | `table` param — `all` (default), an index, or a list/range like `0,2-3` |
| CSV output with correct RFC-4180 escaping | 1, 2, 3 | **In model** | `format=csv` (default) |
| JSON output, objects keyed by header vs. plain arrays | 1 | **In model** | `format=json` + `header` |
| JSON Lines | 1 | **In model** | `format=jsonl` |
| Minify / indentation control for JSON | 1 | **In model** | `json_indent` (0 = minified, up to 8) |
| Delimiter choice (comma / semicolon / tab …) | 1, 2 | **In model** | `delimiter` (single char or `comma`/`tab`/`semicolon`/`pipe`/`space`) |
| Include-header toggle | 1, 2 | **In model** | `header` (default on) |
| Quote-all-fields toggle | 2 | **In model** | `quote` = `minimal` (default) / `all` |
| LF vs CRLF line endings | 2 | **In model** | `newline` = `lf` (default) / `crlf` |
| Trim cell whitespace toggle | 2 | **In model** | `trim` (default on) |
| Handling of formatted text inside cells | 1, 3 (FAQ) | **In model** | `strip_formatting` — off by default (lossless); on renders `**b**`, `` `c` ``, `[t](u)`, `<br>` as plain text |
| Table inventory / preview before exporting | 1, 2 (selector + preview) | **In model**, our own take | `format=list` returns an index of every table found: index, nearest preceding heading, source line, column names, alignments, row count |
| Copy / Download / Clear / Reset buttons | 1, 2, 3 | **In model**, already platform | The shared page runtime gives Copy result + Reset; `format = "text"` pages get a Download link |
| One-click presets | 1 (destination tabs) | **In model** | `[[example]]` chips on the page (CSV of all tables · JSON of one table · inventory) |
| Character / word / line counters on the input | 2 | **Considered, rejected** | Belongs to the shared page runtime, not a per-tool hack; adding a `cfg.slug` branch to the shared runtime is banned by the workspace fix-at-root-cause rule |
| Spreadsheet editor: dedupe, drop empty rows, transpose, case conversion, regex replace | 1 | **Out of model (listed, not built)** | A stateful grid editor is a different product; gizza already ships focused blocks for these (`csv-dedupe`, `csv-regex-replace`, `text-case-convert`) |
| Custom JSON root object name / keyed-array shape | 1 | **Considered, rejected** | Schema bloat for a shape trivially produced by a follow-up `json-transform-rules` run; the two common shapes (objects / arrays) are covered by `header` |
| `.md` file drag-and-drop upload | 1 | **Out of model here** | The page's file-source input is wired for media (`runtime = "ffmpeg"`); a pure text tool takes pasted text. Paste covers the case; noted, not built |
| Chrome/Firefox/Edge extension | 1 | **Out of model** | Browser-extension distribution is outside this repo |
| Excel / XLSX destination | 1 | **Out of model here** | Different tool; CSV with `newline=crlf` + a BOM-free UTF-8 payload imports cleanly, and gizza has `csv-to-xlsx`-class blocks |

## Deliberate behavioural choices

- **GFM-strict detection.** A table is a header row plus a delimiter row (`---`, `:--`, `--:`,
  `:-:`) with a matching cell count, then body rows until a blank/non-pipe line. Pipe lines inside
  fenced code blocks (``` and ~~~) are ignored — a competitor-visible failure mode, since docs are
  full of fenced snippets containing pipes.
- **GFM row/column reconciliation.** Body rows shorter than the header are padded with empty
  cells; cells beyond the header count are dropped, exactly as a Markdown renderer displays them.
  Documented in the page FAQ so it is never a surprise.
- **Headings as labels.** Each table carries the nearest preceding ATX heading, which makes a
  multi-table export self-describing (`labels` param, on by default for multi-table CSV).
  Competitors only offer a bare index.
