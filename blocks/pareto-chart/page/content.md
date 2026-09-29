## About this Pareto chart generator

A Pareto chart combines sorted bars with a cumulative-percentage line. It helps
answer the classic 80/20 question: which few categories explain most of the
impact? Use it for customer complaints, defect causes, downtime reasons, support
queues, spend categories, incident classes, or any non-negative category/value
list.

Paste one `label,value` row per line. The tool detects common delimiters, skips a
header row when present, sums duplicate labels, sorts largest-first by default,
and marks the bars up to the threshold crossing as the vital few. Output can be a
self-contained SVG chart, an aligned summary table, or JSON for downstream checks.

### Worked example

```text
Reason,Count
Late delivery,45
Wrong item,30
Damaged,15
Billing error,7
Rude staff,3
```

With the default 80% threshold, the chart highlights the leading causes through
the category that crosses 80%, then continues the cumulative line to 100%.

### Limits and edge cases

- Up to 10,000 pasted rows and 500 distinct categories before optional tail
  bucketing.
- Values must be finite, zero or positive numbers; an all-zero table is rejected.
- `max_categories` rolls the tail into an `Other` bucket after sorting.
- `threshold = 0` hides the threshold line and vital-few highlighting.
- Rotated labels help with long category names, but very dense charts are still
  easier to read as summary or JSON.

## FAQ

<details>
<summary>What does the 80% threshold mean?</summary>

It is the cumulative share cutoff used to separate the vital few from the trivial
many. The highlighted set includes the row that first reaches or passes the
threshold, because that row is needed to explain the selected share of the total.

</details>

<details>
<summary>Why are my bars sorted differently from the pasted rows?</summary>

The default Pareto view sorts categories from largest to smallest so the
cumulative line shows the biggest contributors first. Choose **Input order** if
you already sorted the data elsewhere or need to audit the original sequence.

</details>

<details>
<summary>How should I handle a long tail of small categories?</summary>

Set **Max categories** to a readable number such as 8 or 10. Remaining categories
are summed into one bucket named by **Other bucket label**, so the percentages and
total stay honest without crowding the chart.

</details>

<details>
<summary>Can percentages or currency values be pasted directly?</summary>

Yes. Values such as `$1,250`, `1 250`, and `12%` are parsed as numbers. The chart
uses their numeric magnitudes, so make sure all rows share a comparable unit.

</details>
