# csv-to-pdf-table — competitor analysis (2026-07-18)

Function: render a CSV file as a formatted, paginated table inside a PDF document.

## Competitors skimmed (top 3 of the live search)

1. **TableConvert — CSV to PDF** (tableconvert.com/csv-to-pdf). Paste or upload CSV,
   preview it in a table editor, then export a clean PDF. Emphasis: delimiter handling,
   a live table preview, header row, copy/download.
2. **CoolUtils — CSV to PDF** (coolutils.com/online/CSV-to-PDF). Free browser converter
   that stresses *automatic column widths* and preserving the row/column structure in a
   tabular PDF; page-size choice.
3. **i2PDF — Table to PDF / CSV to PDF** (i2pdf.com/table-to-pdf). Upload a CSV or build a
   table by hand, choose the delimiter, and export a "good-looking" table PDF entirely in
   the browser (nothing uploaded). Emphasis: delimiter options, page layout, clean styling.

(Secondary references seen in the same result set: CSVTool, csvtotable.com, ihatepdf,
csvtosheets — all reinforce the same table-stakes set below. Paraphrased only; no
competitor copy, branding, or trademarks reproduced.)

## Table-stakes features observed → in-model / out-of-model

| Feature | Competitors | Our decision | Param / note |
| --- | --- | --- | --- |
| Delimiter (comma/semicolon/tab/pipe) | all | **in-model** | `delimiter` (single char or `comma`/`tab`/`semicolon`/`pipe`); default comma |
| First row = header, bold, repeated per page | all | **in-model** | `header` (bool, default true); header row is bold + repeated on every page |
| Auto column widths | CoolUtils, CSVTool | **in-model** | widths sized from real Helvetica AFM glyph widths; scaled to fit the page |
| Automatic page breaks / pagination | CoolUtils, CSVTool | **in-model** | rows flow across as many pages as needed; header repeats |
| Page size (Letter / A4 / Legal) | CoolUtils, i2PDF | **in-model** | `page_size` enum `letter`/`a4`/`legal`; default letter |
| Orientation (portrait / landscape) | i2PDF, TableConvert | **in-model** | `orientation` enum `portrait`/`landscape`; default portrait |
| Font size | TableConvert, i2PDF | **in-model** | `font_size` (points, 5–24, default 10) |
| Alternating row banding (zebra) | CoolUtils, csvtotable | **in-model** | `row_banding` (bool, default true) light-gray odd rows |
| Cell borders / grid | all | **in-model** | `grid` (bool, default true) full cell grid |
| Table title / heading | i2PDF, TableConvert | **in-model** | `title` (optional heading drawn above the table) |
| Right-align numeric columns | implicit in "good-looking" | **in-model** | columns whose data cells are all numeric are right-aligned automatically |
| Live table preview / editor | TableConvert, i2PDF | **out-of-model** | the page recomputes the PDF on every input change; an interactive cell editor is a site-UI feature, out of scope for a pure block |
| Custom fonts / full Unicode (CJK, emoji) | some paid tools | **out-of-model** | uses the base-14 Helvetica family (Latin-1); non-Latin-1 glyphs fall back to `?`. TrueType embedding is a larger effort — listed, not built |
| Encoding auto-detect (Win-1252/ISO-8859) | CoolUtils | **out-of-model** | input arrives as decoded UTF-8 text already; no byte-level re-decoding |
| Custom colors / theme picker | some | **out-of-model** | fixed, tasteful gray header + banding; a full color-picker theme is deferred |
| Column text wrapping inside a cell | some | **out-of-model (v1)** | cells that exceed their (auto-fit, page-scaled) column width are truncated with `...`; multi-line cell wrapping is a future enhancement noted on the page |

## UX controls to match

- Delimiter offered as friendly named choices *and* a raw single char — kept as a text
  field that accepts `comma`/`tab`/`semicolon`/`pipe` or a literal character.
- Page size + orientation as `<select>` dropdowns (enum params → manifest → `<select>`).
- Header / banding / grid as checkboxes (boolean params, default on).
- Font size as a bounded **slider** (5–24) since it is a small numeric range.
- Preset **example chips** (competitors ship sample tables / presets) for a people table,
  a landscape wide table, and a semicolon-delimited numeric table.

## Output / surface decision

Binary `application/pdf`. Chat + CLI return an `application/pdf` download envelope
(`build_media_envelope`); the standalone **page** follows the csv-to-xlsx pattern — the web
export returns a `data:application/pdf;base64,…` URL and `page/custom.js` renders a real
**Download PDF** button (reusing the generator's `#tool-output-download` anchor) plus a size
summary. No competitor copy, branding, or trademarks are reproduced anywhere.
