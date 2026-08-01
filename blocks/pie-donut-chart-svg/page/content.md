## Turn labeled values into a pie or donut chart

Enter a short list of `label, value` pairs and generate a standalone SVG pie or donut chart you can copy, save, or drop into a slide, doc, or README. Percentages are computed automatically from the sum of the values, so you only supply the raw numbers. The chart is drawn with a legend, an optional title, and your choice of colors — as clean, scalable vector graphics with no external dependencies.

Values can be separated by `,`, `:`, or `=`, one entry per line (or `;`-separated), and a JSON array such as `[["Chrome",63],["Safari",20]]` is accepted too. Only zero-or-positive numbers make sense on a pie, so negative or non-numeric values are rejected with a clear error.

### Worked example

Input:

```
Chrome, 63
Safari, 20
Edge, 5
Firefox, 3
Other, 9
```

Use **Chart type** = `pie`, **Legend** = `right`, **Slice order** = `Largest first`, and **Chart title** = `Browser market share`. The output SVG draws one wedge per browser sized by its share of the total, the percentage on each slice, and a legend down the right. Switch **Chart type** to `donut` and raise **Donut hole size** to `0.6` for a ring, or move the **Legend** to `bottom` for a wide layout.

## FAQ

<details>
<summary>What's the difference between the pie and donut modes?</summary>

Both size each slice by its share of the total. A **pie** draws solid wedges from the center; a **donut** is the same chart with a round hole cut out of the middle, controlled by the **Donut hole size** (a fraction of the radius from `0.0` to `0.9`). The hole size only affects the donut mode.

</details>

<details>
<summary>Do I need to enter percentages myself?</summary>

No. Enter the raw values — counts, dollars, votes, anything — and the tool computes each slice's percentage from the sum automatically. Turn **Show percentages** on to print `%` on each slice and in the legend, and **Show raw values** to add the original numbers to the legend rows.

</details>

<details>
<summary>Can I control the colors and the slice order?</summary>

Yes. Leave **Colors** blank to use a built-in 10-color palette, or pass a comma-separated list of CSS colors (`#4e79a7, tomato, rgb(20,120,200)`) that cycles across the slices. **Slice order** keeps your input order or sorts the slices largest- or smallest-first, and **Start angle** rotates where the first slice begins (clockwise from the top).

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. Parsing and SVG generation run entirely in your browser through WebAssembly. Nothing is sent to a server, so the tool works offline and your numbers never leave your machine.

</details>
