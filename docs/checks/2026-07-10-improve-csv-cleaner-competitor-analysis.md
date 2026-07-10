# csv-cleaner — competitor analysis (2026-07-10)

Snapshot of the "online CSV cleaner" space used to shape the descriptor + page.
All observations paraphrased from public tool pages; no competitor copy, branding,
or trademarks reproduced.

## Competitors skimmed
1. **ConvertForge — CSV Cleaner** (convertforge.io/csv-cleaner) — browser-only, no
   upload. Removes empty rows, trims whitespace, removes duplicate/repeated header
   rows, normalizes data. No signup.
2. **Zoer — CSV Cleaner** (zoer.ai) — client-side JS. Remove duplicates, remove
   empty rows, trim whitespace, general sanitize; data never leaves the device.
3. **100Plus Tools — CSV Data Cleaner** (100plus.tools) — remove empty rows, trim
   whitespace, **normalize line endings**, dedupe records, **replace null/empty
   values**, with a table preview before download.
4. **CSV Sanitizer** (csvsanitizer.com) — format data, fix headers, remove duplicate
   rows, normalize dates, trim whitespace.
5. **Basedash — CSV Cleaner** (basedash.com) — trim whitespace, remove empty rows,
   deduplicate identical rows, normalize headers; browser-local.

## Table-stakes features (tag: in-model = pure-Rust/browser-local, fits gizza)
- Trim leading/trailing whitespace on every cell — **in-model** ✅ (`trim`)
- Remove duplicate rows (keep first) — **in-model** ✅ (`dedupe`)
- Remove empty/blank rows — **in-model** ✅ (`drop_empty_rows`)
- Normalize line endings (LF vs CRLF) — **in-model** ✅ (`line_ending` enum)
- Normalize / change the delimiter — **in-model** ✅ (`output_delimiter` enum)
- Replace empty cells / null values with a fill value — **in-model** ✅
  (`empty_cells` enum + `fill_value`)
- Header awareness (keep row 1, exempt from dedup/empty-drop) — **in-model** ✅
  (`header`)
- Download cleaned CSV — **in-model** ✅ (page `format = "text"` auto Download link)
- Table/grid preview before download — **out-of-model (deferred)**: the shared page
  driver renders text output, not an interactive grid. Considered, not built to keep
  the platform generic; the raw cleaned CSV is shown + downloadable.
- Normalize dates / typed-column normalization — **out-of-model**: date parsing/format
  is a separate concern (there are dedicated date tools); left out to keep the cleaner
  format-agnostic.
- Normalize header case/slug — **considered, rejected**: destructive header rewriting is
  surprising in a "cleaner"; trimming already tidies headers and dedupe keying stays
  predictable. A dedicated header tool is the better home.

## Defaults chosen (match the common "sensible clean" preset)
trim=on, dedupe=on, drop_empty_rows=on, header=on, empty_cells=keep, fill_value="",
output_delimiter=same, line_ending=lf. Worked example + preset chips added on the page.

## Sources
- https://convertforge.io/csv-cleaner/
- https://zoer.ai/pages/tool/csv-cleaner.html
- https://100plus.tools/tools/csv-data-cleaner
- https://www.csvsanitizer.com/
- https://www.basedash.com/tools/csv-cleaner
