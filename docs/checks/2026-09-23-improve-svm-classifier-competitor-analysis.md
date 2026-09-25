# svm-classifier — competitor analysis (2026-09-23)

Scan run **before** implementation, so the descriptor could be designed against the table stakes
rather than retro-fitted. All findings are paraphrased from public documentation; no competitor
copy, branding, or trademarks are reproduced here or on the page.

## Competitors reviewed

| # | Tool | Shape | Why it matters |
|---|------|-------|----------------|
| 1 | scikit-learn `svm.SVC` | Python library, the de-facto reference API | Sets the parameter vocabulary everyone else copies (`C`, `kernel`, `gamma`, `degree`, `coef0`, `tol`, `class_weight`, `max_iter`, `decision_function_shape`) and the fitted-attribute vocabulary (`support_vectors_`, `n_support_`, `dual_coef_`, `coef_`, `intercept_`). |
| 2 | LIBSVM (`svm-train` / `svm-scale`) | C library + CLI, the solver underneath most SVM tools | Defines the flag-level defaults (`-t 2` RBF, `-c 1`, `-g 1/num_features`, `-d 3`, `-r 0`, `-e 0.001`), ships built-in **n-fold cross-validation** (`-v`), per-class cost weights (`-wi`), and a **separate feature-scaling tool** because unscaled SVMs behave badly. |
| 3 | An interactive browser "advanced SVM" playground (freetools.mcqsexam.com) | Closest analogue to a gizza tool page | Shows what a *web* SVM UI is expected to expose: four kernels, sliders for C/gamma/degree, a feature-scaling toggle, class weights, train/test split, random seed, **preset configuration buttons** (linear / high-bias / high-variance / complex-polynomial), sample datasets, and a metrics block of accuracy + precision + recall + F1 + confusion matrix alongside a decision-boundary plot with the margin drawn in. |
| 4 | MATLAB `fitcsvm` | Commercial stats toolbox | Confirms binary/one-class framing, standardization as a first-class option, and built-in cross-validation as table stakes. |

## Table stakes → decision

Every item below lands in the descriptor or in the out-of-model list. Nothing was dropped silently.

### In model — built

| Table stake | Seen in | Our param |
|---|---|---|
| Kernel choice, incl. linear + RBF | all four | `kernel` = `linear` \| `rbf` \| `poly` \| `sigmoid` (default `rbf`, matching sklearn/LIBSVM) |
| Regularization cost | all four | `c` (default `1`, like `C=1.0` / `-c 1`) |
| Kernel coefficient with the two symbolic defaults | sklearn (`scale`/`auto`), LIBSVM (`1/num_features`) | `gamma` accepts `scale`, `auto`, or an explicit number (default `scale`); the resolved numeric value is printed |
| Polynomial degree | all four | `degree` (default `3`) |
| Kernel offset | sklearn `coef0`, LIBSVM `-r` | `coef0` (default `0`) |
| Stopping tolerance | sklearn `tol=1e-3`, LIBSVM `-e 0.001` | `tol` (default `0.001`) |
| Solver iteration cap | sklearn `max_iter` | `max_iter` (default `100000`); non-convergence is reported, not hidden |
| Feature scaling / standardization | LIBSVM ships `svm-scale`; sklearn docs pair SVC with `StandardScaler`; playground has a toggle; `fitcsvm` has `Standardize` | `scaling` = `standard` \| `minmax` \| `none`, default `standard` (the correct default for a kernel method; the report states which was applied) |
| Class weighting for imbalanced data | sklearn `class_weight='balanced'`, LIBSVM `-wi` | `class_weight` = `none` \| `balanced` |
| Multiclass strategy | sklearn `decision_function_shape` | `multiclass` = `ovo` \| `ovr` |
| Train/test split + seed | playground | `test_split` (0–0.5) + `seed` |
| n-fold cross-validation | LIBSVM `-v`, `fitcsvm` | `cv_folds` (0 = off, else 2–10; stratified, seeded) |
| Support vectors reported | sklearn `support_vectors_`/`n_support_`/`dual_coef_` | Support-vector table with index, class, α, bounded/free flag, and the per-class counts |
| Margin | playground draws it; it is the defining SVM quantity | Margin width `2/‖w‖` plus `‖w‖`, computed in feature space so it is reported for **every** kernel, not just linear |
| Linear weight vector + intercept | sklearn `coef_`/`intercept_` | Per-feature weights for `kernel = linear`; bias (ρ) for all kernels |
| Accuracy, precision, recall, F1, confusion matrix | playground | All of them, per class plus macro averages |
| Predict new rows | every library's `.predict()` | `predict` field; reports class, decision value, and per-pair votes for multiclass |
| CSV input with a header and a chosen target column | playground, and every library via a dataframe | `data` (CSV/TSV/semicolon/pipe/whitespace), `target`, `features`, `header` |
| Categorical features | playground accepts arbitrary CSV | Automatic one-hot encoding of non-numeric columns (SVMs are numeric-only, so this is required for the advertised CSV input to actually work) |
| Preset configurations | playground's preset buttons | Five `[[example]]` chips, including a high-bias and a high-variance RBF preset |
| Machine-readable output | libraries return objects | `format` = `text` \| `json` \| `csv` |

