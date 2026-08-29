## About this radar chart generator

A radar chart, also called a spider or web chart, compares one or more entities
across the same set of numeric axes. Each value becomes a point on a spoke; the
points for a series are joined into a polygon. This makes radar charts useful for
scorecards, product comparisons, assessment rubrics, team capability maps, and
balanced trade-off views where the shape matters as much as any one number.

Paste a wide table such as `product,Camera,Battery,Speed` followed by one row per
series, long rows such as `series,axis,value`, or a single `axis,value` list. The
default shared scale keeps magnitudes comparable across axes. Use **Normalize per
axis** only when the axes use different units, and **0–100 percent** for scores
that are already percentages.

### Worked example

```text
product,Camera,Battery,Speed,Price
Phone A,8,7,9,6
Phone B,6,9,7,8
```

Set the title to `Phone comparison`, keep the default shared scale, and choose
SVG output. The chart overlays one polygon per phone, with a legend and native
SVG tooltips on the vertex markers.

### Limits and edge cases

- A radar chart needs at least 3 axes. Use a bar chart for one or two measures.
- Up to 5,000 pasted rows, 60 axes, and 24 series are accepted; charts are most
  readable around 3–8 axes and 2–4 series.
- Missing cells in long format are treated as the scale floor and marked as
  `missing` in summary output.
- `scale_max = 0` means auto-scale; otherwise it must be greater than
  `scale_min`.
- PNG/JPEG export is intentionally out of scope for this pure local block; SVG is
  deterministic and can be converted by downstream tools if needed.

## FAQ

<details>
<summary>When is a radar chart a good choice?</summary>

Use it when every item is measured on the same set of axes and you want a compact
shape comparison: product feature scores, candidate rubrics, team maturity, game
stats, or risk profiles. If exact ranking on one metric is the main task, a bar
chart is usually clearer.

</details>

<details>
<summary>Should I use shared, per-axis, or percent scaling?</summary>

Use **shared** when all axes are on the same unit or comparable score range; it is
the default because it avoids misleading shapes. Use **per-axis** when axes mix
units such as revenue, rating, and uptime. Use **percent** when every value is
already a 0–100 score.

</details>

<details>
<summary>Why does the tool require at least three axes?</summary>

With one or two axes there is no meaningful polygon, so the visual becomes a line
or a pair of points. Three axes is the minimum shape for a radar chart; more than
about eight axes can become crowded, even though the hard cap is higher.

</details>

<details>
<summary>Can I compare many series at once?</summary>

The tool accepts up to 24 series, but radar charts are easiest to read with two
to four overlapping polygons. For larger comparisons, use summary or JSON output
or split the data into several smaller charts.

</details>
