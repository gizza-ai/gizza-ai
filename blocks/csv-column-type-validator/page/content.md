## About this tool

**CSV Column Type Validator** checks selected CSV columns against a small declared schema and reports every cell that does not match. It is useful before importing spreadsheets, partner exports, or analytics feeds into a stricter system.

Declare types with a comma- or newline-separated list such as:

```text
age:int, score:float, active:bool, joined:date, plan:enum(free|pro)
```

Then paste CSV data. With a header row enabled, declarations use header names. With **First row is a header** disabled, declarations use 1-based column indexes such as `2:int, 3:enum(free|pro)`.

### Worked example

Input data:

```csv
name,age,score,active,joined,plan
Ada,34,9.5,true,2021-06-01,pro
Bo,twelve,8,yes,2021/06/02,gold
```

Types:

```text
age:int, score:float, active:bool, joined:date, plan:enum(free|pro)
```

The report marks the CSV invalid and lists three offending cells: row 2 `age` is not an integer, row 2 `joined` is not an ISO date, and row 2 `plan` is outside the allowed enum values.

### Supported checks

- **int** — signed whole numbers like `42` or `-7`.
- **float** — decimal or scientific notation like `3.14`, `.5`, `2.`, or `1e6`.
- **bool** — `true/false`, `1/0`, `yes/no`, `t/f`, `y/n` (case-insensitive).
- **date** — controlled by Date format: ISO `YYYY-MM-DD`, US `MM/DD/YYYY`, EU `DD/MM/YYYY`, or any of those.
- **enum(a|b|c)** — exact allowed values; enum matching is case-sensitive after trimming whitespace.

### Limits and edge cases

The parser handles quoted fields, doubled quotes, delimiters inside quotes, quoted newlines, and blank physical lines. It auto-detects comma, tab, semicolon, and pipe delimiters from the first non-blank line. It does not perform regex validation, min/max numeric ranges, email/phone/postal checks, type coercion, or automatic fixing; it is a report-only type checker.

CLI example:

```bash
gizza tool csv-column-type-validator data="name,age
Ada,34
Bo,x" types="age:int" header=true delimiter=comma date_format=iso empty_ok=true max_issues=50 format=text
```

## FAQ

<details>
<summary>How do I validate a CSV without headers?</summary>

Turn off **First row is a header**, then refer to columns by 1-based index in the type declaration. For example, `2:int, 3:enum(free|pro)` validates the second column as an integer and the third column against the two enum values.

</details>

<details>
<summary>What happens when I declare a column that is not in the CSV?</summary>

The result is invalid even if no cell-level type errors are found. Unknown declared columns usually mean a misspelled header, a delimiter mismatch, or a headerless file being checked with header mode still enabled.

</details>

<details>
<summary>Are empty cells valid?</summary>

By default, yes: blank checked cells are allowed. Disable **Allow blank cells** to make every checked cell required; blank values will then be reported as `empty cell (required)`.

</details>

<details>
<summary>Can this validate emails, phone numbers, regexes, or numeric ranges?</summary>

No. This tool focuses on coarse column types: integer, float, boolean, date, and enum. Use dedicated validators for business-specific formats, or pre-clean the CSV before running this type check.

</details>
