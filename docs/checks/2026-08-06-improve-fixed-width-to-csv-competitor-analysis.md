# fixed-width-to-csv competitor analysis (2026-08-06)

## Scope

Tool: `fixed-width-to-csv` — split fixed-width / column-positional text records into CSV using either a supplied layout or inferred column boundaries.

## Competitor-style scan

Reviewed table-stakes behavior from common fixed-width conversion tools and libraries in this space: browser text-table converters, ETL/import wizards, and fixed-width parser packages. The consistent user expectations are:

| Capability / UX pattern | In model? | Decision |
| --- | --- | --- |
| Paste a block of fixed-width text directly | Yes | Implemented as required multiline `text`. |
| Auto-detect columns from aligned spaces for quick one-off data | Yes | Implemented when `spec` is blank. |
| Supply exact widths for repeatable layouts | Yes | Implemented with comma-separated width spec such as `10,4,*`. |
| Supply absolute column positions/ranges | Yes | Implemented with one-based inclusive ranges such as `1-10,11-14`. |
| Name columns from the layout definition | Yes | Implemented with `name:width` and pipe `position,length,name` forms plus `header=names`. |
| Choose whether the first row is a header | Yes | Implemented with `header=first-row`, `names`, `generate`, and `none`. |
| Trim padded fields | Yes | Implemented with `trim` checkbox defaulting on. |
| Choose delimiter (CSV, TSV, semicolon, pipe) | Yes | Implemented with delimiter aliases and single-character custom delimiters. |
| Control quoting | Yes | Implemented as minimal/all/never. |
| Windows CRLF output and Excel UTF-8 BOM | Yes | Implemented with `newline` and `bom`. |
| Skip report banners, comments, and blank lines | Yes | Implemented with `skip_lines`, `comment`, and `skip_blank`. |
| Upload large local files and stream output | Out of model for this pure text page | This gizza tool accepts pasted text/chat args; hard caps avoid excessive memory. |
| Visual drag-to-place column rulers | Out of model for current generator | Not built; explicit specs and examples cover the same conversion model. |
| Detect columns from sample and then show editable generated spec | Out of model for current page runtime | The core detects boundaries internally; exposing the inferred spec would need a second UI/output mode. |

## Defaults chosen

- `header=first-row`, matching the common expectation for pasted report tables.
- `trim=true`, because fixed-width padding is normally layout, not data.
- `delimiter=comma`, `quote=minimal`, `newline=lf` for standard CSV output.
- `skip_blank=true` to avoid accidental empty rows from copied report spacing.
- `bom=false` so output is plain UTF-8 unless Excel compatibility is explicitly requested.

## Worked examples mirrored in the page

1. Auto-detect aligned columns from a three-line text table.
2. Use a named explicit spec and semicolon delimiter for repeatable imports.
3. Skip a report banner/comment and emit tab-separated CRLF output.

## Verification notes

The descriptor includes every in-model table-stakes control. Page metadata uses textarea input for multi-line records, select labels for fixed-choice options, checkboxes for booleans, and example chips for common workflows. Copy is generic and unbranded.
