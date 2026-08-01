## Bin a numeric column of a CSV into buckets

Paste a CSV, pick one numeric column, and this tool sorts each row into a
**bucket** and labels it — locally in your browser, **nothing is uploaded**.
Choose how the buckets are drawn:

- **Equal-width** — split the column's value range into buckets of the same
  width. `bins` sets how many (default `4` = quartiles). Simple to read, but a
  few extreme values can leave some buckets nearly empty.
- **Quantile** — draw the edges so each bucket holds roughly the **same number
  of rows** (equal-frequency). Preferred for skewed data. Duplicate edges from
  repeated values are merged automatically, so you may get fewer buckets than
  requested.
- **Custom edges** — supply your own strictly-ascending boundaries (like
  `0,18,65,120`). Values below the first or above the last edge get a blank
  label.

Label each bucket yourself (**Custom bucket labels**, one per bucket) or let the
tool auto-label them as an **interval range** (like `(50, 75]`) or a 1-based
**bucket index**. **Right-closed intervals** decides whether a boundary value
falls in the lower or upper bucket, and **Output mode** either appends a new
`<column>_bin` column or replaces the source column with the label.

### Worked example

With the default **Equal-width** method and `4` buckets on the `score` column,
this input:

```
name,score
Ann,0
Bo,40
Cy,70
Di,100
```

becomes:

```
name,score,score_bin
Ann,0,"[0, 25]"
Bo,40,"(25, 50]"
Cy,70,"(50, 75]"
Di,100,"(75, 100]"
```

`score` runs `0`–`100`, so four equal-width buckets have edges `0, 25, 50, 75,
100`. `40` lands in `(25, 50]` and `70` in `(50, 75]`. The first bucket is
written `[0, 25]` because the lowest edge is always included; interval labels
that contain a comma are quoted by the CSV writer.

### Limits & edge cases

- The chosen column must be **numeric** — every present value has to parse as a
  finite number, or the run errors. `NaN` and `inf` count as non-numeric.
- **Blank cells stay blank**: an empty target cell gets no label and is left out
  of the edge statistics.
- With **Custom edges**, a value outside the first/last edge gets a **blank**
  label rather than being forced into an end bucket.
- **Quantile** binning can merge duplicate edges on skewed or repeated data, so
  the result may have fewer buckets than `bins`.
- A **constant** column (all values equal) yields a single bucket.
- **Custom bucket labels** must supply exactly one label per bucket, or the run
  errors — handy because quantile merging can change the bucket count.
- Turn off **First row is a header** for files with no header row; then the
  column selector must use a 1-based index and row 1 is binned like any other.
- Everything runs in memory, so very large files are bounded by your browser's
  available memory.

### FAQ

<!-- FAQ MUST be <details>/<summary> accordions with a blank line inside each. -->

<details>
<summary>What is the difference between equal-width and quantile binning?</summary>

**Equal-width** cuts the value range into buckets that each span the same
distance — with `bins = 4` over `0`–`100` you get `0–25`, `25–50`, `50–75`,
`75–100` regardless of how many rows land in each. **Quantile** (equal-frequency)
instead moves the edges so every bucket holds roughly the same *number of rows*;
it's the better choice for skewed data because no bucket ends up nearly empty.

</details>

<details>
<summary>How do I set my own bucket boundaries?</summary>

Choose **Custom edges** as the method and type strictly-ascending, comma-separated
numbers under **Custom edges** — for example `0,18,65,120` makes the buckets
`[0, 18]`, `(18, 65]`, `(65, 120]`. Any value below the first edge or above the
last one gets a blank label. Pair it with **Custom bucket labels** (like
`child,adult,senior`) to name each band.

</details>

<details>
<summary>What do the labels like "(50, 75]" mean?</summary>

They are interval notation for the bucket's boundaries. A square bracket `[` or
`]` means the endpoint is **included**; a round bracket `(` or `)` means it is
**excluded**. So `(50, 75]` covers values greater than `50` up to and including
`75`. Toggle **Right-closed intervals** off to flip this to `[50, 75)`, or switch
**Auto-label style** to *Bucket index* for a plain `1`, `2`, `3`.

</details>

<details>
<summary>Which value goes into which bucket at a boundary?</summary>

A value exactly on an edge follows the **Right-closed intervals** setting. When it
is on (the default), intervals are `(a, b]`, so a boundary value falls in the
**upper** bucket — except the very lowest edge, which is always included. Turn it
off for `[a, b)` intervals, where a boundary value falls in the **lower** bucket
and the very highest edge is included.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The whole computation runs locally with WebAssembly; your CSV never leaves
your browser.

</details>
