# qif-ofx-convert — competitor analysis (2026-07-07)

New-tool build. One WebSearch ("convert QIF OFX bank statement to CSV online tool")
then skimmed the top real competitor tools. All notes are PARAPHRASED — no competitor
copy, branding, or trademarks reproduced.

## Competitors skimmed

1. **csvtools.com/qif-to-csv** — browser-local QIF→CSV. Paste content OR drop a `.qif`
   file (uses only the first file if several are dropped). Emits columns Date, Amount,
   Payee, Memo, Category, Check Number, and drops columns that hold no data. No settings
   surfaced (fixed output). Runs entirely client-side; data never uploaded.
2. **ofxconverter.com** — OFX/QFX/QBO → Excel/CSV. Drag-drop upload. No output-format
   options surfaced on the page. Gated: unregistered = 1 file / 24h, registered = 5 / 24h,
   paid tiers for more. Excel (xlsx) output alongside CSV.
3. **statementextract.com/convert/qif-to-csv** (page returned 403 to the fetch, so profiled
   from the search snippet + the tool's known feature set): positions on instant, private,
   in-browser, Excel-compatible clean output; QIF→CSV. Search snippet stresses local
   processing (no upload).
4. **financefileconverter.com / accountingconverter.com** (listing-level scan): broad
   bank-file converters (PDF/CSV/QBO/OFX/QIF ↔ CSV/Excel/QBO). Free for light use, most
   value is PDF-statement OCR + QBO output (out of a pure-Rust browser model).

Fewer than 5 tools returned a fully profileable page (one 403, two are listing/marketing
pages), so this scan uses the reachable ones. That is honest per the skill's "if fewer than
5 real competitors exist, say so" rule.

## Table-stakes matrix

| Table-stake | Seen at | Fit | Decision |
| --- | --- | --- | --- |
| Paste file content as text | csvtools, statementextract | in-model | multiline textarea (primary input) |
| Drag-drop / file upload | all | page nicety | OUT: pure-text pages use paste, not a file picker; documented on the page |
| Columns: Date, Amount, Payee, Memo, Category, Check # | csvtools | in-model | in normalized column set |
| Columns: Type, FITID (OFX) | ofxconverter (implied) | in-model | added to normalized set (empty for QIF) |
| Handle both QIF and OFX | (two separate tools upstream) | in-model | `format` = auto / qif / ofx, one tool does both |
| Normalize the date | implied (Excel-compatible output) | in-model | `date_format` = iso / us / eu / raw |
| Choose the delimiter | generic CSV tooling | in-model | `delimiter` = comma / semicolon / tab / pipe |
| Drop empty columns | csvtools | in-model | `drop_empty_columns` boolean (matches csvtools) |
| Flip amount signs | statementextract-class | in-model | `invert_amounts` boolean |
| EU decimal comma | some EU converters | in-model but conflicting | CONSIDERED, REJECTED: a comma decimal collides with a comma delimiter and forces CSV quoting; the normalized amount stays a machine-parseable `.`-decimal that every budgeting importer accepts |
| Split-transaction expansion (a row per split) | a few | in-model | REJECTED for v1: one row per transaction (predictable for import templates); split categories are joined into the Category cell and the behavior is stated on the page |
| Multi-file batch | ofxconverter (paid) | out-of-model | OUT: no server/account; convert one export at a time |
| Excel/XLSX output | ofxconverter, statementextract | out-of-model here | OUT: this tool's job is clean CSV; pair with the csv-to-xlsx tool for a workbook |
| PDF-statement conversion / QBO output | financefileconverter et al. | out-of-model | OUT: needs OCR / a QBO writer / a server |

## UX patterns adopted

- Paste-in textarea (private, in-browser, no upload) — the shared gizza value prop already
  matches every competitor's headline claim.
- `format`, `date_format`, `delimiter` render as native `<select>`s from the descriptor
  enums (with friendly `[input.labels]`); the two toggles render as checkboxes.
- Two one-click `[[example]]` preset chips (a QIF sample and an OFX sample) — competitors
  ship no presets, so this is a UX edge, not a copy.
- Empty-column dropping mirrors csvtools' "clean" output but is opt-in so import templates
  can rely on a fixed column set.

## Out-of-model list (considered, not built)

- Drag-drop file upload widget on the page (pure-text page uses paste).
- Multi-file batch conversion (needs a backend/account).
- Excel/XLSX and QBO output (use csv-to-xlsx for a workbook; QBO needs a writer + a server).
- PDF bank-statement extraction (needs OCR / a model).
