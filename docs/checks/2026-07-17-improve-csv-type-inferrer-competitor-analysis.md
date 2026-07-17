# csv-type-inferrer competitor analysis (2026-07-17)

## Scope

Tool: `csv-type-inferrer` — paste CSV text, sniff the delimiter/quote/header shape, infer per-column data types, and emit a JSON schema report plus typed JSON records.

This scan looked at public CSV conversion / CSV ingestion tools and libraries that expose comparable type inference behavior. Notes below are paraphrased and focus on feature coverage, not branding or copy.

## Competitors reviewed

| Source | Relevant behavior observed | Table-stakes parameters / UX | Fit decision |
| --- | --- | --- | --- |
| JSON Viewer Tool CSV-to-JSON converter | Online CSV-to-JSON flow with an optional type-inference switch that converts numeric and boolean-looking cells while using the header row as object keys. | Paste textarea, CSV-to-JSON output, type inference toggle, object/array output options. | In model: typed JSON records, bool/number coercion, textarea input. Out of model for this tool: keyed-object-by-column mode (different output shape). |
| js2ts CSV-to-JSON converter | Online converter advertises header-keyed JSON output and data-type inference for common scalar values. | Paste/upload style input, immediate JSON output, header-derived keys, type conversion. | In model: header-derived field names, JSON records, common scalar inference. Out of model: file upload UI; current gizza page text input keeps the tool deterministic and local. |
| pandas `read_csv` | Widely-used CSV reader infers delimiters/dtypes in common workflows and lets callers override dtype/header/null-value handling. | Explicit delimiter, header control, null tokens, date parsing, dtype inference/override. | In model: delimiter override, header present/absent/auto, null tokens, date detection. Out of model: arbitrary dtype override per column and full pandas NA taxonomy. |
| DuckDB CSV import | CSV sniffer and reader can infer dialect and column types, with controls for delimiter/header and sampling-oriented ingestion. | Auto dialect sniffing, header detection, delimiter override, type inference, configurable ingestion options. | In model: delimiter/header sniffing and schema report. Out of model: database table creation, sampling knobs, remote files. |

## Required in-model capabilities shipped

- Input is a multiline CSV textarea with a realistic worked sample.
- `delimiter` is a fixed-choice enum: `auto`, `comma`, `tab`, `semicolon`, `pipe`.
- `headers` is a fixed-choice enum: `auto`, `present`, `absent`.
- `null_tokens` is user-editable so common markers such as `NA`, `N/A`, `NULL`, `None`, and `nan` do not poison inference.
- `date_detection` is a checkbox and has a non-default advertised state (false keeps date-looking columns as strings).
- `output` is a fixed-choice enum: `both`, `schema`, `records`.
- Output includes dialect details, row/column counts, per-column type/format/nullability/cardinality, and typed records.
- Zero-padded integer-like cells are preserved as strings to avoid corrupting codes.
- Page examples/preset chips cover typed sample data, semicolon/no-header data, and schema-only output.

## Out-of-model / intentionally not built

- File upload, drag-and-drop, and remote URL loading: the current generated page model is single-input text-first and avoids fetch/security concerns.
- Per-column manual dtype overrides: useful in dataframes/databases but too large for this compact schema-inference tool.
- Database import, SQL table creation, and sampling controls: outside the pure local block model.
- Exhaustive locale-aware date/number parsing: the tool supports common unambiguous date/datetime formats and deliberately leaves thousands separators/localized numbers as strings.

## Verification targets derived from the scan

- Exact CLI/page output should prove delimiter sniffing, header detection, typed scalar coercion, and records-only output.
- Tests should cover at least one non-default enum and a query-param deep link.
- Hygiene should ensure the enum descriptor and generated manifest stay synchronized so the page renders selects/checkboxes instead of generic text boxes.
