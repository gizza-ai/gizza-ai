# spreadsheet-formula-audit competitor analysis — 2026-08-06

## Sources scanned

- Spreadsheet auditing features in Microsoft Excel (Error Checking, Trace Dependents/Precedents, Circular References indicator).
- LibreOffice Calc formula auditing / Detective tooling.
- Online workbook repair and formula-audit utilities that report broken references and formula errors.

## Table-stakes capabilities

| Capability / UX pattern | Seen in competitors | Model fit | Decision |
| --- | --- | --- | --- |
| Accept an Excel/LibreOffice workbook | Formula-audit tools operate on workbook files, not pasted CSV | In model (chat/CLI only) | Use `Input::Document` with `url`/`ref`, matching existing spreadsheet blocks; no browser page because binary workbook upload is not a generated text page surface. |
| Report formulas containing `#REF!` | Excel and Calc surface deleted-cell references | In model | Scan stored formula text for `#REF!`. |
| Report cached formula error values | Competitors list cells with `#DIV/0!`, `#VALUE!`, `#NAME?`, `#N/A`, etc. | In model | Read cached cell values via calamine and report typed/string Excel error cells. |
| Detect circular references | Spreadsheet auditors highlight circular dependency chains | In model, bounded | Parse literal A1 references into a dependency graph and report cycles among formula cells. |
| Broken sheet references | Formula auditors flag missing/renamed sheets | In model | Parse sheet-qualified references and report names absent from the workbook. |
| External workbook links | Auditors call out links that cannot be validated locally | In model | Report `[Book.xlsx]` references as external links. |
| Trace arrows / interactive graph navigation | Excel/Calc UIs show clickable arrows between cells | Out of model | Return table/JSON/CSV reports; interactive workbook UI is outside a CLI/chat block. |
| Full recalculation engine with volatile functions | Desktop spreadsheets can recalculate formulas | Out of model | This tool reads stored formulas and cached values only; `INDIRECT`, `OFFSET`, structured references, and defined names are documented limits. |

## Defaults chosen

- `check_cycles=true`, `check_error_values=true`, `include_hidden=true`: comprehensive audit by default.
- `max_findings=200`: enough for triage while bounding LLM output.
- `format=table`: readable default; JSON and CSV are available for automation.

## Verification expectations

- Unit tests generate in-memory `.xlsx` fixtures with rust_xlsxwriter and cover `#REF!`, cached errors, missing sheets, external links, circular references, clean workbooks, output formats, clipping, and parser edge cases.
- CLI verification should use a generated local workbook served via `file://` only if the CLI source resolver supports it; otherwise use a small HTTP fixture URL or attachment ref in integration contexts.
- No page test is required because this is a binary document-input block with no generated page.
