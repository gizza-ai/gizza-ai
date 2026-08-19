## About this tool

Histogram Chart Maker turns a pasted column of numbers into a histogram: it groups the values into bins and draws one bar per bin. Use it to see the shape of a distribution — response times, test scores, prices, measurement error, file sizes — before you commit to a dashboard or a report.

Paste values one per line, or separated by commas, tabs, semicolons, or spaces. A leading header row of text is ignored, so a column copied straight out of a spreadsheet works as-is.

Bin edges come from whichever method suits the data:

- **Auto** takes the finer of Freedman-Diaconis and Sturges — a good default for most data.
- **A named rule** — Sturges, Scott, Freedman-Diaconis, Rice, Doane, or square-root — when you need to match a textbook or a statistics package.
- **An exact bin count**, when you want, say, exactly 10 bars.
- **An exact bin width**, when the bins have to mean something — 10-point score bands, 5 ms latency buckets, decade-wide age groups.

Bars can measure raw counts, relative frequency, percent, density, or a running cumulative count or percent. Switch `output` from `svg` to `table`, `csv`, or `json` when you want the frequency table and summary statistics instead of the picture.

### Worked example

Paste these 20 latency measurements:

```
12
15
15
17
18
18
19
21
24
28
31
33
34
34
36
39
41
44
48
55
```

Set `bin_method=width`, `bin_width=10`, `range_min=0`, `range_max=60`, and `output=table`. The range is fixed at 0–60 rather than the data's own 12–55, so the bins land on round numbers. You get six bins and this frequency table:

```
Bin        Count  Percent  Cumulative %  Count
---------  -----  -------  ------------  -----
[0, 10)    0      0        0             0
[10, 20)   7      35       35            7
[20, 30)   3      15       50            3
[30, 40)   6      30       80            6
[40, 50)   3      15       95            3
[50, 60]   1      5        100           1
```

Switch `output` back to `svg` for the same six bars as a chart. The `[10, 20)` bin is the tallest at 7 of 20 values (35%), and the last bin is written `[50, 60]` because the top bin always closes on the upper edge so the maximum has somewhere to go.

### Limits and edge cases

- Input is capped at 100,000 numbers, and at least 2 are needed. Bin count is capped at 500; an over-fine `bin_width` is rejected with the bin count it would have produced, rather than silently truncated.
- Bins are left-closed `[a, b)` by default, with the final bin closing on the maximum. Set `right_closed` for `(a, b]` intervals, where the first bin instead closes on the minimum.
- `range_min` / `range_max` clip the chart: values outside the range are excluded from every count and reported separately as `excluded` in the `table` and `json` output.
- If every value is identical the range is widened slightly so the chart still has an axis.
- The normal-curve overlay is skipped in the two cumulative modes, where a density curve has no meaning.
- Output is a standalone, deterministic SVG — no fonts, images, scripts, or external plotting libraries are loaded, and the same input always produces byte-identical markup.

## FAQ

<details>
<summary>How many bins should I use?</summary>

Start with `bin_method=auto`, which picks the finer of Freedman-Diaconis and Sturges, then adjust. Too few bins hide structure such as a second peak; too many turn the chart into noise, with most bars at a count of one. Freedman-Diaconis uses the interquartile range, so it copes well with skewed data and outliers; Sturges assumes roughly normal data and is gentle on small samples; Doane corrects Sturges for skew. If the bins have to carry meaning to a reader — score bands, price brackets, 5 ms buckets — use `bin_method=width` with an exact width instead of any rule.

</details>

<details>
<summary>What is the difference between count, percent, and density?</summary>

`count` plots the raw number of values in each bin. `relative` plots that as a fraction of the total and `percent` as a percentage, which lets you compare two datasets of different sizes on the same scale. `density` divides by both the total and the bin width, so the bars integrate to 1 — that is the mode to use when you want to overlay the normal curve, or to compare histograms drawn with different bin widths. `cumulative_count` and `cumulative_percent` plot a running total instead, which answers "how many values are at or below this point".

</details>

<details>
<summary>Which bin does a value sitting exactly on an edge go into?</summary>

By default bins are left-closed, written `[a, b)`: a value equal to `b` belongs to the next bin up, and only the final bin includes its upper edge so the maximum is counted. That is the convention numpy, pandas, and most statistics packages use. Turn on `right_closed` for the opposite convention `(a, b]`, common in classroom and spreadsheet examples, where the edge value falls in the lower bin and the first bin includes the minimum. With 0, 5, 10 split into two bins, the default puts 5 in the upper bin and `right_closed` puts it in the lower one.

</details>

<details>
<summary>Can I fix the axis range instead of using the data's minimum and maximum?</summary>

Yes — set `range_min` and `range_max`. The bins are then laid out across that range rather than from the smallest to the largest value, which is how you get round edges like 0–100 instead of 12.3–98.7, and how you draw two histograms on the same axis so they can be compared. Values outside the range are dropped from the counts and reported as `excluded` in the `table` and `json` output, so a clipped outlier is never silently lost.

</details>

<details>
<summary>How do I save the chart or reuse the numbers?</summary>

The default `svg` output is plain SVG markup. Save it to a `.svg` file, drop it into HTML or a Markdown workflow that allows inline SVG, or open it in a vector editor — it is self-contained, so nothing loads from the network. When you want the numbers rather than the picture, `table` gives a readable frequency table with summary statistics (n, min, max, mean, median, standard deviation, quartiles), `csv` gives the same table with one row per bin for a spreadsheet, and `json` gives the bins plus stats for a script.

</details>
