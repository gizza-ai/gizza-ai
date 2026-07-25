## About this tool

Table to SVG Image turns pasted CSV or JSON table data into a standalone SVG string you can save as `.svg`, paste into documentation, or rasterize with a browser/design tool when you need PNG or JPEG. It is useful for sharing small result tables, leaderboard snippets, status matrices, budget summaries, and dashboard callouts without opening a spreadsheet or screenshot tool.

The renderer supports CSV with custom delimiters, JSON arrays of flat objects, JSON arrays of arrays, and a single JSON object. You can choose a light, dark, slate, blue, green, or minimal theme; set the header/accent color; toggle zebra rows; add a centered title; and align cell text left, center, or right. Output is deterministic SVG text and is generated locally in your browser.

### Worked example

Paste this CSV:

```csv
name,score
Ada,9
Grace,8
Linus,7
```

Use the default light theme, title `Leaderboard`, and accent `#2563eb`. The output begins with a standalone `<svg ...>` element containing a colored header row, the three body rows, grid lines, and escaped text ready to save as `leaderboard.svg`.

### Limits and edge cases

- Output is SVG, not PNG/JPEG. SVG is crisp and easy to convert, but this tool intentionally does not bundle a raster encoder.
- JSON tables should be flat: array-of-objects keys become columns; nested arrays/objects are stringified into cells.
- Very long cell values are measured with a capped width estimate so a single field does not create an unusably wide image.
- CSV parsing supports quoted fields and custom one-character delimiters, including `\t` or `tab` for tab-separated data.

## FAQ

<details>
<summary>Can this export PNG or JPEG directly?</summary>

No. The tool emits a standalone SVG string because SVG is small, sharp at any scale, and safe to generate in WebAssembly without adding a raster image encoder. Save the output as `.svg`; browsers, design tools, and command-line converters can rasterize it to PNG or JPEG when needed.

</details>

<details>
<summary>What JSON shapes are supported?</summary>

Use an array of flat objects such as `[{"name":"Ada","score":9}]`, an array of arrays such as `[["name","score"],["Ada",9]]`, or a single flat object. For object arrays, the union of first-seen keys becomes the header row. Nested JSON values are kept by stringifying them into a cell.

</details>

<details>
<summary>How do headers work for CSV and JSON?</summary>

For CSV and JSON arrays of arrays, the “First row is header” option controls whether the first row becomes a bold styled header. For JSON arrays of objects, object keys always become the header because the values are already named fields.

</details>

<details>
<summary>Is my table uploaded anywhere?</summary>

No. Parsing and SVG rendering happen locally in the browser and in the pure Rust core used by the CLI. The tool does not require a network request to render your table.

</details>
