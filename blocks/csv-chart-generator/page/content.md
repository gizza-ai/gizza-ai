## Turn CSV columns into a clean SVG chart

Paste a small CSV table, choose the columns to plot, and generate a standalone SVG chart you can copy, save, or drop into documentation. The tool supports **bar charts**, **line charts**, **scatter plots**, and **histograms**. Header names are matched case-insensitively; if your CSV has no header row, use 1-based column indexes such as `1` and `2`.

### Worked example

Input:

```csv
month,revenue
Jan,1200
Feb,1800
Mar,1500
Apr,2400
May,2100
```

Use **Chart type** = `bar`, **X column** = `month`, **Y column** = `revenue`, and **Chart title** = `Monthly revenue`. The output SVG draws labeled axes, one bar per month, and a title. Switch to `line` for a trend line, `scatter` for numeric X/Y pairs, or `histogram` to bin one numeric column.

## FAQ

<details>
<summary>What chart types can I create?</summary>

You can render bar charts for categorical X values, line charts for ordered or numeric X values, scatter plots for numeric X/Y pairs, and histograms that bin one numeric column. Each output is a plain SVG string with no external dependencies.

</details>

<details>
<summary>How do I choose columns?</summary>

If the first row is a header, enter the header name exactly or case-insensitively, such as `month` or `revenue`. If the CSV has no header row, use 1-based indexes: `1` for the first column, `2` for the second, and so on.

</details>

<details>
<summary>Why do some values get skipped?</summary>

Rows with missing or non-numeric values in numeric columns are skipped because SVG coordinates require numbers. The chart still renders as long as at least one valid point or bin remains; otherwise the tool returns a clear error.

</details>

<details>
<summary>Is my CSV uploaded anywhere?</summary>

No. CSV parsing and SVG generation run locally in WebAssembly. The chart is generated in your browser, and your data is not sent to a server.

</details>
