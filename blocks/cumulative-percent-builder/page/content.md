## About this tool

Cumulative Percent Builder turns a plain list of categories and values into a Pareto-ready table. Paste rows such as `issue,count`, choose whether to sort descending or keep the input order, and the tool adds percent of total, cumulative count, cumulative sum, cumulative percentage, and a vital/trivial label based on your threshold.

The default workflow matches the common 80/20 analysis: sort the largest values first, compute the running total, and mark every row through the first row that crosses 80% as the vital few. You can change the threshold, bucket the long tail into an `Other` row, and export the result as aligned text, CSV, or Markdown.

Worked example:

```text
issue,count
Scratches,400
Dents,250
Misalignment,150
Packaging,120
Other,80
```

With an 80% threshold, the top three rows account for 800 of 1000 total units, so they are marked vital and the remaining rows are marked trivial. The text output also includes a deterministic fixed-width Pareto chart for quick visual scanning.

Limits and conventions: values must be finite, non-negative numbers and the total must be greater than zero. The maximum paste is 10000 rows. Auto delimiter detection checks comma, tab, semicolon, and pipe before falling back to whitespace. Tail bucketing is opt-in because automatic Other rows can hide a high-impact category.

## FAQ

<details>
<summary>What does the vital-few label mean?</summary>

Rows are marked `vital` until the cumulative percentage first reaches or exceeds the threshold. With the default 80% threshold, the vital-few count is the number of largest categories needed to cover about 80% of the total.

</details>

<details>
<summary>Should I sort descending?</summary>

For Pareto analysis, yes. Descending sort ranks the largest contributors first before the cumulative percentage is computed. Keep input order only when your rows are already in a meaningful sequence and you want a running percentage through that sequence.

</details>

<details>
<summary>When should I use the Top N / Other option?</summary>

Use it when many tiny categories make the table hard to read and you deliberately want to combine the tail. Leave it at `0` when every original category should stay visible.

</details>

<details>
<summary>Can I paste spreadsheet data?</summary>

Yes. Copy two columns from a spreadsheet and leave delimiter on Auto, or explicitly choose Tab. Currency symbols, thousands separators, and underscore digit separators are tolerated in the value column.

</details>
