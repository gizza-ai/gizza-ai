# category-canonicalize competitor analysis — 2026-08-17

## Scope

Tool: `category-canonicalize` — normalize categorical CSV/list values to canonical labels using a user-supplied mapping, with fuzzy suggestions for uncovered values.

## Competitor scan

Search query: online categorical data cleaning canonicalize categories fuzzy mapping CSV tool OpenRefine Trifacta DataPrep.

Reviewed patterns from public data-cleaning tools and docs:

1. Open-source spreadsheet-like data cleaning tools expose clustering/faceting, manual merge decisions, case/whitespace cleanup, and repeatable transformations.
2. Commercial data-prep workbenches commonly provide column selection, recipe/history steps, fuzzy matching suggestions, preview-before-apply, and exportable cleaned tables.
3. General CSV cleaning utilities emphasize delimiter handling, header-aware column selection, null/blank handling, and downloadable CSV/JSON reports.

## Table-stakes matrix

| Capability / UX pattern | Decision | Notes |
| --- | --- | --- |
| Paste CSV/TSV or newline list | In model | Implemented as `data`, delimiter auto-detect, and blank column for single-column lists. |
| Select a header name or 1-based column | In model | `column` accepts names/indexes and comma-separated multiple columns. |
| Supply a canonical mapping | In model | `mapping` supports several separators, `|` variants, bare canonical lines, and comments. |
| Normalize case and whitespace differences | In model | `ignore_case` and `ignore_spacing` default on but are explicit controls. |
| Preview unmatched values before rewriting | In model | `output=suggestions` emits a review CSV with counts and nearest canonical. |
| Fuzzy suggestion threshold | In model | Slider 0–100; `unmatched=fuzzy` only applies suggestions at/above the threshold. |
| Strict pipeline mode | In model | `unmatched=error` fails with uncovered values; `blank` and `keep` cover other workflows. |
| Markdown and JSON audit output | In model | Added for reviewable reports and downstream automation. |
| Recipe/history UI | Out of model | This repo provides one-shot tools, not a multi-step data-prep notebook. |
| Unsupervised clustering with phonetics/ML | Out of model | This tool intentionally applies a user vocabulary; clustering belongs in a separate block. |
| Remote data source connectors | Out of model | Current gizza page takes pasted/local inputs. |

## Descriptor/page decisions

- Required controls: `data`, `mapping`.
- Text controls: `column`, `delimiter`.
- Boolean controls: `header`, `ignore_case`, `ignore_spacing`.
- Enums: `unmatched = keep|fuzzy|blank|error`, `output = csv|markdown|json|suggestions`.
- Slider: `fuzzy_threshold` from 0 to 100.
- Presets cover final CSV, suggestions review, fuzzy application, and plain-list mode.

## Verification focus

- Exact CSV rewriting for a header-selected column.
- Suggestions output ordering and scores.
- Deep-link state for non-default checkbox/enum/threshold choices.
- CLI examples for plain list and CSV table inputs.
