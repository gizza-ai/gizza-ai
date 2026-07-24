# yaml-to-csv — competitor analysis (2026-07-24)

Paraphrased scan only — no competitor copy, branding, or trademarks reproduced.

## Function

Convert YAML data into CSV rows for spreadsheets and data pipelines. The common input is a top-level YAML sequence of objects; a mapping of named records is also useful when YAML keys are ids.

## Competitors skimmed

1. **ConvertSimple YAML to CSV.** Browser paste workflow with YAML input and CSV output; table-stakes are simple paste/convert/copy, clear parse errors, and valid CSV quoting.
2. **Online YAML Tools / Browserling YAML to CSV.** Offers YAML-to-CSV with delimiter-oriented output controls and example data. Emphasizes local quick conversion and preserving nested data in readable cells.
3. **TableConvert YAML to CSV.** Supports pasted YAML and table preview/export for spreadsheet workflows; users expect delimiter choices, header handling, and examples for records with uneven keys.
4. **Konbert YAML to CSV.** Converter focused on structured data files; highlights mapping/list input, nested data flattening, and downloadable CSV.

## Table stakes → decisions

| Capability / UX pattern | Decision |
|---|---|
| Paste multiline YAML into a large textarea | **in-model** — page uses a multiline YAML field with a real placeholder. |
| Top-level list of records | **in-model** — each list item becomes a row. |
| Top-level mapping of records | **in-model** — each mapping entry becomes a row and the entry key can be kept as a column. |
| Union headers for uneven objects | **in-model** — columns are first-seen union across all rows; missing cells are blank. |
| Nested object flattening | **in-model** — nested maps become dot-path columns such as `user.name`. |
| Arrays in cells | **in-model** — default compact JSON, with `joined` and `columns` modes for common spreadsheet workflows. |
| Delimiter selection | **in-model** — comma, tab, semicolon, pipe enum. |
| Header row toggle | **in-model** — checkbox defaults on. |
| Force quote all fields | **in-model** — checkbox for downstream tools that prefer quoted CSV. |
| Download/copy text output | **in-model** — generated text-page output includes the platform text/download surface. |
| Preview/edit spreadsheet grid | **out-of-model** — this repo's generic page model is form + output, not a spreadsheet editor. |
| Upload large files/server conversion/API | **out-of-model** — gizza is browser-local/CLI; large server batch/API workflows are not built here. |
| Multiple YAML documents in one stream | **considered, not built** — table semantics are ambiguous; users can split documents and run one at a time. |

## Worked-example decisions

- Default example is a YAML list of people with an uneven nested `address.city` field, showing union columns and dot-path flattening.
- Mapping example sets `key_column=id` so YAML object keys can become row ids.
- Array example uses `array_mode=columns` to demonstrate repeatable fields for spreadsheet users.

## Limits surfaced on page

- Whole document is parsed in memory, so very large YAML files can be slow in a browser tab.
- Top-level scalars have no columns and are rejected.
- Nested key collisions that flatten to the same dot path overwrite by the later value.
- YAML anchors/aliases are expanded by the parser; custom tags are dropped while preserving the tagged value.
