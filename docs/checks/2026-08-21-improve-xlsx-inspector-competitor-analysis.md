# xlsx-inspector — competitor analysis (2026-08-21)

Scan run **before** implementing, per `create-next-tool` step 4. Everything below is a
paraphrase of publicly documented feature lists; **no competitor copy, branding, or
trademark text was copied into the block, its manifest, or its descriptor.**

## Scope of the backlog row

> `xlsx-inspector` — "Opens an .xlsx workbook and reports each sheet's name, used
> range/dimensions, named ranges, and formula vs value cell counts." (type hint: pure)

## Dup check (why this is not an existing block)

| Existing block | What it does | Overlap |
| --- | --- | --- |
| `xlsx-to-csv` | Emits ONE chosen sheet as RFC-4180 CSV | Converts data; reports no structure |
| `xlsx-sheet-diff` | Cell-by-cell diff of TWO sheets in one workbook | Comparison, not an overview |
| `spreadsheet-formula-audit` | Finds `#REF!`, cached error values, dangling sheet refs, external links, dependency cycles | Formula **breakage**, not workbook **shape** — no dimensions, no per-sheet counts, no named-range listing, no object inventory |
| `csv-to-xlsx` | Writes a workbook | Opposite direction |

No block reports workbook structure (sheet inventory + used range + formula/value split +
defined names + object counts). Confirmed by reading each block's `core/src/lib.rs`.

## Competitors reviewed

1. **Excel Workbook Statistics** (Microsoft 365, Review ▸ Proofing, `Ctrl+Shift+G`) — the
   direct feature analogue. Documented metrics:
   - *Current sheet:* End of sheet / Last cell, Cells with Data, Tables, PivotTables,
     Formulas, Charts, Images, Form Controls, Objects, Comments, Notes.
   - *Workbook:* Sheets, Cells with Data, Tables, PivotTables, Formulas, Charts,
     External Connections, Macros.
2. **Excel Spreadsheet Inquire — Workbook Analysis report** (COM add-in). Report groups:
   Summary, Workbook, Formulas, Cells, Ranges, Warnings. Documented detections include
   total formula counts, hidden and very-hidden sheets, linked workbooks, external data
   connections, array formulas and error formulas.
3. **ASAP Utilities — "List All Named Ranges in Workbook"** (Excel add-in). Per-name
   columns: name, the sheet + cell reference it refers to, scope (workbook-level vs
   worksheet-level), comment/description, visibility (visible vs hidden name).
4. **Browser workbook viewers** (ExcelTool.io Excel Viewer, ConvertICO Excel File Viewer,
   ChatDB Excel Viewer). Shared table stakes: open `.xlsx`/`.xls`/`.xlsm`/`.xlsb` with no
   signup, switch between all worksheets, show each sheet's dimensions, and process the
   file locally rather than uploading it.
5. **Workbook Statistics for Google Sheets** (Workspace Marketplace add-on). Reports, per
   sheet and for the whole workbook: sheet names, cells with data, named ranges, pivot
   tables, cells with formulas, charts.

## Table stakes → decision

| Capability | Seen in | Verdict | Where it landed |
| --- | --- | --- | --- |
| Sheet inventory with names + order | 1,2,3,4,5 | **in-model** | `sheets[].index/name` |
| Used range / "last cell" in A1 notation | 1,4 | **in-model** | `used_range`, `last_cell` |
| Row × column dimensions of the used range | 1,4 | **in-model** | `rows`, `columns` |
| Cells with data (per sheet + workbook) | 1,2,5 | **in-model** | `non_empty_cells` + totals |
| Formula cell count | 1,2,5 | **in-model** | `formula_cells` (from stored `<f>` text) |
| Formula **vs value** split | backlog row | **in-model** | `value_cells` = non-empty ∧ not-formula |
| Cell type breakdown (text/number/date/bool/error) | 2 (Cells group) | **in-model** | `cell_types` |
| Error-value cells, by error literal | 2 (Warnings) | **in-model** | `error_cells` + `error_kinds` |
| Hidden and very-hidden sheets flagged | 2 | **in-model** | `visibility` + `include_hidden` |
| Non-worksheet tabs (chart/macro/dialog sheets) | 2 | **in-model** | `type` |
| Named ranges: name + refers-to reference | 1,3,5 | **in-model** | `named_ranges[]` |
| Named ranges: broken (`#REF!`) flagged | 2,3 | **in-model** | `named_ranges[].broken` |
| Count of Tables | 1,2,5 | **in-model** (OPC part scan) | `objects.tables` |
| Count of PivotTables | 1,2,5 | **in-model** (OPC part scan) | `objects.pivot_tables` |
| Count of Charts | 1,2,5 | **in-model** (OPC part scan) | `objects.charts` |
| Count of Images / media | 1 | **in-model** (OPC part scan) | `objects.images` |
| Count of Comments (incl. threaded) | 1 | **in-model** (OPC part scan) | `objects.comments` |
| External links / connections | 1,2 | **in-model** (OPC part scan) | `objects.external_links`, `objects.data_connections` |
| Macros present (VBA project) | 1,2 | **in-model** (OPC part scan) | `objects.has_macros` |
| Legacy `.xls` / OpenDocument `.ods` input | 4 | **in-model** | `open_workbook_auto_from_rs` |
| Structured output for scripting (JSON/CSV) | — (differentiator) | **in-model** | `format = json|csv` |
| Single-sheet drill-down | 1 (current sheet) | **in-model** | `sheet` param |

