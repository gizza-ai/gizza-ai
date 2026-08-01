# missing-value-report — competitor analysis (2026-07-27)

Function: report null/empty counts and percentages per column for a CSV/table, plus a
missingness pattern summary (which columns are missing together across rows).

## Competitor scan (paraphrased; no copy/branding reproduced)

Searched the function ("missing values report per column count percentage tool pandas
isnull missingno pattern"). Skimmed the top real references:

1. **pandas idiom (Statology / bobbyhadz / DataScienceParichay writeups).** The canonical
   report is `df.isnull().sum()` (count per column) combined with `df.isnull().mean() * 100`
   (percent per column), often concatenated into one table sorted by percent descending.
   `df.isnull().sum().sum()` gives the grand total; rows with any missing are found via
   `df[df.isnull().any(axis=1)]`. Custom "missing" tokens are supplied at read time through
   `read_csv(..., na_values=[...])` on top of the built-in NaN/blank detection.
2. **missingno library.** Adds *pattern/visual* analysis on top of the counts: a matrix plot
   (one cell per value, white = missing), a bar chart of per-column completeness, and a
   heatmap of how strongly two columns' missingness correlate. These are visualizations, not
   text output.
3. **R `mice::md.pattern` idiom (referenced widely for "missingness pattern summary").**
   Produces a grid: one row per distinct present/absent pattern across the columns
   (1 = present, 0 = missing), with the count of rows matching each pattern and a count of
   complete rows. This is the textual form of the "pattern summary" the backlog asks for.

## Table-stakes → design decisions

| Capability | Competitor | In/out of model | Decision |
|---|---|---|---|
| Per-column missing **count** | pandas `isnull().sum()` | in | `column,missing,...` table |
| Per-column missing **percent** | pandas `isnull().mean()*100` | in | `missing_percent` column, trimmed % |
| Present/total per column | derived | in | `present`, `total` columns |
| Sort by % missing descending | pandas concat + sort | in | `sort=missing` (default), plus `column` / `name` |
| Grand total rows / complete rows | pandas | in | `Total rows` + `Complete rows` lines |
| Custom missing tokens | pandas `na_values` | in | `na_values` param (case-insensitive), sensible default |
| Blank/whitespace = missing | pandas default | in | always treated as missing |
| Delimiter (tab/semicolon/pipe) | read_csv `sep` | in | `delimiter` param |
| **Missingness pattern grid** | `mice::md.pattern` | in | `include_patterns` (default on), `count,<col>...` grid, capped by `max_patterns` |
| Visual matrix / bar / heatmap | missingno | **out** (needs a rendering surface) | not built; noted below |
| Imputation / filling missing | pandas `fillna` | **out** (separate function) | already covered by `missing-value-imputer` |
| dtype per column | pandas `info()` | **out** (different tool) | not built |

**Out-of-model (listed, not built):** graphical matrix/bar/heatmap visualizations
(missingno) need a chart surface this text tool doesn't have; imputation is a separate
existing tool (`missing-value-imputer`); dtype inference is a different report.

All in-model table-stakes land in the descriptor: per-column count + percent + present +
total, complete-row summary, configurable NA tokens, delimiter, three sort orders, and the
`md.pattern`-style missingness grid capped by `max_patterns`.
