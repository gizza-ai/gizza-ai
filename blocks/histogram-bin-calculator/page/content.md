## About this tool

A histogram turns numeric data into ranges and counts, but the result changes a lot depending on the bin width. This calculator compares several common rules side by side — Sturges, Scott, Freedman-Diaconis, Rice, and square-root — then applies the rule you choose and prints the resulting bin edges, counts, percentages, and optional cumulative or density columns.

Use it when you have a pasted column from a spreadsheet, experiment, log, or small dataset and want a principled starting point before drawing a histogram. Sturges is simple and often smooth for small samples, Scott uses the sample standard deviation, and Freedman-Diaconis uses the IQR so it is less sensitive to outliers.

### Worked example

Paste this dataset:

```text
1,2,2,3,3,3,4,5,7,9,12
```

With the default `auto` rule, the output first shows the sample summary and every rule's recommended bin count and width. It then builds a histogram table for the selected rule, with interval labels, counts, percentages, and ASCII bars so you can see the distribution shape immediately.

### Input notes and limits

Numbers can be separated by newlines, commas, spaces, tabs, semicolons, or pipes. Use plain decimals or scientific notation such as `12`, `-3.5`, or `1.2e3`; strip currency symbols and thousands separators first. The tool accepts 2 to 100,000 finite values and caps manual/rule-generated bins at 1,000 rows.

## FAQ

<details>
<summary>Which bin rule should I use?</summary>

Start with `auto`, then compare the rule table. Sturges is often reasonable for small, roughly normal samples. Scott can work well for normal-ish data but is pulled by outliers. Freedman-Diaconis uses the IQR and is usually more robust for skewed data.

</details>

<details>
<summary>What does the auto rule do?</summary>

`auto` chooses the finer of Sturges and Freedman-Diaconis, similar to NumPy's default. It keeps Sturges from being too coarse while still falling back when the IQR rule is not informative.

</details>

<details>
<summary>Why do my bin edges look awkward?</summary>

Rule-derived widths are mathematical, so edges can land on values like `1.4286`. Turn on `nice_edges` to round the width up to a 1/2/2.5/5 × power-of-ten step and snap the first edge down when the range start is automatic.

</details>

<details>
<summary>What is density?</summary>

Density is `count / (n × bin_width)`, the bar height for a unit-area histogram. It is useful when comparing histograms with different bin widths or sample sizes.

</details>

<details>
<summary>How are edge values assigned?</summary>

By default bins are left-closed (`[a, b)`) and the final bin includes the maximum. Enable `right_closed` to use `(a, b]`, where values exactly on an edge fall into the lower bin.

</details>
