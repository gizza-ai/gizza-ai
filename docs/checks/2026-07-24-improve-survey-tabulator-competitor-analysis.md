# survey-tabulator — competitor scan & design decisions (2026-07-24)

Paraphrased scan only — no competitor copy, branding, or trademarks reproduced. Purpose: decide the table-stakes surface for a local survey CSV tabulator before implementation.

## Search

One WebSearch: "survey response CSV tabulator crosstab frequency table online". Skimmed the top real tools and documentation pages for survey export analysis and crosstab/frequency workflows.

## Competitors reviewed (paraphrased)

1. **Online survey report dashboards.** Common workflow: import/export response data, show answer distributions per question, hide or count skipped answers, and segment answers by another question or demographic field. Results are usually displayed as counts plus percentages and sometimes charts.
2. **Spreadsheet pivot-table workflows.** The table-stakes path is selecting a row field and a column field from survey CSV headers, then counting records by category. Users expect row/column/grand-total percentages and marginal totals.
3. **Statistics/crosstab calculators.** These typically accept a contingency table or raw categories, then report a chi-square statistic, degrees of freedom, p-value, and an association/effect-size measure.

## Table-stakes params (tagged)

| Param / feature | In-model? | Decision |
| --- | --- | --- |
| Paste or upload survey CSV | in-model | `data` textarea; first row is headers, later rows are responses. |
| Delimiter choice | in-model | `delimiter` accepts comma, tab, semicolon, pipe, or one character. |
| Per-question answer counts | in-model | `mode=overview` tabulates every question or a selected `question`. |
| Percentages next to counts | in-model | Overview always shows count + percent of answered rows. |
| Blank-answer policy | in-model | `include_blanks` checkbox; blanks are dropped by default or counted as `(blank)`. |
| Select a question by header or index | in-model | `question` and `by` accept exact header text or 1-based index. |
| Cross-tab / segmentation | in-model | `mode=crosstab`, `question` as rows, `by` as columns. |
| Row/column/total percentages | in-model | `percent` enum: total, row, column, none. |
| Chi-square association stats | in-model | `stats` checkbox appends chi-square, df, Cramér's V, p-value. |
| Sort and top-N trimming | in-model | `sort` enum plus `top` for overview. |
| Charts and dashboards | out-of-model | This repo's generic page renders text output; charts belong in the consuming UI. |
| Weighting, multi-select splitting, survey-platform APIs | out-of-model for v1 | Listed, not built. They require platform-specific semantics or heavier data modelling. |

## UX controls

- `data` is a textarea with a small sample CSV placeholder.
- `mode`, `percent`, and `sort` are descriptor enums, so they render as selects.
- `include_blanks` and `stats` are checkboxes; tests cover non-default checked states.
- `question`, `by`, `top`, and `delimiter` are text/number fields with placeholders.
- Preset chips cover an overview and a crosstab-with-stats example.

## Model decision

The v1 output is a deterministic monospaced text table. That matches CLI, page, and chat surfaces without needing chart libraries or a binary document format. CSV parsing uses a wasm-safe Rust CSV parser; chi-square p-values use a local regularized-gamma implementation to avoid native or network dependencies.
