# markdown-table-to-csv — competitor analysis (2026-07-24)

Scan of the top public "Markdown table → CSV" converters, done before finishing the
tool's copy/params. All notes are **paraphrased** — no competitor copy, branding, or
trademarks were copied.

## Competitors scanned

| # | Tool | URL |
| - | ---- | --- |
| 1 | TableConvert (Markdown → CSV) | tableconvert.com/markdown-to-csv |
| 2 | Textavia (Markdown Table → CSV) | textavia.com/tools/markdown-table-to-csv |
| 3 | TableFormatConverter | tableformatconverter.com/markdown-to-csv |
| 4 | Aspose (MD → CSV) | products.aspose.app/pdf/conversion/md-to-csv |
| 5 | MarkdownToExcel | markdowntoexcel.com/markdown-to-csv |

## Feature / param matrix (paraphrased)

| Capability | Competitors that ship it | In our model? | Status |
| ---------- | ------------------------ | ------------- | ------ |
| Output delimiter choice (comma / tab / semicolon / pipe / …) | TableConvert, Textavia | in-model | HAVE — `delimiter` (any single char + comma/tab/semicolon/pipe/space aliases) |
| Quote-all / "double-quote wrap" toggle | TableConvert, Textavia | in-model | HAVE — `quote` = minimal / all |
| RFC-4180 minimal quoting (auto-quote cells with delimiter/quote/newline) | all | in-model | HAVE — default `minimal` |
| Include/drop header row | Textavia | in-model | HAVE — `header` bool (default keep) |
| Strip Markdown alignment/separator row (`:---:`) | all (implicit) | in-model | HAVE — separator rows always stripped |
| Trim cell padding | TableConvert, Textavia | in-model | HAVE — always trimmed |
| Ignore surrounding prose (paste whole message) | — (nice-to-have) | in-model | HAVE — non-pipe lines ignored |
| Ragged/short rows padded to widest | — | in-model | HAVE — padded |
| **UTF-8 BOM** (helps Excel detect encoding) | TableConvert | in-model | **ADDED** — `bom` bool (default off) |
| **LF / CRLF line endings** | Textavia | in-model | **ADDED** — `newline` = lf / crlf (default lf) |
| Copy to clipboard / download result | all | in-model | HAVE — generator gives Copy + Download automatically |
| Delimiter/format presets (chips) | TableConvert | in-model | ADDED — `[[example]]` preset chips (CSV / TSV / semicolon / quote-all) |
| Table-number selection when a doc has multiple tables | Textavia | partial | OUT — we treat all pipe lines as one table; noted as a limit on the page |
| In-editor cleanup (dedupe, transpose, case, regex replace, delete empty rows) | TableConvert | out-of-model here | OUT — these are separate gizza tools (csv-dedupe, csv-transpose, …); don't bloat this one |
| Row prefix/suffix, custom row delimiter | TableConvert | out-of-model | OUT — that's a general table-format builder, not a CSV export |
| Upload a `.md` file | TableConvert, Aspose | in-model-ish | OUT — paste covers the flow; file-picker is a UX add, not a capability gap |

## Decisions

- Close the two real capability gaps that fit a browser-local pure tool: **UTF-8 BOM**
  and **CRLF line endings**. Both are single-boolean/enum additions with clear Excel/
  Windows use-cases and appear on real competitors.
- Add `[[example]]` preset chips (CSV, TSV, semicolon, quote-all) — the declarative
  answer to competitors' delimiter presets.
- Reject the in-editor cleanup suite and multi-table selection: those are either other
  gizza tools or would bloat a focused converter. Multi-table behavior (all pipe lines
  merged into one table) is stated as a limit on the page instead.

All in-model table-stakes are represented in the descriptor. Out-of-model items are
listed above, not silently dropped.
