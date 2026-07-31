## About this tool

The **Numeric Range Check** validates pasted CSV data by checking selected columns against a minimum, maximum, or both. It reports every flagged cell with the data row, physical line, column name, value, and reason, then summarizes how many rows and cells were checked.

Use it for quick data-quality checks before importing a spreadsheet, running a model, or handing a CSV to another system. Columns can be named from the header row, selected by 1-based index for headerless data, or set to `all` when every column should be numeric and in range.

## Worked example

Input CSV:

```csv
name,age
Ada,34
Bo,150
Cy,-3
```

With `columns=age`, `min=0`, and `max=120`, the report flags Bo's `150` as above the maximum and Cy's `-3` as below the minimum.

## Limits and edge cases

- At least one of **Minimum** or **Maximum** must be provided; leave one blank for a one-sided range check.
- Inclusive bounds allow values equal to the min or max. Turn inclusive bounds off for a strict open interval.
- Non-numeric cells can be flagged as issues or ignored. `nan` and infinity are treated as non-numeric.
- The issue list is capped by **Max issues to list**, but the summary still reports the full flagged count.
- Delimiter auto-detection covers comma, tab, semicolon, and pipe based on the first non-blank row.

## FAQ

<details>
<summary>Can I check more than one column?</summary>

Yes. Enter column names or indexes separated by commas or new lines, such as `age, score`, or use `all` to check every column.

</details>

<details>
<summary>How are blank and non-numeric cells handled?</summary>

Blank cells are allowed by default and can be required by turning off **Allow blank cells**. Non-empty values that cannot be parsed as finite numbers are either flagged or ignored based on the **Non-numeric cells** setting.

</details>

<details>
<summary>Does it modify or reject CSV rows?</summary>

No. This is a report-only validator. It does not rewrite the CSV or block entries like a spreadsheet data-validation rule would.

</details>

<details>
<summary>Can I use only a minimum or only a maximum?</summary>

Yes. Leave the other bound blank to run a one-sided check such as “values must be at least 0” or “values must be at most 100”.

</details>
