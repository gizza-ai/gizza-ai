# csv-collapse-rows — competitor analysis (2026-07-26)

**Tool function:** group CSV rows by one or more key columns and collapse a chosen
column's values from every row in the group into a single delimited cell — the
CSV equivalent of SQL `GROUP_CONCAT` / `STRING_AGG` / `LISTAGG`, pandas
`groupby(...).agg(', '.join)`, and R dplyr `summarise(paste(x, collapse=", "))`.

## Competitor scan (top real approaches skimmed)

1. **SQL `GROUP_CONCAT` / `STRING_AGG` / `LISTAGG`** (MySQL / Postgres / Oracle).
   Table-stakes surface: a `GROUP BY` key list, a single column to aggregate, a
   configurable separator (default `,`), optional `DISTINCT` (dedupe) and
   `ORDER BY` inside the aggregate (sort the collapsed values).
2. **pandas `groupby(keys)[col].agg(sep.join)`** (Python).  Keys = list of
   columns; separator is the string passed to `join`; `unique()` gives dedupe;
   `sorted()` gives ordered output; blank/NaN cells are typically dropped first.
3. **R dplyr `group_by(...) %>% summarise(x = paste(x, collapse = ", "))`** and
   `toString()`.  Same shape: group keys, one collapsed column, a collapse
   separator, optional `unique()` / `sort()`.

(Spreadsheet `TEXTJOIN`+helper-column and awk associative-array recipes were also
skimmed; they expose the same knobs — key, target column, separator, de-dup.)

## Table-stakes parameters (each tagged in-model / out-of-model)

| Capability | In-model? | Decision |
|---|---|---|
| Group-by **key columns** (multi, by name or index) | in-model | `key_columns` (comma list) |
| **Column to collapse** | in-model | `collapse_column` |
| **Separator** between collapsed values (default `, `) | in-model | `separator` |
| **Dedupe** values within a group (`DISTINCT`) | in-model | `dedupe` (bool) |
| **Sort** collapsed values (none / asc / desc) | in-model | `sort_values` (enum) |
| **Skip blank** cells in the collapse column | in-model | `skip_empty` (bool) |
| Input/output field **delimiter** (comma/tab/semicolon/pipe) | in-model | `delimiter` (enum) |
| **Header** row present | in-model | `has_header` (bool) |
| Numeric aggregates (sum/avg/count) alongside collapse | out-of-model here | Covered by the separate **csv-group-by** tool — kept out to keep this one focused on list-collapse |
| Multiple collapse columns in one pass | out-of-model here | Run the tool once per column; single-column keeps the UI unambiguous |

## Defaults chosen

- `separator = ", "` (comma-space — the SQL/pandas convention; CSV quoting keeps
  it safe inside one cell).
- `sort_values = none` (preserve first-seen row order, matching `GROUP_CONCAT`
  without `ORDER BY`).
- `dedupe = false`, `skip_empty = true`, `delimiter = comma`, `has_header = true`.

## Worked example

Input:

```
region,product
East,Apple
West,Banana
East,Cherry
East,Apple
```

`key_columns=region`, `collapse_column=product`, `dedupe=true` →

```
region,product
East,"Apple, Cherry"
West,Banana
```

## UX control patterns adopted

- `sort_values` and `delimiter` render as **`<select>`** (fixed-choice `enumv`).
- `dedupe`, `skip_empty`, `has_header` render as **checkboxes** (boolean).
- `[[example]]` **preset chips** prefill a worked scenario in one click (the
  competitor tools all ship canned examples / docs snippets).
- Groups are emitted in **first-seen order** (predictable, matches SQL/pandas).

Paraphrased throughout — no competitor copy, branding, or trademarks reproduced.
