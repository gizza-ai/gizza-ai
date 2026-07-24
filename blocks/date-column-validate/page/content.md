## About this tool

**Date Column Validator** checks that every value in one CSV date column parses against a date format you choose, and reports the exact rows that do not. It is useful before importing spreadsheets, partner exports, or analytics feeds into a system that expects a strict date format.

Pick the column by header name (for example `joined`) or by 0-based index (for example `1` for the second column), choose a format, and run. The report shows how many values were checked, how many are valid, how many are invalid, and — for each offending cell — its data-row number, its line number in the original text, the value, and why it failed.

### Formats you can check against

- **ISO date** — `YYYY-MM-DD`, such as `2021-06-01` (the default).
- **US date** — `MM/DD/YYYY`, such as `06/15/2021`.
- **EU date** — `DD/MM/YYYY`, such as `15/06/2021`.
- **ISO date-time** — `YYYY-MM-DDThh:mm:ss`, such as `2021-06-01T12:30:00`.
- **RFC 3339** — a full date-time with offset, such as `2021-06-01T12:30:00Z`.
- **Custom pattern** — any chrono/strftime pattern in the Custom pattern field, such as `%d-%b-%Y` for `01-Jun-2021`, `%Y%m%d` for `20210601`, or `%H:%M:%S` for a time of day.

Common specifiers for a custom pattern: `%Y` four-digit year, `%m` month, `%d` day, `%b` short month name, `%H:%M:%S` time.

### Worked example

Input data:

```csv
name,joined
Ada,2021-06-01
Bo,2021/06/02
Cy,2021-13-40
```

Column `joined`, format **ISO date**. The report checks 3 rows, marks 2 valid and 2 invalid: row 2 `2021/06/02` uses slashes instead of dashes, and row 3 `2021-13-40` is not a real calendar date (month 13, day 40). Impossible calendar dates — month 13, February 30, bad leap days like `2021-02-29` — are rejected, not just badly shaped strings.

CLI example:

```bash
gizza tool date-column-validate data="name,joined
Ada,2021-06-01
Bo,2021/06/02" column=joined preset=iso-date has_header=true allow_blank=true delimiter=auto max_issues=50 output=text
```

### Limits and edge cases

The parser handles quoted fields, doubled quotes, delimiters inside quotes, quoted newlines, and blank physical lines. It auto-detects comma, tab, semicolon, and pipe delimiters from the first non-blank line, or you can set the delimiter yourself. Blank cells count as valid by default; turn off **Allow blank cells** to flag them. The invalid list is capped by **Max issues to list** (1–1000) so large files stay readable, but the summary always reports the full invalid count. This tool validates one date column at a time and does not fix, reformat, or convert values — it is a report-only checker.

## FAQ

<details>
<summary>How do I validate a CSV that has no header row?</summary>

Turn off **First row is a header** and refer to the column by 0-based index. For example, `0` is the first column and `1` is the second. A numeric value in the Column field is always read as a 0-based index, even when a header is present.

</details>

<details>
<summary>What date formats can I check against?</summary>

Presets cover ISO `YYYY-MM-DD`, US `MM/DD/YYYY`, EU `DD/MM/YYYY`, ISO date-time `YYYY-MM-DDThh:mm:ss`, and full RFC 3339. For anything else, choose **Custom pattern** and supply a chrono/strftime pattern such as `%d-%b-%Y`, `%Y%m%d`, or `%H:%M:%S`.

</details>

<details>
<summary>Does it catch impossible dates like February 30?</summary>

Yes. Values are parsed as real calendar dates, so month `13`, day `40`, `2021-02-30`, and a non-leap `2021-02-29` are all reported as invalid even though they have the right shape.

</details>

<details>
<summary>Are blank cells treated as valid?</summary>

By default, yes — blank cells are skipped and counted as valid. Turn off **Allow blank cells** to report every blank cell in the column as invalid instead.

</details>

<details>
<summary>Why does the report list fewer invalid rows than the invalid count?</summary>

The listed rows are capped by **Max issues to list** (1–1000, default 50) so a large file stays readable. The summary's Invalid count is never capped, so it always reflects the true total; raise the cap to list more offending rows.

</details>