### Out-of-model (listed, deliberately NOT built)

- **Named-range scope (workbook-level vs worksheet-level) and name comments/visibility**
  (ASAP Utilities). `calamine` exposes defined names as a flat `(name, formula)` list with
  no scope, comment, or hidden flag, so these cannot be reported truthfully. The page/report
  states this limit; the referenced sheet is still derived from the formula text.
- **Per-sheet attribution of charts/images/pivot tables/comments.** The OPC part list gives
  reliable workbook-level counts; mapping each part to its sheet needs full
  `_rels`/drawing-relationship resolution, which is a separate parser. Reported at workbook
  level, exactly as Excel's own workbook-level statistics does.
- **Form Controls / Objects / Notes counts** (Workbook Statistics current-sheet rows). These
  live inside per-sheet drawing and legacy VML parts that would need the same relationship
  walk; not exposed as countable top-level parts.
- **Merged-cell regions.** `calamine` only exposes merged regions on the concrete `Xlsx`
  reader, not through the format-agnostic `Sheets` enum used here.
- **Array-formula and dynamic-array/spill detection** (Inquire). The stored formula text
  does not distinguish a CSE array formula from a scalar one without parsing the
  `<f t="array">` attribute, which `calamine` does not surface.
- **Object counts for `.xls` and `.ods`.** Those are CFB / ODF containers, not OPC ZIPs, so
  the part scan does not apply; the report says so explicitly rather than printing zeros.
- **Formula recalculation** (values are whatever the writing application cached). Same
  documented limit as `spreadsheet-formula-audit`; a recalculation engine is out-of-model
  per the `xlsx-recalculate` skiplist entry.

### UX / control patterns adopted

Competitor viewers are all "pick a sheet, see its stats"; Workbook Statistics is
"whole workbook at a glance". This block does both: default = whole-workbook report,
`sheet=<name|index>` = drill-down. `include_hidden`, `include_named_ranges`,
`include_object_counts` and `max_named_ranges` keep the report proportionate, and
`format=table|json|csv` matches the sibling `spreadsheet-formula-audit` / `xlsx-sheet-diff`
renderer trio so the three workbook tools compose in one pipeline.

No page surface: the input is binary workbook bytes, which the tool-page generator's pure
runtime cannot accept — same chat + CLI shape as `xlsx-to-csv`, `xlsx-sheet-diff` and
`spreadsheet-formula-audit`.

## Sources

- https://support.microsoft.com/en-us/office/check-workbook-statistics-afa12d4b-9584-4826-99a8-33228467e006
- https://support.microsoft.com/en-us/office/analyze-a-workbook-with-spreadsheet-inquire-5991e8fa-f1c1-401a-ae3f-469384ae3e3b
- https://www.asap-utilities.com/asap-utilities-excel-tools-tip.php?tip=175&utilities=42&lang=en_us
- https://www.exceltool.io/excel-viewer
- https://convertico.com/excel-file-viewer/
- https://www.chatdb.ai/tools/excel-viewer
- https://workspace.google.com/marketplace/app/workbook_statistics/1062814409654
