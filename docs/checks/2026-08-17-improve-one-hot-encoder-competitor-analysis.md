# one-hot-encoder competitor analysis — 2026-08-17

## Scope

Tool: `one-hot-encoder` — expand one categorical CSV column into binary indicator/dummy-variable columns for modelling and feature-engineering workflows.

## Competitor scan

Search terms used by the builder: one-hot encoder online, dummy variable CSV encoder, pandas get_dummies, scikit-learn OneHotEncoder, feature-engine OneHotEncoder.

Reviewed table-stakes from common data-science tools and libraries:

1. Dataframe encoders commonly let users select input columns, keep or drop the original category, choose a generated-name prefix/separator, and optionally drop a reference level.
2. ML preprocessing encoders emphasize deterministic category ordering, binary indicator values, unknown/missing handling, and safeguards for high-cardinality categories.
3. CSV-focused utilities need header-aware column selection, delimiter choices, previewable output, and copy-paste examples rather than a train/transform object lifecycle.

## Table-stakes matrix

| Capability / UX pattern | Decision | Notes |
| --- | --- | --- |
| Header/name or 1-based column selection | In model | Implemented with `column` and `has_header`. |
| One output column per distinct category | In model | Core emits appended indicator columns in a chosen order. |
| Generated column prefix/separator | In model | `prefix` plus `separator`, blank prefix defaults to source column. |
| Keep or drop source column | In model | `drop_original` defaults true, can be turned off. |
| Drop reference level | In model | `drop = none|first|last|if-binary` covers k and k-1 encodings. |
| Missing/blank handling | In model | `zeros`, `separate`, `blank`, and `error` are explicit. |
| High-cardinality limits | In model | `max_categories`, `min_count`, `other_column`, and a 512-column hard cap. |
| Custom positive/negative values | In model | Supports 1/0, true/false, Y/N, etc. |
| Column order controls | In model | Alphabetical, frequency, first-seen. |
| Fit/transform object with persisted vocabulary | Out of model | Gizza tools are one-shot transforms over one pasted CSV. |
| Multi-column batch encoding | Out of model for this block | Repeatable by running the one-column tool multiple times; multi-column orchestration would complicate the page. |
| Sparse-matrix output | Out of model | This repo returns CSV/text surfaces, not scipy sparse matrices. |

## Descriptor/page decisions

- Required controls: `data`, `column`.
- Fixed-choice controls: `drop`, `missing`, `sort`, `delimiter`.
- Numeric sliders: `max_categories`, `min_count`.
- Boolean controls: `drop_original`, `other_column`, `case_sensitive`, `has_header`.
- Text controls: `prefix`, `separator`, `positive`, `negative`.
- Presets cover normal encoding, reference drop, top-N with other bucket, and true/false outputs.

## Verification focus

- Exact CSV output for a header-selected column.
- Deep-link with `drop=first`, `drop_original=false`, and non-default sort.
- Missing/other/value-matrix cases through CLI.
- Hard cap and enum drift through unit tests and hygiene.