### Out of model — listed, not built

| Table stake | Why it does not fit |
|---|---|
| Decision-boundary plot with the margin and support vectors drawn | This block's page surface renders text (`format = "text"`); image-bytes output has no page render mode in this repo. The numeric equivalents (margin width, ‖w‖, support-vector table) are reported instead. |
| Probability estimates (sklearn `probability`, LIBSVM `-b`) | Platt scaling needs an internal cross-validated sigmoid fit on top of every binary sub-model — a large amount of extra solver work for a calibration most users of a paste-a-table tool do not consume. The raw decision value (the signed distance driving the prediction) is reported per prediction instead. |
| ν-SVC, one-class SVM, ε-SVR / ν-SVR (LIBSVM `-s 1..4`) | Out of scope: this tool is the **classifier**. Regression is already served by `blocks/regression-model-trainer` and `blocks/least-squares-regression`. |
| Shrinking heuristic (sklearn `shrinking`, LIBSVM `-h`) | A pure solver-speed heuristic with no effect on the reported result. At this tool's 2,000-row cap the full kernel matrix is cached outright, which is faster than shrinking would be. |
| Kernel cache sizing (LIBSVM `-m`) | Not user-relevant here — the cap is chosen so the whole kernel matrix always fits. |
| Sample/synthetic datasets (moons, circles, XOR, iris) bundled in the UI | Preset chips carry worked data inline instead, which keeps the page dependency-free; bundling named public datasets would add fixtures without adding capability. |
| Export to Python code / PNG / SVG | Page-chrome features of a plotting playground; the generated CLI example already gives a reproducible invocation, and JSON output covers programmatic reuse. |
| `break_ties`, `decision_function_shape='ovr'` score reshaping subtleties | Both strategies are offered directly as `multiclass`; the deeper sklearn-specific reshaping semantics have no analogue outside that API. |

## Design consequences taken into the build

1. **`scaling` defaults to `standard`, not `none`.** Every competitor either ships a scaling tool
   next to the trainer or documents scaling as a prerequisite; an RBF SVM on raw unscaled columns
   is the single most common way these tools are misused. The report always prints which scaling
   was applied so the result is never silently transformed.
2. **Margin is computed in feature space** (`‖w‖² = ΣᵢΣⱼ αᵢαⱼyᵢyⱼK(xᵢ,xⱼ)`), so the headline number
   promised by the tool description exists for RBF/poly/sigmoid too, not only for linear.
3. **One-hot encoding is a capability, not a nicety.** The advertised input is "tabular data"; every
   reference implementation requires a numeric matrix, so the encoding step has to live in the tool
   or the advertised input is a lie for any table with a text column.
4. **Non-convergence is surfaced.** sklearn warns on `max_iter` exhaustion; a silent partially-solved
   model would make the margin and support-vector counts wrong, so the report states the iteration
   count and whether the KKT tolerance was met.
