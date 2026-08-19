# stepwise-feature-selection — competitor scan (2026-08-16)

Scan run **before** implementation, per the create-next-tool recipe. Everything below is
**paraphrased** from public documentation — no competitor copy, branding, or trademarks are
reproduced, and nothing was copied into the tool.

Search: "stepwise regression online tool forward backward AIC BIC feature selection calculator"
(WebSearch, 2026-08-16). The result set is dominated by tutorials/blog posts; the three entries
below are the real *tools* (one hosted calculator, one R package, one desktop statistics package)
that a user comparing options would actually land on.

## Competitor profiles (paraphrased)

### 1. MetricGate — hosted stepwise regression calculator
- **URL:** metricgate.com/docs/stepwise-regression/
- **Features:** browser-hosted calculator; user picks a response variable and candidate
  predictors, defines the search scope between a null (intercept-only) model and a full model,
  then runs the search.
- **Params/options:** direction (forward / backward / both); criterion (AIC, BIC, or a p-value
  threshold); scope (null model, full model); significance level for the final coefficient tests.
  Defaults are not documented on the page.
- **Input:** dataset upload / built-in example datasets; exact accepted file formats not stated.
- **Output:** selected-model coefficients, the AIC trajectory across steps, R², per-coefficient
  p-values, a summary of the add/remove sequence, a null-vs-stepwise-vs-full model comparison,
  and optional residual diagnostic plots. Optional-output toggles let the user turn individual
  report sections on or off.
- **Limits stated:** none numeric for stepwise; notes that best-subset search is only practical
  up to roughly 20 predictors.
- **Free vs paid:** calculator usable on the site; account/workspace features gated.

### 2. StepReg (R package, CRAN vignette)
- **URL:** cran.r-project.org/web/packages/StepReg/vignettes/StepReg.html
- **Features:** forward, backward, bidirectional, and best-subset strategies; multiple strategies
  and multiple metrics can be requested in one call; plot methods visualise the selection path.
- **Params/options:** strategy; metric (AIC, AICc, BIC, and significance-level based selection);
  entry/stay significance levels; an `include` argument that forces named variables to stay in
  every candidate model; several response types beyond linear (logistic, Cox, multivariate).
- **Output:** the selection-process table (which variable entered or left at each step) plus the
  final model summarised through the usual R accessors (coefficients, fit statistics).
- **Limits:** none stated; bounded by R's own memory.
- **Free vs paid:** free, open source — but requires installing R.

### 3. Minitab — stepwise regression inside Fit Regression Model
- **URL:** support.minitab.com/.../perform-stepwise-regression/
- **Features:** five procedures — stepwise (add and remove), forward selection, backward
  elimination, forward selection driven by an information criterion, and forward selection
  validated against a test set or cross-validation.
- **Params/options:** alpha-to-enter and alpha-to-remove set independently; AICc or BIC when the
  information-criterion procedure is chosen; terms that must appear in every model (forced in)
  versus terms only seeded into the initial model; hierarchy enforcement for interaction terms.
- **Output:** coefficients, p-values and model summary statistics **for each step**; training vs
  validation R² plots for the validated procedure.
- **Limits:** none published; desktop-licensed software.
- **Free vs paid:** commercial licence.

## Table-stakes checklist → decision

| Capability (≥1 competitor ships it) | Fit | Decision |
| --- | --- | --- |
| Direction: forward / backward / bidirectional | in-model | **built** — `direction` enum, default `both` |
| Criterion: AIC | in-model | **built** — `criterion=aic` (default) |
| Criterion: BIC | in-model | **built** — `criterion=bic` |
| Criterion: AICc (small-sample corrected) | in-model | **built** — `criterion=aicc` |
| Criterion: p-value thresholds | in-model | **built** — `criterion=pvalue` |
| Separate alpha-to-enter / alpha-to-remove | in-model | **built** — `alpha_enter` 0.05, `alpha_remove` 0.10 |
| Force variables into every model (`include`) | in-model | **built** — `force`, by name or index |
| Choose the response/target column | in-model | **built** — `target`: name, 1-based index, `first`, `last` |
| Named columns (header row / explicit names) | in-model | **built** — `header` + `labels` |
| Per-step selection trace | in-model | **built** — "Selection path" section with the criterion after each move |
| Final coefficients + std error + t + p | in-model | **built** — coefficient table |
| Model fit stats (R², adjusted R², RMSE, F-test) | in-model | **built** — "Fit" section |
| Null vs selected vs full model comparison | in-model | **built** — "Model comparison" table |
| Fitted equation written out | in-model | **built** — formula line |
| Output rounding control | in-model | **built** — `decimals` |
| Example datasets / one-click presets | in-model | **built** — three `[[example]]` preset chips on the page |
| Stated caps and singularity behaviour | in-model | **built** — caps in the descriptor, on the page, and in errors |
| Residual diagnostic plots | out-of-model | **listed, not built** — the page renders text output; a plot renderer is a separate visual tool |
| Dataset file upload (CSV/XLSX) | out-of-model | **listed, not built** — this is a paste-in pure-compute block; file ingest belongs to the file-input tool family |
| Logistic / Cox / multivariate responses | out-of-model for this tool | **listed, not built** — different likelihoods; a separate block, not a flag on a linear-regression selector |
| Cross-validation / hold-out validated selection | in-model but **rejected** | `blocks/regression-model-trainer` already owns train/test split + k-fold CV; duplicating it here would be schema bloat with a worse split story |
| Best-subset (exhaustive) search | in-model but **rejected** | a different algorithm (2^k), only practical to ~20 predictors; the tool is named and scoped for *stepwise* |
| Multiple strategies/metrics in one call | in-model but **rejected** | returns a matrix of models instead of an answer; hurts the one-paste-one-answer UX. Preset chips cover comparing runs |

## Notes carried into the build

- **AIC convention.** The reported AIC/BIC use the standard regression form
  `n·ln(RSS/n) + penalty·k` (k = number of fitted coefficients including the intercept) — the same
  scale-free convention R's `step()` uses. It differs from a full log-likelihood AIC by a constant
  that is identical for every model on the same data, so comparisons and the selection path are
  unaffected. This is stated on the page so a user cross-checking against `AIC(lm(...))` isn't
  surprised by the offset.
- **AICc convention.** `AIC + 2k(k+1)/(n−k−1)`, with the model rejected outright when
  `n − k − 1 ≤ 0`.
- **Ties/stopping.** A move is only taken when it improves the criterion by more than a small
  relative tolerance, so bidirectional search can't oscillate.
