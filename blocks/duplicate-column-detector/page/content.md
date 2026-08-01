## About this tool

**Duplicate Column Detector** finds repeated columns in CSV/table data. Two
columns are duplicates when their values match down every row. By default the
header name is ignored, so `email` and `contact` count as duplicates if their cell
values are identical; turn that off when you only want repeated header names with
matching values.

The tool keeps the first (leftmost) column in each duplicate group and treats
later matches as redundant copies. Use the default report to see the groups, switch
to CSV output to remove redundant columns, or choose JSON for scripting.
Everything runs locally in your browser — your data is not uploaded.

### Worked example

Input:

```
id,name,email,id_copy,contact
1,Alice,a@x.com,1,a@x.com
2,Bob,b@y.com,2,b@y.com
```

Default report:

```
Scanned 5 columns across 2 data rows.
Found 2 duplicate column groups; 2 redundant columns can be removed (3 columns remain unique).

Duplicate column groups (kept → redundant copies):
  keep "id" (col 1)  ==  drop "id_copy" (col 4)
  keep "email" (col 3)  ==  drop "contact" (col 5)

Use output=csv to get the table with the redundant columns removed.
```

With **Output = Cleaned CSV**, the result is:

```
id,name,email
1,Alice,a@x.com
2,Bob,b@y.com
```

### Options

- **First row is a header** — on by default; header names are used in reports and
  preserved in cleaned CSV output.
- **Delimiter** — comma, tab, semicolon, or pipe.
- **Ignore case** — on by default, so `Alice` and `alice` compare equal.
- **Ignore whitespace** — on by default, so stray spaces and repeated whitespace
  do not prevent a match.
- **Ignore header names** — on by default; duplicate detection compares values
  rather than requiring names to match. Turn it off to require matching header
  names too.
- **Output** — human report, cleaned CSV, or JSON groups.

### Limits

- This finds exact duplicate value sequences after optional case/whitespace
  normalization. It does not do fuzzy or semantic column matching.
- The first duplicate is kept. Reorder columns before pasting if a different copy
  should survive.
- Ragged rows are allowed; missing cells are compared as empty strings.
- Very large files should be handled in a data-cleaning script; this page is best
  for paste-sized tables and quick audits.

## FAQ

<details>
<summary>Does it compare the header names?</summary>

By default, no. The common duplicate-column cleanup pattern compares the values,
so differently named columns with the same cells are considered duplicates. Turn
**Ignore header names** off when names must also match.

</details>

<details>
<summary>Which duplicate column is kept?</summary>

The first (leftmost) column in each duplicate group is kept and later copies are
reported or removed. This is deterministic and mirrors common spreadsheet and
pandas cleanup workflows.

</details>

<details>
<summary>Can it remove the duplicate columns for me?</summary>

Yes. Set **Output** to **Cleaned CSV** to emit the table with redundant columns
removed. The header row is preserved when **First row is a header** is enabled.

</details>

<details>
<summary>Is my table uploaded anywhere?</summary>

No. The tool is compiled to WebAssembly and runs entirely in your browser. Your
CSV/table data never leaves your device.

</details>
