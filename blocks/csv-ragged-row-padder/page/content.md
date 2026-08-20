## Repair CSV rows with inconsistent column counts

Paste a CSV, TSV, semicolon file, or pipe-delimited table whose rows do not all have the same number of fields. This tool normalizes the table to a target width: short rows get padded with blank cells (or your pad value), while rows with too many fields can be truncated, merged into the final column, flagged in a report, or dropped.

It preserves CSV quoting with the Rust `csv` parser/writer instead of splitting on commas by hand, so fields like `"New York, NY"` stay intact. Use it before importing messy exports into spreadsheets, databases, warehouse loaders, validation suites, or command-line tools that reject ragged records.

### Worked example

Input:

```csv
name,email,age
Ada,ada@example.com,37
Grace,grace@example.com
Linus,linus@example.com,54,extra
```

With **Infer width from** set to `Header row`, **Long rows** set to `Truncate extras`, and an empty **Pad value**, the repaired CSV is:

```csv
name,email,age
Ada,ada@example.com,37
Grace,grace@example.com,
Linus,linus@example.com,54
```

Choose **Output → Repair report** first when you want to audit what changed before exporting the repaired CSV.

### Controls and limits

- **Target width**: `0` infers it; otherwise enter the exact number of fields every row should have.
- **Infer width from**: header row, widest row, or most common row width.
- **Long rows**: truncate extras, merge extras into the last field, flag only, or drop the row.
- **Delimiter**: auto-detects comma, tab, semicolon, or pipe outside quotes; you can force one.
- **Pad value**: blank by default; use `NULL`, `NA`, or another marker if your importer needs explicit missing values.
- Input is capped at 5 MB and target width at 10,000 fields. For bigger files, use the CLI in a local pipeline.
- This repairs row width only. It does not infer column types, validate business rules, or deduplicate rows.

## FAQ

<details>
<summary>Why do CSV rows become ragged?</summary>

Common causes are missing trailing delimiters, broken exports, hand-edited files, optional columns, unescaped commas, or concatenating data from different schemas. This tool fixes the row-width problem after the CSV parser has interpreted the quoting and delimiter structure.

</details>

<details>
<summary>Should I truncate, merge, flag, or drop long rows?</summary>

Use **flag** when auditing unknown data, because it leaves long rows intact and lists them. Use **merge** when the extra fields are really part of the final free-text column. Use **truncate** only when extras are known junk, and **drop** when long rows are invalid records you want removed.

</details>

<details>
<summary>What does “infer width from mode” mean?</summary>

Mode picks the most common row width. It is useful when the header is missing or broken and most data rows already have the right shape. Ties choose the wider width so fewer values are discarded.

</details>

<details>
<summary>Will quoted commas or tabs be preserved?</summary>

Yes. Delimiter sniffing counts delimiters outside quotes, and the repair step uses the Rust CSV reader and writer. Quoted fields are parsed as cells, then re-quoted as needed in the output.

</details>

<details>
<summary>Can this fix malformed quoting?</summary>

No. If the input has broken quotes that the CSV parser cannot read, the tool reports a parse error. Fix quoting first, then use this tool to normalize the number of fields per row.

</details>
