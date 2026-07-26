## About this tool

Regex Column Validate checks one CSV column against a regular expression and lists
which rows fail the rule. Use it for format checks such as ZIP/postal codes,
phone-like IDs, invoice numbers, SKU shapes, or any one-column data-quality rule
that is easier to express as a regex.

By default the regex must match the whole cell. Turn off full-cell matching when
you only need the pattern to appear somewhere inside each value. You can also
invert the report to flag values that **do** match a forbidden pattern.

### Worked example

CSV:

```csv
name,zip
Ada,12345
Bo,1234
Cy,90210
```

With column `zip`, pattern `\d{5}`, and full-cell matching on, the text report
flags Bo's row because `1234` has only four digits:

```text
Rows checked: 3
Valid: 2
Invalid: 1
row 2 (line 3): "1234" — does not match the pattern
```

## Limits & edge cases

- Regex syntax is Rust `regex` syntax. Backreferences and look-around are not
  supported by that engine.
- Use inline flags such as `(?i)`, `(?m)`, or `(?s)` for advanced regex modes.
- Numeric column selections are 0-based indexes. Header names work when the
  header checkbox is on.
- `max_issues` only caps the listed rows. The total invalid count is still
  reported.
- Blank cells can be allowed or treated as invalid before regex matching.

## FAQ

<details>
<summary>Does the pattern have to match the whole cell?</summary>

By default, yes. The tool anchors the pattern to the full cell so `\d{5}` means
exactly five digits. Turn off "Require full-cell match" to accept any substring
match inside the cell.

</details>

<details>
<summary>Can I find values that match a forbidden pattern?</summary>

Yes. Change "Flag which values" to "Values that match". That inverts the report
and is useful for finding all-caps placeholders, test IDs, or other disallowed
formats.

</details>

<details>
<summary>How do I use case-insensitive or multiline regex flags?</summary>

Use the checkbox for case-insensitive matching, or embed Rust regex inline flags
in the pattern, such as `(?i)hello` or `(?m)^ID-\d+$`.

</details>

<details>
<summary>Can it validate multiple columns at once?</summary>

No. This focused tool validates one column and one regex rule per run. For a
larger schema with multiple rules, use a dedicated data validator or run this
tool once per column.

</details>
