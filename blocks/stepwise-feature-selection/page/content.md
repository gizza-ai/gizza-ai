## About this tool

Stepwise feature selection helps you choose a smaller ordinary-least-squares regression model from a wider numeric table. Paste one observation per row, choose the target column, and select whether the search should start empty and add terms, start full and remove terms, or move in both directions. The output is a plain-text audit trail: every accepted add/drop move, the selected predictors, coefficient estimates, standard errors, t and p values, R² / adjusted R², RMSE, information criteria, an F-test, dropped predictors, and a null-vs-selected-vs-full model comparison.

Use it for quick exploratory screening before a deeper model review. It is deterministic, local, and works with comma-, tab-, semicolon-, or whitespace-separated numeric rows. The first row can be a header, or you can supply labels separately.

### Worked example

Paste this table and set **Target column** to `sales`, **First row is a header** on, and **Search direction** to `Both`:

```text
ads,price,temp,sales
2,19,21,30.4
5,15,14,49.0
3,18,30,37.0
8,12,18,67.1
6,14,25,56.4
9,11,11,75.1
4,17,27,40.4
7,13,16,62.7
10,10,23,79.7
5,16,29,48.4
8,12,13,67.8
3,20,19,31.9
```

The report should keep the advertising and price columns, drop the temperature noise column, and show the final fitted equation with the selected model's fit statistics.

### Limits and edge cases

- The tool accepts up to 20,000 rows and 60 total columns, including the target.
- All values after an optional header must be finite numbers; missing cells and text cells are reported with their row and column.
- Stepwise selection is a screening heuristic, not a guarantee of causal predictors or stable inference. Validate the chosen model on held-out data and with domain knowledge.
- Perfectly collinear candidate subsets are skipped so the search can continue, but a table with too few usable rows or a constant target is rejected.
- `AIC`, `BIC`, and `AICc` compare models on the same data. `BIC` penalizes larger models more strongly; `pvalue` mode uses `alpha_enter` and `alpha_remove` thresholds instead.

## FAQ

<details>
<summary>Which search direction should I choose?</summary>

Use **forward** when you have many candidate predictors and want to start from an intercept-only model. Use **backward** when you want to start from all predictors and prune. Use **both** when you want classic stepwise behavior: add a strong predictor, then re-check whether an older predictor became redundant.

</details>

<details>
<summary>What is the difference between AIC, BIC, AICc, and p-value mode?</summary>

AIC usually allows somewhat larger predictive models, while BIC applies a stronger penalty for extra predictors. AICc is AIC corrected for smaller samples. P-value mode does not optimize an information criterion; it adds predictors below `alpha_enter` and removes predictors above `alpha_remove`.

</details>

<details>
<summary>Can I force a control variable to stay in the model?</summary>

Yes. Put one or more predictor names or 1-based column numbers in **Always keep predictors**, separated by commas. Forced predictors seed the starting model and are never removed, which is useful for controls that must stay in every candidate model.

</details>

<details>
<summary>Does this replace statistical validation?</summary>

No. Stepwise regression is useful for quick exploration, but it can overfit and make p-values look more certain than they are. Treat the output as a candidate model, then check residuals, stability, multicollinearity, and out-of-sample performance.

</details>
