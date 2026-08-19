## About this tool

Train a regression model from a pasted numeric table without uploading data. The tool parses CSV,
TSV, semicolon, pipe, or whitespace-delimited rows, picks a target column, and fits ordinary least
squares, ridge regression, or a bounded deterministic random forest. Reports include R², RMSE, MAE,
row counts, optional hold-out and cross-validation metrics, plus coefficients for linear/ridge or
feature-importance shares for random forest.

Worked example:

```csv
x,y
1,3
2,5
3,7
4,9
```

With target `y` and model `linear`, the fitted equation is `y = 1 + 2·x`, with R² of 1 and RMSE 0.
For multiple predictors, set `features` to a comma-separated list such as `sqft,rooms`, or leave it
blank to use every non-target numeric column.

Limits and edge cases: the table is capped at 20,000 rows and 100 columns. All selected predictors
and the target must be numeric; encode categories as numeric dummy columns before using this tool.
Random forest work is capped, so reduce rows, trees, or CV folds for larger tables. Coefficients are
not causal claims; they summarize the pasted data under the chosen model.

## FAQ

<details>
<summary>Which model should I choose?</summary>

Use `linear` for a quick ordinary least squares equation, `ridge` when predictors are correlated or
have different scales, and `random_forest` when the relationship is nonlinear and you care more about
prediction and importance than a simple equation.

</details>

<details>
<summary>How do I select columns?</summary>

`target` accepts `last`, `first`, a 1-based index, or a header name. `features` is optional; leave it
blank to use every other column, or list columns such as `sqft,rooms` or `1,3` to fit only those
predictors.

</details>

<details>
<summary>What happens to missing or non-numeric values?</summary>

Rows with missing values such as `NA`, `null`, or blank cells in selected columns are dropped and
reported. Non-numeric text in selected predictors or the target is an error, because this tool does
not perform automatic categorical encoding.

</details>

<details>
<summary>Is the random forest reproducible?</summary>

Yes. Bootstrap samples, feature choices, train/test splits, and cross-validation folds all use the
`seed` value, so the same data and options produce the same report.

</details>
