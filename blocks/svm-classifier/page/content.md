## About this tool

A support vector machine (SVM) classifier looks for a boundary that separates classes with the widest possible margin. Rows closest to that boundary become **support vectors**: they are the training examples that actually define the model. This tool trains a deterministic C-SVC model in your browser or CLI and reports the margin, support-vector counts, confusion matrix, per-class precision/recall/F1, optional hold-out and cross-validation accuracy, plus predictions for new rows.

Paste a labeled numeric or mixed CSV/TSV table. The target column defaults to the last column, and every other column becomes a feature unless you list specific feature names. Numeric columns are scaled by default; text feature columns are one-hot encoded so ordinary tabular data works without preprocessing.

## Worked example

```text
x,y,label
1,1,a
2,1,a
1,2,a
8,8,b
9,8,b
8,9,b
```

With `kernel = linear`, `target = label`, and predictions:

```text
1,1
9,9
```

the report shows a perfect training split, support vectors from both classes, a margin width, and predicted classes `a` then `b`. Switch to `kernel = rbf` for curved class boundaries, or set a `test_split` / `cv_folds` value to get a less optimistic accuracy estimate than training accuracy.

## Choosing controls

- **Kernel** is the main modeling choice. Start with `linear` when the classes are roughly separable by a straight line; use `rbf` for general nonlinear boundaries; use `poly` or `sigmoid` only when you need those specific shapes.
- **C** controls regularization. Lower values tolerate more margin violations and usually generalize better; higher values try harder to classify every training row.
- **Gamma** controls how local RBF/poly/sigmoid kernels are. `scale` is the default, `auto` is `1 / feature_count`, and explicit numbers like `0.5` are accepted.
- **Scaling** should usually stay `standard`. Kernel SVMs compare distances, so unscaled units can dominate the boundary.
- **Class weights** can be set to `balanced` when one class has far fewer rows.
- **Hold-out split** and **CV folds** are optional checks. Training accuracy is useful for debugging, but it is optimistic because it is measured on the rows used to fit the model.

## Limits and edge cases

- Up to 2,000 training rows, 100 input columns, 20 classes, 50 levels per categorical feature, and 1,000 prediction rows.
- Missing values in the target or selected features are dropped and counted. If every usable row disappears, the tool returns an error.
- A target column must contain at least two classes. A class-like column with too many distinct values is probably an ID or continuous variable and is rejected.
- Probability estimates are not produced. The prediction table reports the raw decision value instead.
- Decision-boundary plots are not generated; the numeric equivalents are the margin width, `||w||`, support vectors, and confusion matrix.
- If the solver hits `max_iter`, the report says so and marks the fit as partially optimized rather than hiding the warning.

## FAQ

<details>
<summary>What is a support vector?</summary>

A support vector is a training row that lies on or inside the margin and therefore affects the fitted boundary. Moving a non-support-vector row usually does not change the model; moving a support vector can change the margin, bias, or predicted class boundary. The report lists support-vector row numbers, their class labels, their alpha values, and whether each is at the `C` bound.

</details>

<details>
<summary>Should I use linear or RBF?</summary>

Use `linear` first when you want readable feature weights or have many columns. Use `rbf` when classes bend around each other and a straight boundary underfits. RBF models are more flexible, so check hold-out or cross-validation accuracy and watch the note that says every row became a support vector; that often means gamma or C is too aggressive.

</details>

<details>
<summary>Why does the tool scale features by default?</summary>

An SVM kernel compares distances or dot products. If one column is measured in thousands and another in decimals, the large-unit column can drown out the rest even if it is not more important. `standard` scaling subtracts the mean and divides by standard deviation for each encoded feature column, matching the preprocessing commonly paired with SVM classifiers.

</details>

<details>
<summary>Can I use text or categorical columns?</summary>

Yes. Any selected feature column that is not fully numeric is one-hot encoded into columns like `color=red` and `color=green`. Columns with more than 50 distinct values are rejected as likely IDs because one-hot encoding them would make a huge sparse model and usually overfit.

</details>

<details>
<summary>Why is training accuracy sometimes too high?</summary>

Training accuracy is measured on the same rows the model used to fit the boundary, so it can overstate real performance. Set `test_split` to hold out a stratified sample or set `cv_folds` to run seeded stratified cross-validation. Those checks are slower but more honest.

</details>
