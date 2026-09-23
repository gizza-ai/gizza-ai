## About this tool

MARS — Multivariate Adaptive Regression Splines — fits a nonlinear relationship as a sum of straight-line pieces that join at automatically chosen breakpoints. Each piece is a *hinge function*: `h(x - t)` is `x - t` when `x` is above the knot `t` and `0` below it, and `h(t - x)` is its mirror image. Because the knots are picked from the data rather than fixed in advance, MARS finds where a trend changes slope instead of forcing one global line or one high-degree polynomial through everything.

Paste a numeric table, pick the target column, and the tool runs both MARS passes: a **forward pass** that grows hinge terms until they stop paying for themselves, and a **backward pass** that removes terms and keeps whichever sub-model scores best on GCV (generalized cross-validation). You get the explicit equation, the term table with coefficients, the knots per variable, fit statistics, variable importance, and predictions for any new rows you supply.

Columns can be separated by commas, tabs, semicolons, pipes or plain spaces, and a header row is detected automatically. The target defaults to the last column; name it explicitly with `target`, and restrict the predictors with `features`.

## Worked example: a curve one straight line cannot fit

This series falls to a minimum and rises again — a V. Ordinary linear regression fits it with a slope of roughly zero and explains nothing.

```text
x,y
0,5
1,4
2,3
3,2
4,1
5,0
6,1
7,2
8,3
9,4
10,5
```

With the defaults, MARS puts a knot exactly at the turn and reproduces the series:

```text
y = 0.0000
    + 1.0000 * h(x - 5.0000)
    + 1.0000 * h(5.0000 - x)
```

The fit block reports `R² 1.0000` and `GRSq 1.0000`, and the `Knots` section lists `x  5.0000`. Put `11` and `12` in the prediction box and the two hinge terms extrapolate the rising arm to `y = 6` and `y = 7`.

Read the equation left to right: below `x = 5` only `h(5 - x)` is active, so `y` falls at one unit per step; above `x = 5` only `h(x - 5)` is active, so it rises at one unit per step. Every MARS model reads this way, which is why it stays interpretable where a tree ensemble does not.

## Worked example: when you need interactions

Interaction degree is the knob worth understanding. This table is `y = a × b` on a 5 × 5 grid — a surface no sum of single-variable curves can reproduce:

```text
a,b,y
0,0,0
0,1,0
0,2,0
0,3,0
0,4,0
1,0,0
1,1,1
1,2,2
1,3,3
1,4,4
2,0,0
2,1,2
2,2,4
2,3,6
2,4,8
3,0,0
3,1,3
3,2,6
3,3,9
3,4,12
4,0,0
4,1,4
4,2,8
4,3,12
4,4,16
```

At the default `max_degree = 1` the model is additive — one curve in `a` plus one curve in `b` — and it tops out at `R² 0.7765`, `GRSq 0.5271`. Raise the degree to 2 and the forward pass is allowed to multiply hinges together, producing terms such as `h(b - 2.0000) * a`. The model reaches `R² 1.0000` with 7 terms, and predicting `3,4` returns exactly `12`.

The lesson generalises: start additive, and only raise the degree if GRSq (not R²) improves. R² always goes up when you add terms; GRSq is GCV expressed on the R² scale, so it charges you for the extra complexity and is the honest number to compare across settings.

## Choosing the controls

- **Max terms** bounds the forward pass. More terms means a richer starting basis for pruning to choose from, at the cost of time. The default of 21 covers most pasted tables.
- **Max interaction degree** is 1 (additive) by default; 2 allows two-way products of hinges, 3 allows three-way.
- **Prune by GCV** is on by default and is what keeps the final model small. Turn it off to inspect the raw forward-pass basis.
- **Kept-term cap** forces the pruning pass to stop at a fixed number of terms instead of letting GCV decide. `0` means auto.
- **GCV penalty** is the cost charged per term, matching the convention used by the standard MARS implementations. Higher values give smaller models; `0` disables the complexity charge.
- **Minimum knot span** and **edge protection span** control how far apart knots must be and how many rows at each end of a variable's range are ineligible as knots. `0` derives both from Friedman's formulas, with the edge span capped so at least half the rows stay eligible on short tables.
- **Minimum RSS improvement** stops the forward pass once a new hinge pair buys less than that relative improvement. Raise it to stop earlier.

## Limits and edge cases

- Up to 5,000 data rows, 50 columns, and 1,000 prediction rows per run.
- The table must be completely numeric. Blank, `NA`, `n/a`, `NaN`, `null`, `-` and `?` cells are rejected with the row and column named, rather than silently imputed or dropped.
- Every row must have the same number of columns as the first row, and the target column must vary — a constant target is rejected.
- Prediction rows carry feature values only, in the same order as the training features, with no target column.
- Knots are only ever placed at observed values of a variable, so a true breakpoint that falls between two measured points is approximated by the nearest eligible one.
- Knot candidates are thinned evenly under a fixed work budget, at most 256 per variable and parent term, so a wide or long table stays responsive. The `Model` block reports the candidate count actually used, along with the spans in force.
- The fit is piecewise **linear**. There is no cubic-smoothed hinge basis, so the fitted curve has corners at the knots.
- Everything is deterministic: no sampling, no random restarts, no fast-pass heuristics. The same input always produces byte-identical output.
- Regression only — one numeric target, no classification or binomial link — and pruning is by GCV, not k-fold cross-validation.

## FAQ

<details>
<summary>What is a hinge function, and how do I read the equation?</summary>

`h(x - 5)` equals `x - 5` when `x > 5` and `0` otherwise; `h(5 - x)` is the mirror, equal to `5 - x` when `x < 5` and `0` otherwise. A coefficient on a hinge term is the slope that switches on once the knot is crossed. So `y = 0 + 1 * h(x - 5) + 1 * h(5 - x)` means "fall at 1 per unit until x = 5, then rise at 1 per unit" — the equation is a literal description of the shape.

</details>

<details>
<summary>How is this different from polynomial or linear regression?</summary>

Polynomial regression applies one global formula to the whole range, so a bend in the middle distorts the fit at both ends and extrapolation swings wildly. MARS keeps the model local: it selects breakpoints from the data and fits straight segments between them, so a change of slope in one region does not disturb the others. It also selects variables — predictors that earn no terms simply do not appear in the equation.

</details>

<details>
<summary>What do GCV and GRSq mean, and which should I compare?</summary>

GCV is a penalised estimate of out-of-sample error: residual sum of squares adjusted upward for the number of terms and knots the model spent. GRSq restates it on the R² scale, where 1 is perfect and 0 matches the intercept-only model. Compare **GRSq** across different settings. R² can only increase as terms are added, so it will happily reward an overfit model; GRSq will not.

</details>

<details>
<summary>Why does the model keep a term whose coefficient is 0?</summary>

Forward-pass hinges are added in mirrored pairs, and pruning evaluates whole sub-models by GCV rather than dropping individual near-zero coefficients. If removing a term does not improve the GCV score, it stays, showing a coefficient of zero at your chosen decimal precision. Lower the kept-term cap, or raise the GCV penalty, to force a leaner model.

</details>

<details>
<summary>Can it handle missing values or text categories?</summary>

No. Every cell has to parse as a finite number, and a missing or non-numeric cell is reported as an error naming the row and column. Encode categories yourself first — one column of 0/1 indicators per level works well with MARS, since a hinge on a 0/1 column reduces to a plain step.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The model is fitted by a WebAssembly module running in this page, so the table you paste never leaves your device. The same compiled code backs the command-line and chat versions of this tool, so all three produce identical output for identical input.

</details>
