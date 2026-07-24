# multiple-regression — competitor analysis (2026-07-25)

Scan of real online multiple-linear-regression calculators, BEFORE implementation.
Paraphrased only — no competitor copy/branding reproduced.

## Competitors skimmed

1. **Social Science Statistics — Multiple Regression** (socscistatistics.com/tests/multipleregression)
2. **StatsKingdom — Multiple Linear Regression** (statskingdom.com/410multi_linear_regression.html)
3. **StatsCalculators — Multiple Linear Regression** (statscalculators.com/.../multiple-linear-regression-calculator)
   (plus lighter reads: standard-insights, onlinestatscalculator, numiqo)

## Table-stakes params / defaults / UX

| Capability | Competitors | Our decision |
|---|---|---|
| Paste a data matrix (rows = observations, columns = variables) | all (tab/CSV/Excel paste) | **in-model** — `data` multiline field, split on comma/tab/whitespace per row, newline per row |
| Choose which column is Y (dependent) | statskingdom = rightmost; statscalculators = pick a column | **in-model** — `response` accepts `last`/`first`/1-based index (default `last`, the common convention) |
| Column names / labels | statscalculators | **in-model** — `labels` comma-separated (defaults v1..vN, response named too) |
| Force zero Y-intercept | statskingdom ("Force zero Y-intercept") | **in-model** — `intercept` boolean (default true) |
| Coefficients incl. intercept (b0..bk) | all | **in-model** — coefficient table |
| Std error / t-stat / p-value per coefficient | all | **in-model** |
| R² and adjusted R² | all | **in-model** |
| F-statistic + p-value (overall model significance) | all | **in-model** — right-tailed F(df1, df2) |
| Residual standard error + residual df | statskingdom, statscalculators | **in-model** |
| Confidence intervals for coefficients | statscalculators, statskingdom | **in-model** — `conf_level` (default 0.95) |
| Significance level α customizable | statskingdom | **in-model** — via `conf_level` (CI ↔ α = 1−level) |
| Predicted values + residuals list | all | **in-model** — returned as `fitted`/`residuals` arrays (structured output) |

## Considered, NOT built (out-of-model or scope)

- **Residual / QQ / correlation plots, histograms** — visual diagnostics; this is a text/JSON tool, no chart surface here (correlation-heatmap covers the SVG-matrix case). Out.
- **Variable transformations (log/ln/sqrt/square), power regression** — pre-transform your columns before pasting; keeps the schema focused. Out (documented on page).
- **Stepwise / backward selection, exclude-outliers, standardize** — automated model-search / data-mutation workflows beyond a single fit. Considered, rejected (schema bloat, changes the requested model silently).
- **VIF / multicollinearity, skewness, normality tests** — additional diagnostics; the tool already errors clearly on singular (perfectly collinear) predictors. Considered; left out to keep the output an interpretable regression table.
- **Categorical predictor auto-encoding** — expects numeric columns; encode dummies yourself. Out.

## Math approach (pure Rust, wasm-safe, no deps beyond serde)

- OLS via normal equations (XᵀX)b = Xᵀy, solved by Gauss-Jordan with partial pivoting; (XᵀX)⁻¹ reused for the coefficient covariance = σ²·(XᵀX)⁻¹.
- σ² = RSS/(n−p); SE_j = √covⱼⱼ; t_j = b_j/SE_j; two-tailed p via Student-t CDF.
- R² = 1−RSS/TSS (centered TSS with intercept, uncentered without); adjusted R² by df.
- F = ((TSS−RSS)/df₁)/(RSS/df₂); right-tailed p via F CDF.
- t/F tail probabilities via the regularized incomplete beta function (Numerical-Recipes `betacf` continued fraction + `gammln`) — deterministic, no RNG, instantiates under wasmi.
