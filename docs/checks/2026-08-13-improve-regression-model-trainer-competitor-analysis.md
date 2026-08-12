# regression-model-trainer — competitor analysis (2026-08-13)

Scan run before finalizing implementation. Notes are paraphrased observations only; no competitor copy, branding, or assets are reused.

## Competitor scan

1. Browser/worksheet regression calculators commonly accept a pasted table or typed x/y columns, let the user pick linear regression, and report an equation, R² and sometimes residual/error metrics. Their UX favors a multiline data box, target/predictor selectors, and a compact readable report.
2. Statistics packages and notebook examples expose ordinary least squares and ridge/lasso-style regularization. Table-stakes knobs are target column, feature subset, standardization, regularization strength, train/test split, cross-validation, and machine-readable output for scripting.
3. AutoML/regression trainer services add random forests or tree ensembles, feature importance, model downloads, charts and batch prediction. Server features such as accounts, persistent models, uploaded datasets and deployable endpoints are out of model for a local wasm block, but a bounded deterministic random forest and importance table are in model.

## Table stakes and decisions

| Capability | Fit | Decision |
| --- | --- | --- |
| Pasted numeric table | in-model | `data` textarea; CSV/TSV/semicolon/pipe/whitespace parsing |
| Target column selection | in-model | `target` accepts name, 1-based index, `first`, `last` |
| Feature subset | in-model | `features` comma-separated allowlist |
| OLS linear regression | in-model | `model=linear`, coefficients and equation |
| Ridge regression | in-model | `model=ridge`, `alpha`, optional standardization |
| Random forest regression | in-model with bounds | `model=random_forest`, `trees`, `max_depth`, deterministic seed and work cap |
| R², RMSE, MAE | in-model | reported for train, optional test, OOB/CV where applicable |
| Train/test split | in-model | deterministic `test_split` up to 0.5 |
| Cross validation | in-model with bounds | `cv_folds` 0 or 2–10 |
| Feature importance | in-model | variance-reduction shares for random forest |
| JSON/CSV output | in-model | `format=text|json|csv` |
| Charts, residual plots, downloadable model artifacts | out-of-model | page stays text-first; no persistent model file |
| Uploading/private datasets, hosted AutoML, deployment endpoints | out-of-model | no backend/accounts; local-only pasted data |
| Categorical encoding | out-of-model for this tool | docs tell users to encode categories as numeric columns first |

## Implementation stance

The shipped tool is deterministic pure Rust: no ML runtime, no network, no model download. Random forest is intentionally small and bounded so it is usable in wasm; larger AutoML workflows remain out of scope. The product differentiator is transparent metrics and coefficients/importances that are reproducible from the pasted table and seed.
