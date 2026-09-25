## About this treemap chart generator

A treemap shows part-to-whole relationships with nested rectangles: each tile's
area is proportional to its value, and parent groups contain the tiles beneath
them. Use this tool for compact breakdowns such as disk usage, budgets,
portfolio weights, support-ticket categories, product revenue, or page views by
section.

Paste simple `label,value` rows for a flat chart, path rows such as
`src/ui/app.rs,860` for a hierarchy, or a grouped table such as
`region,city,value`. Duplicate paths are aggregated before layout. The default
`squarified` tiling creates near-square tiles for easier comparison; `slice_dice`
preserves a stripe order, and `binary` recursively splits the space.

### Worked example

Paste this data and set **Show percentages** to true:

```text
label,value
Documents,4200
Photos,3100
Videos,2400
Music,900
Archives,500
```

The largest rectangle represents Documents, followed by Photos and Videos. Add a
title such as `Storage by folder`, switch the palette, or set **Keep top N per
level** to fold smaller entries into `Other` when the chart becomes crowded.

### Limits and edge cases

- Up to 20,000 pasted rows and 5,000 rendered leaf tiles.
- Values must be finite numbers greater than or equal to zero; at least one value
  must be positive.
- A first header row is skipped when its last column is not numeric.
- `max_depth` caps nesting at 12 levels; deeper path segments are aggregated into
  their ancestor.
- Output is deterministic SVG, a summary table, or JSON geometry. PNG export is
  intentionally out of scope for this pure local block.

## FAQ

<details>
<summary>When should I use a treemap instead of a pie or bar chart?</summary>

Use a treemap when you have many categories or a hierarchy and want a compact
part-to-whole view. A pie chart gets hard to read past a few slices, while a bar
chart is better when exact rank comparison matters more than filling a limited
space.

</details>

<details>
<summary>What input formats does the tool accept?</summary>

For a flat chart, paste rows like `Label,123`. For a path hierarchy, paste rows
like `Parent/Child,123` and keep the separator as `/` or change it to match your
data. For grouped tables, every column except the last becomes a nesting level,
for example `Region,City,Value`.

</details>

<details>
<summary>Why did some labels disappear?</summary>

Labels are only drawn when the tile is large enough for the text. Increase the
canvas size, lower the font size, group small entries with **Keep top N per
level**, or switch to summary/JSON output if you need every exact value.

</details>

<details>
<summary>Can I use this for disk usage or folder-size data?</summary>

Yes. Export paths and byte counts as `folder/file,value`, choose the path layout,
and optionally set a top-N limit so tiny files are grouped into `Other` rather
than creating unreadable slivers.

</details>
