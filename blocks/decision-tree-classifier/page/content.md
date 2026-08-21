## About this tool

Fit a single decision tree to a pasted, labelled table and read back exactly how it decides. The
tool parses CSV, TSV, semicolon, pipe, or whitespace-delimited rows, treats one column as the class
label, and greedily grows a tree by Gini impurity decrease (CART), Shannon information gain (ID3),
or the gain ratio (C4.5). Feature columns can be numeric — split on a midpoint threshold — or
categorical, split either one-vs-rest or with one branch per value. Everything runs locally in
WebAssembly; the table never leaves the browser.

The report includes numbered `IF … THEN` rules, a text tree, normalised feature importance,
training accuracy with a confusion matrix, an optional hold-out check, and predictions for rows you
paste in. Output can also be JSON, flat CSV, or a Graphviz DOT digraph you can render as a diagram.

Worked example:

```csv
color,size,ripe
red,small,yes
red,large,yes
green,small,no
green,large,no
```

With the class column `ripe` and the default Gini criterion, the tree splits once and prints:

```text
Tree:
├─ color = green → no  [n=2, 100.0%]
└─ color != green → yes  [n=2, 100.0%]

Rules:
1. IF color = green THEN ripe = no  [n=2, 100.0%]
2. IF color != green THEN ripe = yes  [n=2, 100.0%]
```

`size` never gets used, so its importance is 0 and `color` takes the full 1.0 — the tree tells you
which column actually carries the signal.

Limits and edge cases: tables are capped at 20,000 rows and 100 columns, tree depth at 20, and
pasted prediction rows at 1,000. A feature or class column with more than 200 distinct values is
rejected as an id-like column. Rows with a blank, `NA`, `null`, or `?` value in the class column or
any selected feature are dropped and counted in the report. Accuracy measured on the same rows the
tree was fitted to is optimistic — set a hold-out test split, or shrink `max_depth`, to see how much
of the fit is real.

## FAQ

<details>
<summary>Which criterion should I pick?</summary>

`gini` is the CART default and is fast and stable — start here. `entropy` is classic ID3
information gain and usually produces a very similar tree. `gain_ratio` is the C4.5 correction:
it divides the information gain by how finely a feature fragments the data, which stops a
many-valued column (like a date or a product code) from winning every split just because it splits
the rows into lots of tiny pure groups.

</details>

<details>
<summary>How do I stop the tree from overfitting?</summary>

Use the pre-pruning knobs. `max_depth` caps how long a rule can get, `min_samples_split` stops
small nodes from splitting at all, `min_samples_leaf` refuses splits that would strand a handful of
rows in a branch, and `min_gain` throws away splits whose score is below a threshold. Combine any of
them with a `test_split` hold-out to check the effect on unseen rows. Cost-complexity pruning
(`ccp_alpha`) and C4.5 confidence-factor post-pruning are not implemented.

</details>

<details>
<summary>Can I mix text and numeric columns?</summary>

Yes. Each feature column is classified automatically: if every value in it parses as a number it is
treated as numeric and split with a `<=` threshold at the midpoint between two observed values;
otherwise it is categorical and split on its values. Categorical splits are one-vs-rest by default
(`color = green` versus `color != green`); switch **Categorical splits** to multiway for one branch
per distinct value, the way ID3 and C4.5 present them.

</details>

<details>
<summary>How do I classify new rows?</summary>

Paste them into **Rows to classify**, one per line. Three layouts are accepted: the full table
layout (the class column is ignored), just the feature columns in order, or a header row naming the
columns followed by the values. Each prediction reports the class, the leaf's purity as a
confidence, and which numbered rule fired. An unseen category takes the `!=` branch; a missing value
follows whichever branch held the most training rows.

</details>

<details>
<summary>What do the importance numbers mean?</summary>

They are impurity-decrease shares, the same idea as a standard `feature_importances_` vector: every
split adds the impurity it removed, weighted by the share of rows reaching that node, and the totals
are normalised to sum to 1. A column that is never split on scores 0. They describe this tree on
this data — they are not causal claims, and a correlated column can absorb another one's credit.

</details>

<details>
<summary>Is the result reproducible?</summary>

Yes. Tree fitting is completely deterministic — no random feature sampling, no tie-breaking by
chance — so the same table and options always produce the same rules. The only randomness is the
shuffle behind the hold-out test split, and that is driven by `seed`.

</details>
