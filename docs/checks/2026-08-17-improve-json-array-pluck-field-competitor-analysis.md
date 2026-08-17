# json-array-pluck-field competitor analysis — 2026-08-17

Tool: `json-array-pluck-field` — extract one field or path from each object in a JSON array into a flat list.

## Sources scanned

- Spoold JSON operations/query tool: array-of-object operations including extracting a property, picking columns, filtering, uniqueness, and grouped counts.
- JSONLint JSONPath Finder & Evaluator: JSONPath expression entry, nested paths, array navigation, and result extraction.
- JSONQueryTool online query tester: multiple query syntaxes, document extraction, JSON/XML query evaluation, and plain JavaScript-style querying.
- JSONPath online evaluator: JSONPath expression evaluation with matched values and paths.

## Table-stakes capabilities and fit decisions

| Capability / UX pattern | Observed table stake | Fit decision |
| --- | --- | --- |
| Paste JSON document into a large text area | All comparable tools start with pasted JSON or file-like text input. | In model: multiline `json` input accepts top-level arrays, wrapper objects, NDJSON, and single objects. |
| Enter a field/path expression | Competitors expose JSONPath or query expression boxes. | In model: `field` supports simple keys, dotted paths, array indexes, `*`, `**`, and JSONPath muscle memory such as `$..city`. |
| Select an array inside a wrapper response | API responses commonly wrap rows under `data`, `items`, or `results`. | In model: optional `root` path chooses the array; blank auto-detects a top-level array or first array-valued property. |
| Multiple output forms | Query tools often show JSON results and copyable text; array tools commonly need CSV/list output. | In model: `format` enum covers lines, CSV, TSV, JSON, and custom delimiter. |
| Missing/null handling | Practical extracts need a choice between sparse rows, placeholders, or errors. | In model: `missing` enum supports skip, empty, null, and error. |
| Complex nested values | If a selected value is an object or array, tools need a predictable rendering. | In model: `complex_values` enum supports compact JSON, labels, or skip. |
| Deduplicate values | Several tools include unique/distinct operations. | In model: `unique` checkbox keeps first occurrence. |
| Query filters, projections, grouping, arbitrary JavaScript | Full query/evaluator tools support broad expression languages. | Out of model for this small block: this tool intentionally avoids a general query language and only plucks one field/path from each row. |
| Tree viewer/editor or path picker UI | Some tools provide rich JSON tree interactions. | Out of model in current page model: generic tool pages render declarative form controls, not an interactive tree picker. |

## Descriptor/page choices

- Required inputs are `json` and `field`.
- `format`, `missing`, and `complex_values` are enums so the page renders selects instead of free-text boxes.
- `quote` and `unique` are checkboxes for common list cleanup operations.
- Preset chips cover nested fields, CSV email lists, wrapped API responses, and wildcard fan-out.
- Documentation states limits and distinguishes this focused pluck operation from full JSONPath/query evaluators.
