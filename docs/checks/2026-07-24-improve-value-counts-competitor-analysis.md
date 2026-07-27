# value-counts — competitor analysis (2026-07-24)

Tool function: count the distinct values in one chosen column of a CSV/table, with each
value's count and its percentage of the total, ranked most-frequent-first (the pandas
`Series.value_counts()` idiom). Paraphrased analysis only — no competitor copy/branding reused.

## Competitors skimmed (top 3 + reference)

1. **CSV Tools — "Count Values"** (csvtools.com/count-values). Browser-based, no upload,
   counts how often each unique value appears; output copy-to-clipboard (tab-separated for
   Excel) or save as CSV. Also a sibling "Percentage of Total" tool that appends a share
   column. Table-stakes: per-value counts, browser-local/no-upload, copyable output.
2. **DataXForge — "Unique Value Count"** (dataxforge.com/tools/unique-value-count). Free,
   browser-local ("data is never uploaded"), counts distinct values per column. Table-stakes:
   distinct-value tally, privacy/local compute.
3. **Datablist — distinct values from a CSV column** (datablist.com/learn/csv/...). Cloud CSV
   editor; "get distinct values from a column" with the sum of occurrences per value.
   Table-stakes: distinct values + occurrence count. Editor/storage is cloud → out-of-model.
- **Reference: pandas `value_counts()`** — the canonical semantics: sort by count descending
  by default (`sort`/`ascending`), `normalize=True` for percentages/proportions, `dropna`
  to include or drop empty/NaN. These map directly onto our params.

## Table-stakes → decision (in-model / out-of-model)

| Capability | Decision | Where |
|---|---|---|
| Pick a column by header name **or** 1-based index | in-model | `column` param |
| Per-value **count** | in-model | output `count` column |
| Per-value **percentage** of total (normalize) | in-model | output `percent` column |
| Sort by **count desc** (default) or by **value** | in-model | `sort` enum |
| **Case-insensitive** grouping ("Apple" = "apple") | in-model | `case_sensitive` (default true) |
| Include vs drop **empty** cells (pandas `dropna`) | in-model | `include_empty` (default false) |
| Non-comma **delimiters** (tab/semicolon/pipe) | in-model | `delimiter` param |
| Copy / download the result | in-model (platform) | page auto Copy + text Download |
| Preset examples | in-model (platform) | `[[example]]` chips |
| Count values across **all columns at once** | considered, rejected | keep single-column/one-series focus (pandas value_counts is per-series); `csv-stats` covers per-column summaries |
| Bar-chart / visualization of the distribution | out-of-model | listed, not built (`csv-chart-generator` covers charting) |
| Cloud CSV editor / saved datasets | out-of-model | needs a backend/account |

Distinct from existing blocks: `count-line-frequency` counts plain one-per-line text with no
column concept and no percentages; `csv-group-by` is general multi-column aggregation; `csv-stats`
is per-column summary statistics. `value-counts` is the focused single-column value_counts with
percentages.
