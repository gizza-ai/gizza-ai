## About this tool

Use this comparator when you have a baseline classifier and a candidate classifier and want to know exactly what changed. Paste each confusion matrix as a square count grid, as raw `actual,predicted` rows, or as `actual,predicted,count` triples. The tool aligns class names, computes every metric for A and B, then reports `candidate − baseline` deltas.

The output is designed for model-regression checks: a new model can improve accuracy while hurting a minority class. The per-class table shows support, precision, recall and F-score for both versions; the entrywise delta grid shows which actual/predicted cells moved; the biggest-movers section calls out gains and regressions so you do not have to scan every row by eye.

### Worked example

Baseline matrix:

```text
actual,cat,dog,fox
cat,42,5,3
dog,6,38,6
fox,4,7,39
```

Candidate matrix:

```text
actual,cat,dog,fox
cat,46,3,1
dog,4,43,3
fox,9,11,30
```

The candidate improves the cat and dog diagonals but makes more fox errors. Sorting by **Biggest F-score loss first** puts that regression at the top, while the entrywise grid shows the exact `B − A` count shifts.

### Input shapes

- **Matrix:** a square K×K count grid. Rows are actual classes and columns are predictions by default.
- **Label pairs:** one observation per row, such as `cat,dog` for actual cat predicted as dog.
- **Count triples:** tallied rows like `actual,predicted,count`.
- Headers and row labels are detected automatically, but you can force matrix/table/label mode, separator, header handling and orientation.

### Limits and assumptions

- Counts must be whole numbers; row-normalised percentages are rejected because entrywise count deltas and support totals would be misleading.
- Both matrices must describe the same set of classes. If names appear in a different order, the candidate is reordered to match the baseline.
- Up to 50 classes and 50,000 pasted rows per input are accepted.
- The accuracy significance test is an **unpaired** two-proportion z-test. If both models scored the same test items, a paired McNemar test would be better, but it cannot be reconstructed from two aggregate confusion matrices alone.
- All calculations run locally in your browser.

## FAQ

<details>
<summary>Which direction is the delta?</summary>

Every delta is `candidate − baseline`, so a positive precision, recall, F-score or accuracy delta means the candidate improved that metric. For the entrywise grid, a positive diagonal cell is usually good, while a positive off-diagonal cell means the candidate made more of that specific mistake.

</details>

<details>
<summary>What if my matrix has rows as predictions and columns as actual classes?</summary>

Set **Which axis is the true class** to **Columns are actual, rows predicted**. The matrix is transposed before metrics are computed. This matters because the wrong orientation swaps precision and recall.

</details>

<details>
<summary>Can this compare two models evaluated on the same examples?</summary>

It can compare their aggregate matrices and metric deltas, but the p-value is conservative for paired data. A true paired significance test needs the per-example disagreement table — for example, how many examples model A got right while model B got wrong — which is not available in ordinary confusion matrices.

</details>

<details>
<summary>Why do some classes show a zero precision or recall?</summary>

When a denominator is zero, the rate is undefined. The report follows the common scikit-learn convention of printing `0` so deltas remain sortable, and it also adds a note naming the classes and metrics that were undefined so the zero is not silent.

</details>

<details>
<summary>Should I sort by F-score gain or regression?</summary>

Use **Biggest F-score gain first** when you are looking for improvements. Use **Biggest F-score loss first** during release checks: it puts the class most harmed by the candidate at the top, which is often more important than a small overall accuracy gain.

</details>
