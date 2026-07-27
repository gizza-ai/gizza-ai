## About this tool

Before you clean or model a dataset you need to know where the holes are. This tool takes a CSV or TSV table with a header row and reports, for every column, how many cells are missing, how many are present, the row total, and the missing percentage — the spreadsheet equivalent of pandas' `df.isnull().sum()` and `df.isnull().mean() * 100` in one pass. A blank or whitespace-only cell always counts as missing, and you can add your own tokens (like `NA`, `null`, or `#N/A`) that should be treated the same way.

On top of the per-column counts it can show a **missingness-pattern grid**: each distinct present(1)/missing(0) combination across the columns, with how many rows share it, most-common-first. This is the textual form of R's `mice::md.pattern` idiom, and it answers a question the per-column counts cannot — *which columns tend to go missing together*. If two fields are always blank in the same rows, the grid makes that jump out.

### Worked example

Paste this table:

```text
name,age,city
Alice,30,NYC
Bob,,LA
,25,
Carol,40,NYC
```

You get a per-column table showing `age` and `city` and `name` each missing one cell (25%), a summary line reporting 4 total rows with 2 complete rows (50%), and a pattern grid: two fully-present rows, one row missing `age`, and one row missing both `name` and `city`.

### Controls

- **Delimiter** accepts a single character or a name — `comma` (default), `tab`, `semicolon`, or `pipe`. Use `tab` for TSV data.
- **Extra missing tokens** is a case-insensitive, comma-separated list added to the built-in blank detection. Leave it empty to count *only* blank cells as missing, so a literal `NA` stays a real value.
- **Sort columns by** orders the per-column table: most-missing-first (default), original column order, or column name A–Z.
- **Show missingness-pattern grid** toggles the `mice::md.pattern`-style grid on or off.
- **Max pattern rows** caps how many pattern rows are listed; any extra patterns are summarized as a `(N more…)` note.

### Limits and edge cases

Short rows (fewer fields than the header) count the absent trailing columns as missing. Quoted fields with embedded commas or newlines are parsed per RFC 4180. This is a profiler, not a fixer: to fill or drop missing cells use a dedicated imputation tool, and for a visual matrix/bar/heatmap of missingness reach for a charting library. Everything here runs locally in your browser — nothing is uploaded.

## FAQ

<details>
<summary>What counts as a missing value?</summary>

A cell is missing when it is empty, contains only whitespace, is absent because the row is shorter than the header, or matches one of your **extra missing tokens** (compared case-insensitively after trimming). By default the tokens `NA`, `N/A`, `null`, `NaN`, `None`, and `#N/A` are treated as missing.

</details>

<details>
<summary>How do I count only truly blank cells?</summary>

Clear the **Extra missing tokens** field. With no tokens configured, only empty and whitespace-only cells are counted as missing, so a literal string like `NA` is kept as a real value.

</details>

<details>
<summary>What is the missingness-pattern grid?</summary>

It groups rows by their present(1)/missing(0) fingerprint across all columns and shows the count of rows sharing each pattern, most-common-first. It mirrors R's `mice::md.pattern` and reveals which columns go missing together — for example, whether `latitude` and `longitude` are always blank in the same rows.

</details>

<details>
<summary>Does it handle TSV and other delimiters?</summary>

Yes. Set **Delimiter** to `tab` for TSV, or use `semicolon`, `pipe`, `comma`, or any single character. The same delimiter is used to read the input and to write the report table.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The report is computed entirely in your browser via WebAssembly. Your table never leaves your machine.

</details>
