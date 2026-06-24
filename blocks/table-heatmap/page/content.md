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
