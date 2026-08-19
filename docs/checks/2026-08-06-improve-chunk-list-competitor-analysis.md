# chunk-list competitor analysis — 2026-08-06

## Sources scanned

- Browser list splitters and chunk/list batch generators.
- CSV row/column split utilities used for spreadsheet batching.
- Developer batching examples for API limits and SQL `IN` lists.

## Table-stakes capabilities

| Capability / UX pattern | Seen in competitors | Model fit | Decision |
| --- | --- | --- | --- |
| Paste list text | List chunkers start from pasted lines or delimited items | In model | Required `items` textarea with placeholders and examples. |
| Multiple separators | Competitors accept newline and comma lists, sometimes tabs/pipes | In model | Add `auto`, comma, newline, semicolon, tab, pipe, and custom separator modes. |
| Fixed chunk size | Core capability is N items per batch | In model | `chunk_size` default 10, minimum 1. |
| Keep input order | Batching tools preserve order by default | In model | Preserve item order and put remainder in the last chunk. |
| Label chunks | Many tools number groups for copying | In model | `label_chunks` checkbox default true. |
| JSON/CSV/Markdown outputs | Automation and spreadsheet workflows need structured forms | In model | Add plain, JSON, CSV, and Markdown output enum. |
| Download/upload file UI | Some tools load files directly | Out of model for this pure text page | Text paste is sufficient; file upload is a site-level enhancement. |
| Randomization/sorting | Some list tools sort or shuffle | Out of model for this row | Existing list-converter handles sorting/shuffle; chunk-list stays focused on batching. |

## Defaults chosen

- `input_separator=auto`: handles common pasted lines and comma lists.
- `chunk_size=10`: practical API/review batch size without overwhelming output.
- `output=plain`, `label_chunks=true`: copy-friendly default.

## Verification expectations

- Unit tests cover exact plain output, auto/custom separators, CSV quoting, JSON, Markdown, and error paths.
- Page tests assert real output, deep-link parameters, non-default checkbox state, and enum modes.
- CLI verification includes an exact-output case and advertised value checks for separator/output enums.
