## About this tool

The interquartile range (IQR) method — **Tukey's fences** — is the textbook way to
decide which rows of a dataset are outliers without assuming the data is normally
distributed. For the column you pick it computes the first quartile **Q1**, the third
quartile **Q3** and **IQR = Q3 − Q1**, then treats any value below

```
Q1 − k · IQR      (lower fence)
```

or above

```
Q3 + k · IQR      (upper fence)
```

as an outlier. `k = 1.5` is Tukey's classic *mild* fence (the whisker length of a box
plot); `k = 3` keeps only the *extreme* outliers. Whole rows are then removed, so the
result is still a valid table you can paste straight back into a spreadsheet.

### A worked example

Input (with `price` as the analysed column, `k = 1.5`):

```
name,price
a,10
b,11
c,12
d,13
e,100
```

The five prices sort to `10 11 12 13 100`, giving `Q1 = 11`, `Q3 = 13`, `IQR = 2` and
fences `11 − 1.5·2 = 8` … `13 + 1.5·2 = 16`. Only `100` is outside, so its row goes:

```
name,price
a,10
b,11
c,12
d,13
```

Switch **Output** to the report and you get the same numbers written out — quartiles,
both fences, and how many rows were flagged — instead of the table.

### The four actions

- **Remove** — drop the outlier rows (the default; this is the "trim").
- **Keep only them** — the inverse selection, so you can eyeball what *would* be
  dropped before committing to it.
- **Clip (winsorize)** — leave every row in place but clamp each out-of-fence cell to
  its own fence, so `100` becomes `16` in the example above. Useful when losing rows
  would break a paired dataset.
- **Flag** — append an `outlier` column of `true`/`false` and drop nothing.

### Choosing columns

Leave **Columns** blank and every column whose values are all numeric is fenced;
name one (or several, comma-separated) to be explicit. Columns are matched by
header name, or by 1-based index when the header option is off. With more than
one column selected, **any** means a row goes as soon as one of its cells is out
of fence, **all** means every selected cell must be.

### Limits

- Up to **5,000 data rows** per run — split larger files into batches.
- Quartiles need at least one numeric value in the column; with a single value the
  IQR is 0, so the fences collapse onto that value and nothing is trimmed.
- Blank and non-numeric cells never take part in the quartile maths. Whether their
  *rows* survive is the **Blank / non-numeric cells** setting.
- Only the IQR method is offered here. For z-score or modified-z-score (MAD)
  detection over a list of numbers, use the outlier-detector tool; to see the box
  plot the fences come from, use the box-plot chart tool.

## FAQ

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The whole calculation is compiled to WebAssembly and runs inside your browser
tab — the CSV never leaves your device, and the page works offline once loaded.

</details>

<details>
<summary>Which quartile definition does it use?</summary>

By default **linear interpolation** between order statistics — what
`numpy.percentile`, `pandas.quantile` and Excel's `QUARTILE.INC` do. Two other
conventions are selectable, because textbooks disagree: **exclusive** (Moore &
McCabe / TI-83) splits the sorted values at the median and leaves the median out of
both halves when the count is odd, and **inclusive** (Tukey's hinges) puts the
median in both halves. On an odd-length dataset they can give different quartiles —
`1…9` gives `Q1 = 3, Q3 = 7` under linear and `Q1 = 2.5, Q3 = 7.5` under exclusive —
so pick the one your reference tool uses.

</details>

<details>
<summary>Why does my row survive even though the value looks extreme?</summary>

The fences are derived from the data itself, so a "big" number is only an outlier
relative to that column's spread. If a quarter of the values are large, the IQR is
large too and the fences move out with it. Lower `k` toward 0 to tighten them, or
switch to the report output to see exactly where the fences landed.

</details>

<details>
<summary>Can I trim on more than one column at once?</summary>

Yes — list them comma-separated. Each column gets its own quartiles and its own
fences (they are never pooled), and the **any / all** setting decides whether one
out-of-fence cell is enough to drop the row or whether every selected cell has to
be out of fence.

</details>

<details>
<summary>What happens to the header row and to other delimiters?</summary>

With *first row is a header* on, that row is copied through untouched and is never
fence-tested; its names are what the **Columns** box matches. Tab-, semicolon- and
pipe-separated files are supported, and the output is written back with the same
delimiter, quoting only the fields that need it.

</details>
