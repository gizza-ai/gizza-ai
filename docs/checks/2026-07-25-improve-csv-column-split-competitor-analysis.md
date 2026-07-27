# csv-column-split — competitor analysis (2026-07-25)

Tool function: split one CSV column into several columns by a delimiter, or concatenate
several columns into one. Pure text, in-browser. (All findings paraphrased from public
tool pages — no copy/branding reproduced.)

## Competitors skimmed

1. **TextGround — Excel Column Splitter** (textground.com/tools/excel-column-splitter)
   Browser alternative to Excel "Text to Columns". Custom delimiter per column, "keep
   original columns alongside" checkbox, works on Excel/CSV. Marketed for name/address
   splitting. Sparse public docs on defaults.
2. **csvtools.com — Split Column** (csvtools.com/split-column)
   Pick the column from a dropdown; enter the delimiter char/string; splits on the FIRST
   occurrence only into exactly two columns; optionally rename the two new columns;
   checkbox to keep the first row as a header; trims leading/trailing whitespace from split
   values by default (disable in advanced); if the delimiter is absent the whole value goes
   to column 1 and column 2 is blank; original column is kept, new columns appended.
   Example: `"Portland, OR"` split on `,` → `Portland` + `OR` (trimmed).
3. **SplitForge — Split a Column in CSV** (splitforge.app/blog/split-column-csv)
   Delimiters: space, comma, pipe, tab, or any custom char; regex/pattern mode. "Split all
   occurrences" → one output column per occurrence (unlimited output columns). Name each
   output column (pre-filled suggestions like First/Last Name). Keep original by default or
   replace it; place new columns after the source, at the end, or replacing the source.
   Handles 1M+ rows. No concat feature mentioned.
4. **Datablist — CSV Rows Splitter** (datablist.com/tools/csv-rows-splitter) — adjacent:
   splits a multi-value cell into multiple ROWS (one per value) rather than columns. Out of
   scope for a column-split tool but confirms delimiter + column selection is the core UX.

## Table-stakes → our decision

| Capability | Competitors | In this tool | Param |
| --- | --- | --- | --- |
| Choose which column to split | all | yes, by header name or 1-based index | `columns` |
| Value delimiter (char/string) | all | yes, any string | `separator` |
| Split into N vs first-only | csvtools=first-only(2); splitforge=all | yes: `max_columns` (0=all, 2=first-only) | `max_columns` |
| Rename output columns | csvtools, splitforge | yes, comma-separated; auto `<col>_1..` otherwise | `names` |
| Keep vs replace original | all | yes, default replace | `keep_source` |
| Trim split values | csvtools default-on | yes, default on | `trim` |
| Header handling | all | yes | `has_header` |
| CSV field delimiter (,/tab/;/\|) | implicit | yes | `delimiter` |
| Concatenate columns into one | (none of the split tools) | yes — our second mode | `mode=concat` |
| Regex/pattern split | splitforge | OUT — literal string separator only (regex deferred) | — |
| Split into rows | datablist | OUT — that's a different tool (csv-group-split / explode) | — |
| Column placement chooser (after/end/replace) | splitforge | simplified: replace-in-place (default) or keep-after-source | `keep_source` |

Concat mode is a genuine gap in the split-only competitors: joining `first` + `last` into
`full_name` with a chosen separator, dropping or keeping the sources. We ship both directions
in one tool, matching Excel's Text-to-Columns *and* the `&`/CONCAT inverse.

## Defaults chosen
- `mode=split`, `separator=","`, `max_columns=0` (split on every occurrence),
  `keep_source=false` (new columns replace the source), `trim=true`, `has_header=true`,
  `delimiter=","` (CSV field separator).
- Missing separator in a cell → the whole value lands in the first output column, the rest
  blank (matches csvtools). Output is padded to a rectangular width across all rows.

## UX
- Page uses a `<select>` for `mode` and `delimiter`, checkboxes for `keep_source`/`trim`/
  `has_header`, a number field for `max_columns`, and `[[example]]` preset chips for the two
  headline flows (split a name, join first+last).

## Out-of-model / deferred
- Regex / pattern split (literal-string separator only for now).
- Split-to-rows (explode) — a distinct transform, not a column split.
- Per-column different delimiters in one pass, and >1M-row streaming.
