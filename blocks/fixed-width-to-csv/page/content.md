## About this tool

Fixed-width text is common in mainframe exports, bank files, old reporting systems, COBOL-style extracts, and command output where columns are aligned by position instead of separated by commas. This converter turns those records into CSV without uploading anything.

Leave the column spec blank to auto-detect boundaries from whitespace that lines up on every row. For production imports, provide an explicit spec so the same layout is used every time: widths such as `10,4,*`, one-based ranges such as `1-10,11-14,15-30`, named widths such as `name:10,age:4,city:*`, or pipe-separated `position,length,name` entries such as `1,10,name|11,4,age|15,*,city`.

## Worked example

Input:

```text
name      age city
Ada        36 London
Bo          7 Oslo
```

With auto-detection and the default first-row header mode, the result is:

```csv
name,age,city
Ada,36,London
Bo,7,Oslo
```

For a repeatable import, use the spec `name:10,age:4,city:*` and set the header mode to “Use names from the column spec”. That reads characters 1-10 as `name`, 11-14 as `age`, and the rest of each line as `city`.

## Limits and edge cases

- Positions and widths are counted in Unicode characters, not UTF-8 bytes.
- The rightmost `*` column reads to the end of each line, so longer notes are not clipped.
- Short rows are padded with empty CSV fields.
- Auto-detection treats a character position as a separator only when every row has whitespace there, so explicit specs are safer for irregular files.
- A run converts up to 50,000 data lines and 512 columns.
- `quote=never` is intentionally lossy if a field contains the selected delimiter; use minimal quoting for normal CSV imports.

## FAQ

<details>
<summary>When should I use an explicit column spec instead of auto-detect?</summary>

Use an explicit spec when the file layout is known or must be repeatable. Auto-detect is convenient for quick report output, but a real fixed-width feed should usually be parsed with widths or ranges such as `10,4,*` or `1-10,11-14,15-30`.

</details>

<details>
<summary>Are column positions zero-based or one-based?</summary>

Specs use one-based positions because that is how most fixed-width layout documents describe columns. For example, `1-10` means the first through tenth characters. Width specs such as `10,4,*` start at character 1 and advance automatically.

</details>

<details>
<summary>Can I create TSV or semicolon-separated output?</summary>

Yes. Set the delimiter to `tab`, `semicolon`, `pipe`, `space`, `colon`, or any single character. The output still uses the same CSV quoting rules, so fields containing the delimiter are protected when `quote=minimal` or `quote=all` is selected.

</details>

<details>
<summary>Why did the first input line disappear from the data?</summary>

The default header mode treats the first line as column names. If your input has no header row, choose “Generate col1, col2, …” or “No header row”. If your spec has names such as `name:10,age:4`, choose “Use names from the column spec”.

</details>
