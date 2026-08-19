## About this tool

Add a generated key column to CSV or delimiter-separated data without leaving your browser. Use it for spreadsheet imports, test fixtures, database seeds, invoices, exports that need stable row IDs, or quick QA matrices.

The default mode adds a leading `index` column starting at 1:

```csv
name,city
Ada,London
Lin,Taipei
```

becomes:

```csv
index,name,city
1,Ada,London
2,Lin,Taipei
```

You can also build zero-padded values such as `INV-0001`, add one UUID per row, or create a composite key from existing columns like `region-dept`. Delimiters can be auto-detected or fixed to comma, tab, semicolon, or pipe, and quoted CSV fields are parsed with normal RFC 4180 double-quote rules.

### Limits and edge cases

- Input is capped at 5,000,000 bytes to keep browser runs responsive.
- `has_header` is on by default. Turn it off for headerless CSV so every row gets a generated value.
- `position = before` or `after` needs `reference_column`, which can be a header name or a 1-based column number.
- Composite mode needs `columns`, a comma-separated list of source columns by header name or 1-based number.
- UUID v4 values are random. UUID v7 values are time-ordered for the current run.
- The output preserves the chosen delimiter, but CSV quoting may be normalized by the CSV writer.

## FAQ

<details>
<summary>Can I start counting at zero instead of one?</summary>

Yes. Set **Start number** to `0`. You can also change **Step** to count by 5s, count down with a negative step, or combine it with **Zero-pad width** for IDs like `0000`, `0001`, `0002`.

</details>

<details>
<summary>How do I add IDs at the end of the CSV instead of the first column?</summary>

Set **Insert position** to `End`. To place the new column near an existing field, choose `Before reference column` or `After reference column`, then enter a header name such as `email` or a 1-based column number such as `3`.

</details>

<details>
<summary>What is composite mode for?</summary>

Composite mode joins values from existing columns into a business key. For example, with columns `region,dept`, rows like `EU,ops` and `US,eng` become keys `EU-ops` and `US-eng`. Change **Composite separator** if you need `EU::ops` or another format.

</details>

<details>
<summary>What UUID formats are available?</summary>

UUID mode can emit standard lowercase UUIDs with hyphens, uppercase UUIDs, compact UUIDs with no hyphens, `{braced}` UUIDs, or `urn:uuid:` strings. Choose v4 for random IDs or v7 for IDs that sort in row order for the current run.

</details>
