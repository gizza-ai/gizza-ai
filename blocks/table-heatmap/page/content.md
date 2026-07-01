## About this tool

**Table heatmap** applies spreadsheet-style **color-scale conditional formatting**
to a CSV/table and gives you a styled **HTML `<table>`** with shaded numeric cells —
the kind of "heat map" you'd build in Excel or Google Sheets, ready to paste into a
report, email, or web page.

- Every cell that parses as a **number** is shaded by its value on the chosen color
  scale. Numbers with thousands commas, a `$`/`€`/`£` sign, a trailing `%`, or
  accounting-style `(parentheses)` negatives are all recognized.
- **Text** cells and the **header** row are left plain so labels stay readable.
- Pick a **color scale**: red→yellow→green (default), green→yellow→red, the diverging
  blue→white→red, or a simple white→green / white→blue.
- **Scale each column independently** (default) so every column gets its own min→max
  range, or turn it off to color the whole table on one global scale.
- Pin the **minimum**, **maximum**, and (for diverging scales) the **midpoint** to fixed
  values — like Excel's number-type color-scale anchors — instead of the data range.
- Cell text color (black or white) is chosen automatically for contrast.

### Privacy

Everything runs **in your browser** via WebAssembly — your data is never uploaded.
Also available from the [gizza CLI](/) and in chat.

### Common uses

- Turn a metrics or sales spreadsheet into a color-coded HTML table for a dashboard or report.
- Highlight highs and lows in a comparison table at a glance.

## FAQ

<details>
<summary>Which cell values are recognized as numbers?</summary>

Anything that parses numerically after stripping formatting: thousands commas
(`1,234.5`), a leading `$`, `€`, or `£`, a trailing `%`, and accounting-style
`(500)` negatives. Cells that don't parse — labels, dates, blanks — are left
unshaded, as is the header row when *header* is on (the default).

</details>

<details>
<summary>Should I scale per column or across the whole table?</summary>

Per-column (the default) gives each column its own min→max range, so a
"revenue in millions" column and a "percent growth" column both use the full
color range — that's what Excel does per selection. Turn it off for one global
scale when all columns share a unit. Note that pinning **both** min and max
overrides per-column scaling entirely.

</details>

<details>
<summary>What does the midpoint setting do?</summary>

It only applies to the diverging scales (red→yellow→green, green→yellow→red,
blue→white→red): the value you pin is mapped to the neutral middle color, with
values below and above fanning out toward the two ends. Left unset, the
midpoint sits halfway between min and max. Sequential scales (white→green,
white→blue) ignore it.

</details>

<details>
<summary>Will numbers stay readable on dark cell colors?</summary>

Yes — each shaded cell's text is switched between near-black and white
automatically based on the background's luminance (a WCAG-style contrast
check), so values at the saturated ends of a scale remain legible.

</details>
