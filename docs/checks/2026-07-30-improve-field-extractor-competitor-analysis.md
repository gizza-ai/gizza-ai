# field-extractor — competitor analysis (2026-07-30)

Tool: **field-extractor** — extract specific fields/columns or character ranges from each line
with positive/negative indices; a friendly `cut`/`awk` replacement. Pure, browser-local, no upload.

## Competitors scanned

1. **miniwebtool — Text Column Extractor** (`miniwebtool.com/text-column-extractor/`)
2. **Browserling — Extract Column** (`browserling.com/tools/extract-column`)
3. **MeFancy — Delimited Column Extractor** (`mefancy.com/textchange/delimited-column-extractor`)
4. **i2TEXT — Extract Column from Text** (`i2text.com/extract-column-from-text`)
5. **text-tools.dev — Extract Delimited Columns** — UNREACHABLE at scan time (DNS `ENOTFOUND`);
   replaced in the working set by i2TEXT (kept in the list for completeness).

(All paraphrased — no competitor copy, branding, or trademarks reproduced.)

## Table-stakes (what every real competitor ships)

| Capability | Competitors | Our decision |
|---|---|---|
| Multi-line paste input | all | ✅ `text` (multiline) |
| Choose a delimiter (comma, tab, pipe, semicolon, colon, space, custom) | all | ✅ `delimiter` free-text string with `\t` escape support + example chips for the common ones — MORE flexible than a fixed dropdown (any multi-char string works, unlike `cut -d`) |
| Smart whitespace split (collapse runs) — awk default | miniwebtool, i2text | ✅ blank `delimiter` = collapse runs of whitespace (awk `$1..$n`) |
| 1-based column numbering | all | ✅ 1-based |
| Single / multiple columns (`1,3,5`) | all | ✅ |
| Ranges (`1-3`), combined (`1,3-5,7`) | miniwebtool, mefancy, i2text | ✅ |
| Reorder columns (`3,1,2`) | mefancy, i2text | ✅ (selectors emit in the order given) |
| Output delimiter (same-as-input / newline / comma / tab / custom) | miniwebtool, mefancy, i2text | ✅ `output_delimiter` (blank = same as input; `\t`/`\n` escapes; `newline` keyword) |
| Trim whitespace from fields | miniwebtool, mefancy | ✅ `trim` boolean |
| Skip empty lines | miniwebtool, mefancy | ✅ `skip_empty_lines` boolean |
| Skip first/header row | i2text | ✅ `skip_header` boolean |
| Missing-column handling | miniwebtool (skip/empty/placeholder) | Behaviour, not a param: an explicitly-numbered field that's out of range emits an empty string (matches `cut`); ranges stop at the last field. Documented on the page. |

## Differentiators (in-model, none of the competitors have these — the "cut/awk replacement" edge)

- **Negative indices** (`-1` = last field, `-2` = second-to-last) — the description's headline feature; no competitor supports it.
- **Open-ended ranges** (`3-` = from field 3 to the end), like `cut`.
- **Character-position mode** (`chars`) — extract by character range per line (`cut -c`); no competitor offers it.
- **Multi-character / escaped delimiters** (`\t`, `::`, ` | `) — `cut -d` is single-char only.
- **Unicode-safe** character mode (counts code points, never splits a character).

## Considered, rejected (in-model but declined)

- **Unique rows / Sort result** (i2text toggles): out of scope — gizza already ships dedicated
  `find-unique-lines`, `remove-duplicate-lines`, and `sort-lines` tools; folding them in here would
  duplicate those and bloat the schema. Left to those tools (composable via CLI pipes).
- **Fixed delimiter dropdown**: a free-text `delimiter` field + preset example chips covers every
  dropdown value AND custom/multi-char delimiters in one control, so a `<select>` would be strictly
  less capable.
- **Quoted/escaped-CSV parsing**: explicitly a simple splitter (same limitation every competitor
  states). gizza already has `csv-column-split` / `csv-reorder-columns` for RFC-4180 CSV. Stated on
  the page as a known limit.

## Out-of-model (need a backend / account — not built)

- File upload > a few MB, drag-and-drop bulk, download-as-file for huge outputs (browser-local paste
  covers the tool's scope; the CLI handles large files/pipes).

## Result

Descriptor ships all table-stakes plus the four differentiators from the start. No competitor copy
reproduced. Rejected items recorded above with reasons.
