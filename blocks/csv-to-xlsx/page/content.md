## Turn a CSV, TSV or JSON table into a real Excel workbook

Paste your table, pick the source format (or leave it on **Auto-detect**), and download a genuine
binary `.xlsx` file — the same format Excel, Google Sheets, LibreOffice and Numbers open natively.
This is a real workbook, not a renamed CSV: numbers are written as **native number cells**, `true` /
`false` become **boolean cells**, the first row can be a **bold, frozen header**, and each column is
**auto-sized** to its content. Everything runs in your browser with WebAssembly — your data is never
uploaded to a server.

### Worked example

Input (CSV, with "Detect number & boolean types" on):

```
name,age,active
Alice,30,true
Bob,25,false
```

Output: a `People.xlsx` workbook whose first sheet has a bold, frozen header row
(`name` · `age` · `active`), then two data rows. In the resulting spreadsheet the **age** column
holds real numbers (`30`, `25`) you can sum or sort, and the **active** column holds real booleans —
not the text `"30"` or `"true"`. Set the **Sheet name** field to name the worksheet tab (it also
becomes the download's filename).

JSON works too — paste a JSON array of objects and the keys become the columns:

```
[{"product":"Widget","qty":12,"price":9.99},{"product":"Gadget","qty":3,"price":19.5}]
```

## FAQ

<details>
<summary>Is this a real .xlsx file, or just a CSV renamed to .xlsx?</summary>

It is a real, standards-compliant Office Open XML (`.xlsx`) workbook — a ZIP of XML parts, exactly
what Excel writes. It opens without the "the file format doesn't match the extension" warning you get
from a renamed CSV, and it carries native cell types (numbers, booleans) plus formatting like the
bold, frozen header row.

</details>

<details>
<summary>Do numbers stay numeric, and are leading zeros preserved?</summary>

With **Detect number & boolean types** on (the default), delimited cells that look like whole numbers
or decimals become native number cells, and `true` / `false` become booleans — so totals, sorts and
charts work. Values with a leading zero (`007`), a leading `+`, or underscores stay **text**, so zip
codes and phone numbers aren't mangled. Turn the option off to force **every** delimited cell to text.
JSON and NDJSON keep whatever types they already declare.

</details>

<details>
<summary>Which input formats and delimiters are supported?</summary>

CSV (comma), TSV (tab), semicolon-separated, and pipe-separated delimited text, plus a JSON array of
objects/rows and NDJSON (one JSON value per line, a.k.a. JSONL). **Auto-detect** sniffs the format
from your text, picking the most frequent of comma / tab / semicolon / pipe on the first row (or JSON
when the text starts with `[` or `{`). Pick a specific format from the dropdown to override the guess.

</details>

<details>
<summary>How are the columns and the header row decided?</summary>

For delimited input with **First row is a header** on, the header cells become the bold, frozen top
row. For a JSON array of objects the column set is the **union of all keys** in first-seen order, so
rows with different fields still line up (missing values are left blank). Turn the header option off to
write only data rows with no header.

</details>

<details>
<summary>Are there any size or shape limits?</summary>

A worksheet can hold at most 1,048,576 rows and 16,384 columns (Excel's own limits) — going over
returns a clear error. The workbook is capped at 24 MB. Nested JSON values (arrays/objects inside a
cell) are written as their compact JSON text so nothing is silently dropped. Everything is computed
locally in your browser, so very large pastes are bounded by your device's memory.

</details>
