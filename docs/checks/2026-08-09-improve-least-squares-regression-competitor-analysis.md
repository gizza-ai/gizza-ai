# least-squares-regression — competitor analysis (2026-08-09)

Scan run **before** implementation, per `/create-next-tool` step 3. All notes are paraphrased
observations of publicly visible behaviour; no competitor copy, wording, branding or trademark is
reproduced or used anywhere in the tool.

## Why this is not a duplicate of `blocks/multiple-regression`

`multiple-regression` fits `y = b0 + b1·x1 + … + bk·xk` across **several predictor columns** and is
an *inference* tool (standard errors, t, two-tailed p-values, confidence intervals, F-test). It
requires ≥2 data columns and has no polynomial basis expansion — fitting a quadratic there means the
user hand-computes an `x²` column.

`least-squares-regression` is the **single-variable curve-fit / trendline** tool: paste `(x, y)`
pairs, choose a polynomial **degree** (1 = straight line), and get the fitted equation, coefficients,
R², adjusted R², RMSE and residuals, plus predictions at new x values. That is a different job, a
different input shape and a capability (`degree ≥ 2`) the existing block cannot express. The backlog
row for `multiple-regression` states the same split ("curve-fit only handles a single x/y
trendline").

## Competitors reviewed

| # | Tool | Focus |
|---|------|-------|
| 1 | Omni Calculator — polynomial regression | Degree 1–6 curve fit |
| 2 | Omni Calculator — least squares regression line | Straight-line fit |
| 3 | Statskingdom — linear regression calculator | Straight line + full diagnostics |
| 4 | Arachnoid PolySolve | Polynomial fit, arbitrary degree |
| 5 | StatsUnlock — polynomial regression calculator | Degree 2–6 + reporting output |

### 1. Omni Calculator — polynomial regression
- Data entered as up to 30 `(x, y)` rows in a growing table; ≥5 points required.
- **Degree dropdown**: linear / quadratic / cubic / quartic / quintic / sextic (1–6).
- **Precision dropdown**: 2–6 decimal places.
- Outputs: the polynomial equation `y = a₄x⁴ + … + a₀`, every coefficient, R², scatter plot with the
  fitted curve. No adjusted R², residuals or standard errors.

### 2. Omni Calculator — least squares regression line
- Table of up to 30 `(x, y)` points, minimum 2.
- Outputs: the line `y = b + ax`, Pearson correlation `r`, standard deviations of the slope and
  intercept, and the intermediate sums `Sx, Sy, Sxx, Syy, Sxy, Δ`.
- Adjustable output precision. No R², residuals or predictions at a chosen x.

### 3. Statskingdom — linear regression calculator
- `(X, Y)` table that also accepts a **spreadsheet paste**; non-numeric rows are dropped silently.
- Options: significance level α, **decimal precision 1–10**, a **zero-Y-intercept checkbox**,
  residual-outlier parameter k, trend-line toggle.
- Outputs: `Y = b0 + b1·X`, R² and R, slope/intercept, mean squared residual, fitted and predicted
  Y values, confidence and prediction intervals, plus residual/QQ/histogram plots and outlier tests.
- UX: Calculate / Clear / **Example** / Last run / Copy / Import buttons, step-by-step toggle.

### 4. Arachnoid PolySolve
- Data pasted as `x,y` pairs into a text area (clipboard paste is the documented path).
- Degree spinner up to 99, with an explicit documented warning that **degree ≥ 7 degrades from
  floating-point resolution**.
- Outputs: coefficients, correlation coefficient, standard error; adjustable decimal precision;
  degree 1 is formatted as `f(x) = mx + b`; a table generator evaluates the fit over a start/end/step
  range.

### 5. StatsUnlock — polynomial regression calculator
- Entry modes: paste comma-separated X and Y lists, CSV/TXT/XLSX upload, manual table, or one of five
  built-in sample datasets.
- Options: degree 2–6, α ∈ {0.01, 0.05, 0.10}, X-centering toggle, CI level 90/95/99%.
- Outputs: equation with per-term standard error, t and p; R², adjusted R², F with df, overall p,
  AIC, BIC; residual/QQ diagnostic plots; exportable prose summaries; DOC/PDF download.

## Table stakes → decision

| Capability | Seen in | Decision |
|---|---|---|
| Paste `(x, y)` pairs, one per line, mixed separators | 1,2,3,4 | **In** — `data`, split on comma / tab / semicolon / whitespace |
| Separate X list + Y list entry | 5 | **In** — optional `y` param; when set, `data` is the X list |
| Spreadsheet paste tolerance (blank lines, stray rows) | 3,4 | **In** — blank lines skipped; clear per-line error otherwise |
| Optional header row with column names | 3,5 | **In** — `header = auto\|yes\|no`; `auto` drops a non-numeric first row and uses it to name the axes in the equation |
| Polynomial degree selector | 1,4,5 | **In** — `degree` 1–10, default 1 (straight line) |
| Documented degree/conditioning limit | 4 | **In** — stated on the page; an ill-conditioned/rank-deficient fit errors instead of printing junk |
| Force zero Y-intercept | 3 | **In** — `intercept` boolean, default on |
| Decimal precision control | 1,2,3,4 | **In** — `decimals` 0–12, default 6 |
| Fitted equation string | 1,2,3,4,5 | **In** — `y = 2 + 3·x − 0.5·x²`, using header names when present |
| Coefficients + standard error per term | 2,4,5 | **In** — estimate + standard error for every term |
| R² | 1,3,5 | **In** |
| Adjusted R² | 5 | **In** |
| Pearson r / correlation coefficient | 2,3,4 | **In** — reported for `degree = 1` (where it is defined for the fit) |
| RMSE / residual standard error | 3,4,5 | **In** — both (RMSE and residual standard error with its df) |
| Per-observation fitted values + residuals | 3,5 | **In** — full table in `csv` / `json`, summary stats in `text` |
| Predict y at new x values | 3 | **In** — `predict_x`, comma-separated |
| Sample datasets / example presets | 3,5 | **In** — `[[example]]` preset chips on the page |
| CSV export of the fit table | 5 | **In** — `format = "csv"`; the page also offers a download link for text output |
| p-values / t-stats / CIs / F-test / AIC / BIC | 3,5 | **Deliberately out of scope here** — the sibling `multiple-regression` block already provides the full OLS inference table (SE, t, two-tailed p, CI, F-test) for linear models. Duplicating it would blur the two tools; this one stays the curve-fit/trendline tool. Noted, not silently dropped. |
| Scatter plot / fitted-curve chart, QQ + residual plots | 1,3,5 | **Out of model** — this block's page output is text; charting lives in the existing `csv-chart-generator` / `correlation-heatmap` blocks. |
| CSV / XLSX file upload | 5 | **Out of model** — pure-compute block with a text input; pasting the columns is the supported path. |
| DOC/PDF export, APA prose templates | 5 | **Out of model** — reporting/authoring feature, not a compute capability. |
| Confidence + prediction intervals around the curve | 3,5 | **Out of model for now** — needs the t-distribution machinery that `multiple-regression` owns. |

## UX patterns adopted

- **Degree as a labelled select** (`linear`, `quadratic`, … up to degree 10) via `[input.labels]`,
  mirroring the degree dropdowns in competitors 1 and 5 rather than a bare number box.
- **`[[example]]` preset chips** — the equivalent of competitor 3's "Example" button and competitor
  5's sample datasets: a straight-line fit, a quadratic curve, a header-row CSV paste, and a
  through-the-origin fit.
- **Multiline paste field** with a realistic placeholder, so spreadsheet copy-paste works directly.
- **Explicit precision control**, matching all four precision dropdowns seen.
- Every limit found in the scan (max degree, minimum point count, conditioning, non-numeric rows)
  is stated in the page copy, not just enforced in code.
