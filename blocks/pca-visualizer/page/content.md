## About this tool

High-dimensional tables are hard to inspect directly: six measurements per sample already means no ordinary x/y chart can show the whole row. This tool turns each row into one point in a two-dimensional scatter plot so clusters, outliers, and label separation become visible.

Use **PCA** when you want a linear projection with interpretable axes. The x and y captions show how much variance PC1 and PC2 explain, and the calculation is the same deterministic Jacobi PCA engine used by the sibling PCA calculator. Use **t-SNE** when the goal is a visual cluster map: it preserves local neighbours rather than global distances. This implementation is deterministic too — it starts from PCA scores instead of a random layout — so rerunning the same table produces byte-identical output.

One non-numeric column can carry a class, species, cohort, cluster, or other label. The tool drops that column from the math, uses it to colour the points, and draws a legend. Leave **Label column** empty to auto-detect the only text column, or name it explicitly if the table has several text columns or a numeric group code.

### Worked example

Paste this small labelled table:

```text
sepal_len,sepal_wid,petal_len,petal_wid,species
5.1,3.5,1.4,0.2,setosa
4.9,3.0,1.4,0.2,setosa
5.8,2.7,4.1,1.0,versicolor
6.4,3.2,4.5,1.5,versicolor
6.5,3.0,5.8,2.2,virginica
7.6,3.0,6.6,2.1,virginica
```

Keep **Projection method** as PCA, **Label column** as `species`, and the output as SVG. The result is a standalone scatter plot: every row is a coloured circle, the legend lists the three species, and the axes are labelled with the variance share explained by PC1 and PC2. Switching **Output** to CSV returns rows like `index,label,pc1,pc2`, ready to feed into another charting tool.

For a non-linear cluster map, switch **Projection method** to t-SNE. Start with perplexity around 3–10 for a tiny table, 30 for a few hundred rows, and increase **Iterations** if the layout still looks compressed.

### Limits and edge cases

- PCA accepts up to **5,000 rows** and **100 numeric columns**. t-SNE accepts up to **1,000 rows** because it compares every pair of points on every iteration.
- The input needs at least **3 rows** and **2 numeric variables** after the label column is removed.
- Separators can be comma, tab, semicolon, pipe, or whitespace. A header row is detected automatically.
- At most one non-numeric column can remain after label detection. If your table has IDs and labels, set **Label column** explicitly and remove or numeric-encode the ID column.
- **Standardize numeric columns** is on by default. Turn it off only when all variables are already comparable or when you intentionally want high-variance columns to dominate.
- t-SNE distances are useful for cluster inspection, not exact geometry. Do not interpret the absolute axis positions or distances between far-away clusters as measured quantities.
- **Draw text labels** is capped at 200 points; beyond that, labels overlap and make the SVG too large.
- The SVG output is plain markup. Save it as `.svg` to open it in a browser, vector editor, or report.

## FAQ

<details>
<summary>When should I use PCA instead of t-SNE?</summary>

Use PCA first when you want a fast, stable overview and axes that mean something: PC1 and PC2 are linear combinations of your original variables, and the captions report explained variance. Use t-SNE when you mostly care about whether nearby points form clusters. t-SNE can make local groups clearer, but its axes are not interpretable measurements.

</details>

<details>
<summary>How does the tool choose the label column?</summary>

If **Label column** is empty, the tool looks for a single non-numeric column and uses it as the label. That works for tables like `x,y,z,class`. If there are multiple text columns, or if the group column is numeric, set the label column by header name (`species`) or by 1-based index (`5`). The selected column is excluded from the projection and used only for colours and legend text.

</details>

<details>
<summary>Why does t-SNE look different from PCA?</summary>

PCA preserves the broad linear variance structure, so points that are globally far apart in the original variables tend to stay far apart. t-SNE optimizes neighbourhoods: points with similar neighbours are pulled close, and unrelated clusters are pushed apart for readability. That makes it great for cluster maps, but the exact distance between two separate clusters is not a reliable quantity.

</details>

<details>
<summary>Should I standardize the columns?</summary>

Usually yes. Without standardization, a column measured in thousands can dominate a column measured between 0 and 1, even if both are equally important. Keep **Standardize numeric columns** enabled for mixed units such as height/weight/age, gene counts, survey scales, or financial ratios. Turn it off only when raw variance is meaningful.

</details>

<details>
<summary>Can I use the result outside this page?</summary>

Yes. The default SVG is standalone and can be saved directly as a vector chart. CSV output gives `index,label,pc1,pc2` or `index,label,tsne1,tsne2` coordinates for another plotting tool. JSON output includes the full projection, categories, variable names, PCA explained variance, and the t-SNE perplexity actually used after small-table clamping.

</details>
