# Competitor analysis: correlated-sample-generator

Date: 2026-08-16

## Scope

Tool under review: generate multivariate-normal sample rows from a supplied mean vector and covariance or correlation matrix, with deterministic output suitable for local CLI and browser use.

Search query used: `online correlated sample generator multivariate normal covariance matrix tool`.

## Competitor / reference scan

| Source | What it provides | Table-stakes found | In-model decisions for this tool | Out-of-model / not shipped |
| --- | --- | --- | --- | --- |
| Agricarehub multivariate normal calculator | Web calculator for multivariate-normal probability/density style workflows. Search result and fetched page title confirm a calculator built around mean vector and covariance matrix inputs. | Clear mean vector and covariance-matrix entry; explanatory copy for correlated continuous variables; calculator-first UX. | Keep matrix and mean as first-class fields, explain covariance/correlation mode in the descriptor and page, and include worked examples. | Probability/density integration and risk metrics are adjacent but not sample generation, so they are not included. |
| RandomGen `Generator.multivariate_normal` documentation | Python API for drawing samples from a multivariate normal distribution. The fetched docs list `mean`, `cov`, `size`, `check_valid`, `tol`, and `method`, and describe symmetric positive-semidefinite covariance requirements. | Mean vector, covariance matrix, sample count/shape, validation tolerance, handling invalid covariance matrices, and selectable factorisation method. | Include `mean`, `covariance`, `samples`, `tol`, and `method`. Provide `cholesky` and `eigen` modes, explicit validation errors, and deterministic `seed`. | Broadcasted array shapes and multi-batch sampling are Python-array features and do not fit the simple gizza form model. |
| MetricGate multivariate normal distribution calculator | Search result describes a page that computes density, generates random samples, and visualizes a multivariate normal distribution from mean vector and covariance matrix. | Combined calculator/sample workflow, CSV-like sample output, visual confirmation/summary, and examples for arbitrary dimension. | Include sample generation, multiple output formats, and `stats` output that reports achieved mean/covariance/correlation for quick verification. | Plotting/interactive visualization is useful but outside the current text-output page pattern for this pure block. |
| NumPy `random.multivariate_normal` documentation | Widely used Python reference for drawing samples from `mean` and `cov`; search snippet confirms example with bivariate covariance and expected correlation. | Mean/covariance terminology, deterministic seeding via caller RNG, sample count, and acceptance of positive-semidefinite covariance. | Match the common `mean`/`covariance` mental model; document that larger samples converge to requested correlations; keep seeded deterministic rows. | Full NumPy-compatible array broadcasting and warning modes are not needed for the CLI/page surface. |

## Capability decisions

Built in-model:

- Covariance matrix input with rows separated by semicolons, newlines, whitespace/commas, or JSON.
- Correlation-matrix mode with optional standard deviations.
- Mean vector and optional column labels.
- Sample count with hard caps: 1-100000 rows and 200000 emitted cells.
- Deterministic seed.
- Factorisation choice: `cholesky` for positive-definite matrices and `eigen` for positive-semidefinite/singular matrices.
- Validation tolerance for symmetry/eigenvalue checks.
- `empirical` mode for exact sample mean/covariance matching, matching a known statistics workflow.
- Output formats: CSV, TSV, JSON with achieved statistics, and stats-only report.
- Preset chips for common two-variable, real-unit, AR(1), empirical, and perfect-correlation cases.

Out of model / deferred:

- Probability-density and CDF/risk calculations, because this tool's backlog item is sample generation.
- Plots, ellipses and scatter visualizations, because the current reusable tool page output is text-oriented and the result table/statistics are verifiable headlessly.
- Python/NumPy broadcasting semantics and batched covariance arrays, because they do not map cleanly to the gizza form model.
- Streaming very large datasets beyond the emitted-cell cap, because browser and CLI surfaces should remain responsive.

## UX/control decisions

- Matrix and mean remain text fields because matrices are pasted data, not scalar controls.
- Fixed choices use enums: `matrix_kind`, `method`, and `output`.
- Booleans use checkboxes: `empirical` and `header`.
- `decimals` uses a slider because it is a bounded integer with a small range.
- Example chips cover the common tasks competitors document: correlated bivariate normals, correlation-plus-SD units, AR(1), exact empirical fixtures, and semidefinite/perfect-correlation matrices.

## Verification notes

The descriptor and manifest include every in-model table-stake above. Text output keeps the tool locally verifiable: CLI tests can assert exact seeded rows, JSON fields, and error messages; page tests can assert real generated output and query-param deep links.
