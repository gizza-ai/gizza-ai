## About this tool

Boxplot Chart Generator renders box-and-whisker plots from pasted numeric data. Use it for quick distribution checks: compare latency by region, scores by class, order values by category, or spot outliers before you build a dashboard.

The tool accepts three common data shapes:

- A single list of values, one per line or separated by commas.
- A tidy table such as `group,value`, where one column names the group and one column holds the number.
- A wide table, where each numeric column becomes its own box.

By default it uses linear quartiles, Tukey whiskers at `1.5 × IQR`, outlier markers, a mean marker, and a light SVG chart. Switch `output` to `summary` for a text table or `json` for scriptable stats.

### Worked example

Paste this data:

```csv
group,value
A,1
A,2
A,3
A,4
B,5
B,6
B,7
B,20
```

Keep `layout=auto`, `whiskers=tukey`, and `points=outliers`. Group B's value `20` sits beyond the Tukey fence, so it is drawn as an outlier point while the whisker stops at the highest non-outlier value.

### Limits and edge cases

- Input is capped at 100,000 numeric values and 60 groups to keep browser runs responsive.
- Empty cells in wide tables are ignored; non-numeric cells in value columns produce a line-numbered error.
- `percentile` whiskers use the lower percentile and `100 - percentile` for the upper whisker.
- The SVG is deterministic and self-contained; no fonts, images, or external plotting libraries are loaded.

## FAQ

<details>
<summary>Which quartile method should I choose?</summary>

Use `linear` when you want percentile-style interpolation, which is common in charting tools and spreadsheets. Use `inclusive` or `exclusive` when you need to match a specific statistics package or classroom convention. The `summary` and `json` outputs make it easy to compare the resulting Q1, median, and Q3 values.

</details>

<details>
<summary>Why does an outlier not move the whisker all the way to the maximum?</summary>

With `whiskers=tukey`, whiskers stop at the most extreme value still inside `Q1 - k×IQR` and `Q3 + k×IQR`. Values outside those fences are outliers and are drawn separately. Choose `whiskers=minmax` if you want whiskers to span the full data range.

</details>

<details>
<summary>How do I plot grouped CSV data?</summary>

Paste a table with one group column and one numeric value column, such as `region,latency_ms`. `layout=auto` usually detects this; if your headers are unusual, set `layout=long`, `group_column=region`, and `value_column=latency_ms` explicitly.

</details>

<details>
<summary>Can I use the output in documentation or a report?</summary>

Yes. The default output is plain SVG markup, so you can save it as an `.svg` file, paste it into HTML/Markdown workflows that allow SVG, or switch to `summary`/`json` when you need the computed statistics instead of the chart image.

</details>
