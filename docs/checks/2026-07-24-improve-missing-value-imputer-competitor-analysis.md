# missing-value-imputer — competitor analysis (2026-07-24)

Tool function: fill missing cells in a CSV using mean / median / most-frequent / constant /
KNN imputation, entirely in the browser. All copy below is **paraphrased** — no competitor
copy, branding, or trademarks reproduced.

## Landscape

Missing-value imputation has no dominant free "online tool" page; the de-facto standard is the
scikit-learn `SimpleImputer` / `KNNImputer` API plus pandas `fillna`, which is what practitioners
compare any imputation tool against. The top results are the scikit-learn docs and KNN tutorials
that document the exact strategy set, parameter names, and defaults. Those are the reference
"competitors" whose feature/parameter surface ours must match.

### 1. scikit-learn `SimpleImputer` (reference standard)
- **Features (paraphrased):** univariate per-column imputation; strategies `mean`, `median`,
  `most_frequent`, `constant`.
- **Params/defaults:** `strategy='mean'`, `fill_value=None` (required for `constant`),
  `missing_values=np.nan`, `add_indicator=False`, `keep_empty_features=False`.
- **Behavior:** mean/median require numeric columns; most_frequent/constant work on any column.

### 2. scikit-learn `KNNImputer` (reference standard)
- **Features:** multivariate imputation from the k nearest complete-enough rows.
- **Params/defaults:** `n_neighbors=5`, `weights='uniform'` (alt `'distance'`),
  `metric='nan_euclidean'`, `missing_values=np.nan`.
- **Behavior (paraphrased):** distance ignores coordinates missing in either row and rescales by
  total/ present coordinate count (nan-euclidean); the imputed value is the (optionally
  distance-weighted) average of the neighbours that have a value for that feature; falls back to
  the column mean when no neighbour has the value.

### 3. pandas `fillna` / KNN tutorials (MachineLearningMastery, GeeksforGeeks — paraphrased)
- **Features:** constant fill, forward/backward fill, per-column mean/median/mode; `na_values`
  to treat markers like `?`, `NA`, empty as missing when reading a CSV.
- **UX patterns worth matching:** let the user declare which strings count as missing; let the
  user restrict imputation to selected columns; sensible KNN default of k=5.
- **Worked example shape:** horse-colic-style tabular data, ~hundreds of rows × tens of numeric
  features, `?` used as the missing marker.

## Table-stakes → decision (every one lands in the descriptor OR the out-of-model list)

| Table-stake | Decision | Where |
| --- | --- | --- |
| mean / median / most_frequent / constant strategies | in-model | `strategy` enum |
| KNN imputation (nan-euclidean) | in-model (spiked: pure f64 arithmetic, no crate) | `strategy=knn` |
| `n_neighbors` (default 5) | in-model | `n_neighbors` |
| KNN `weights` uniform/distance | in-model | `weights` enum |
| `fill_value` for constant | in-model | `fill_value` |
| custom missing markers (`?`, `NA`, `null`…) | in-model | `na_tokens` |
| restrict to selected columns | in-model | `columns` |
| CSV header / delimiter handling | in-model | `header`, `delimiter` |
| numeric-only guard for mean/median/knn | in-model (documented behavior) | core skips non-numeric cols |

### Out-of-model / considered, not built
- **IterativeImputer (MICE) regression imputation** — pure-Rust feasible but a large iterative
  round-robin regression engine; disproportionate scope for one tool. Listed, not built.
- **Missing-value indicator columns (`add_indicator`)** — changes the output column shape;
  declined to keep the output a clean same-shape CSV. Listed, not built.
- **Model-persistence / fit-on-train-apply-to-test split** — needs stateful sessions/accounts;
  out of the browser-local, single-input model.

## Design outcome
Descriptor params: `data`, `strategy` (enum), `header` (bool), `delimiter`, `columns`,
`na_tokens`, `fill_value`, `n_neighbors` (int, KNN), `weights` (enum, KNN). Non-numeric cells are
left unchanged under mean/median/knn (matching sklearn's numeric requirement); most_frequent and
constant apply to any column.
