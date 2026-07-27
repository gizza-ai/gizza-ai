# dbf-table-parser — competitor analysis (2026-07-25)

Function: parse a dBase/`.dbf` table file into its column definitions + rows and export
them as CSV or JSON, fully in the browser (wasm) / CLI — no upload, no account.

## Competitors scanned

1. **DBF2002 "DBF Converter"** (dbf2002.com) — desktop + CLI. Rich option set: custom
   delimiter (`/SEPTAB`, `/SEPPIPE`, comma/space/semicolon/any char), column
   select + reorder (`/COLUMNS:NAME,STREET`), data filter (`/FILTER`), include deleted
   records (`/EXPORTALL`), double-quote text qualifier, batch/folder conversion, drop
   seconds from datetime. Paid Windows app.
2. **Convert.Guru DBF converter** — online. Outputs **CSV, JSON, XLSX**; auto-detects the
   file's format; preview before download; notes DBF's 10-char column-name limit and
   suggests GeoJSON for Shapefile attribute tables. Little exposed config.
3. **DBFOpener.com** — free online *viewer*: opens a `.dbf`, shows the table (column
   headers + rows) in a grid, and exports each table to CSV. No signup. Read-only viewer
   angle.
4. **AnyConv / OnlineConvertFree / FreeFileConvert** — generic drag-drop converters,
   DBF → CSV / XLSX / SQL, no options beyond the target format; server-side upload.
5. **RebaseData DBF→CSV** — online + downloadable Java tool; returns a ZIP of one CSV per
   input table; handles multi-file sets. Server-side.

## Table-stakes → decision (every item lands in the descriptor or is listed here)

| Capability | Fit | Decision |
|---|---|---|
| CSV output | in-model | `format=csv` (default) |
| JSON output (with column defs) | in-model | `format=json` → `{columns, row_count, rows}` |
| Column definitions (name/type/length/decimal) | in-model | surfaced in JSON `columns[]` |
| Custom delimiter (comma/tab/semicolon/any char) | in-model | `delimiter` (single char or `"tab"`) |
| Header row toggle | in-model | `header` (default true) |
| Column select + reorder | in-model | `columns` (names or 0-based indices) |
| Include deleted records | in-model | `include_deleted` (default false, matches most viewers) |
| Trim character-field padding | in-model | `trim` (default true — DBF right-pads C fields) |
| Code-page / encoding | in-model (subset) | `encoding` = auto/utf-8/latin1/cp1252 |
| Row limit / preview | in-model | `limit` (0 = all) |
| Date normalisation (YYYYMMDD → ISO) | in-model | automatic for `D` fields |
| FoxPro 4-byte integer (`I`) fields | in-model | decoded to numbers |
| Data filter / query (`/FILTER`) | out-of-model | listed — use the existing `csv-query` / `csv-filter` tools on the output |
| XLSX export | out-of-model | listed — pipe CSV into the existing `csv-to-xlsx` tool |
| SQL export | out-of-model | listed — pipe CSV into the existing `csv-to-sql` tool |
| Memo (`M`) field bodies (`.dbt`/`.fpt`) | out-of-model | listed — the sidecar memo file isn't provided to a single-file tool; memo cells emit empty and this is stated on the block |
| Shapefile geometry (`.shp`) | out-of-model | listed — this parses the `.dbf` attribute table only |
| Batch / folder conversion, multi-file ZIP | out-of-model | listed — one file per call |

## Notes

- Never copies competitor copy, branding, or trademarks — analysed for feature/UX ideas only.
- Binary-file input (`.dbf` bytes) → text output: same shape as `blocks/xlsx-to-csv` and
  `blocks/arrow-feather-to-csv` — a **chat + CLI** block with **no standalone page** (a
  binary upload is neither a pure-text page input nor an ffmpeg media transform), so there
  is no Playwright page surface to verify; CLI + descriptor/drift tests are the load-bearing
  local gates.
- Field types handled: `C` character, `N`/`F` numeric, `D` date (→ `YYYY-MM-DD`), `L`
  logical (→ bool), `I` 4-byte integer; `M` memo → empty (no sidecar); other/binary types
  → decoded text with a stated caveat.
