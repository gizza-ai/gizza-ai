# mars-spline-regression — competitor analysis (2026-09-23)

Scan run **before** implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All findings are paraphrased from public documentation; no competitor copy, branding or
trademarks are reproduced here or on the page.

## Scope

Backlog row: `mars-spline-regression` — "Fits Multivariate Adaptive Regression Splines
(hinge functions) to capture nonlinear, piecewise relationships and returns the model and
predictions." Type hint `pure`.

## Duplicate check (done first)

| Existing block | What it does | Overlap verdict |
| --- | --- | --- |
| `least-squares-regression` | Linear / polynomial OLS on one x,y column pair | Global polynomial, one predictor, fixed basis — **no adaptive knots**, no multivariate. Not a dup. |
| `multiple-regression` | Multi-predictor linear OLS | Purely linear, no hinges/knots. Not a dup. |
| `spline-smoother` | Cubic **smoothing** spline over one noisy series, penalty chosen by GCV/CV | A 1-D smoother, not a model with a term table; no feature selection, no interactions. Not a dup. |
| `regression-model-trainer` | Table → linear / ridge / random-forest fit with R², RMSE, coefficients or importances | Closest neighbour. Its three model arms all produce either a *global linear* equation or a *black-box* forest. MARS's deliverable — an explicit piecewise-linear equation made of `h(x − t)` hinge terms with automatically selected knots, a forward/backward pruning trace and GCV/GRSq — is not reachable from any of them. Not a dup. |
| `interpolation`, `linear-interpolate-gaps`, `exponential-smoother`, `moving-average` | Interpolation / smoothing of a series | Different problem class. Not a dup. |

Conclusion: buildable, distinct. The distinguishing artifact is the **hinge-term model with
automatically chosen knots**, which no shipped block produces.

## Competitors reviewed

1. **py-earth** (`Earth` estimator, scikit-learn-contrib) — the reference Python MARS
   implementation.
2. **R `earth` package** (as documented in the UC Business Analytics R guide's MARS
   chapter) — the de-facto reference implementation and the one most tutorials use.
3. **tidymodels `parsnip` `mars()` with the `earth` engine** — the modern tuning-facing
   wrapper; shows which knobs practitioners actually expect to expose.

## Table-stakes comparison

| Capability | py-earth | R earth | parsnip `mars()` | Our decision |
| --- | --- | --- | --- | --- |
| Hinge basis `h(x−t)` / `h(t−x)`, forward knot search | yes | yes | yes | **in** — the core algorithm |
| Backward pruning pass selected by GCV | `enable_pruning` (default on) | `pmethod="backward"` | `prune_method` (default backward) | **in** — `prune` boolean, default on |
| Max terms from the forward pass | `max_terms` | `nk` | — | **in** — `max_terms`, default 21 |
| Max terms kept after pruning | — | `nprune` | `num_terms` | **in** — `nprune`, `0` = auto (best GCV) |
| Interaction degree | `max_degree` (default 1) | `degree` (default 1) | `prod_degree` (default 1) | **in** — `max_degree` 1–3, default 1 |
| GCV penalty per knot | `penalty` (default 3.0) | `penalty` | — | **in** — `penalty`, default 3.0 |
| Minimum span between knots | `minspan` (default auto) | `minspan` | — | **in** — `minspan`, `0` = auto (Friedman's formula) |
| End span (extreme values ineligible as knots) | `endspan` (default auto) | `endspan` | — | **in** — `endspan`, `0` = auto |
| Forward-pass stopping threshold on R² gain | `thresh` (default 0.001) | `thresh` | — | **in** — `thresh`, default 0.001 |
| Allow plain linear terms (no knot) | `allow_linear` (default on) | `linpreds`-adjacent | — | **in** — `allow_linear`, default on |
| Report RSS, R², GCV, GRSq | yes | yes | via metrics | **in** — all four, plus RMSE/MAE and term counts |
| Coefficient / basis-function table | `summary()` | `summary()` | — | **in** — term table + a readable equation |
| Variable importance | `feature_importance_type` (`rss`, `gcv`, `nb_subsets`) | `evimp()` | — | **in** — RSS-drop importance + per-variable term counts |
| Predict on new rows | `predict()` | `predict()` | `predict()` | **in** — `predict` param, one row per line |
| Choose target / feature columns of a pasted table | via the caller's dataframe | formula interface | recipe/formula | **in** — `target` (name, 1-based index or `last`) + `features` |
| Header handling for pasted data | n/a (dataframe) | n/a | n/a | **in** — `header` auto/yes/no, matching sibling blocks |
| Machine-readable output | Python objects | R objects | R objects | **in** — `format` text/csv/json |
| Fast forward pass heuristics (`use_fast`, `fast_K`, `fast_h`) | yes | yes | — | **out-of-model** — a speed heuristic that changes results; we instead thin knot candidates deterministically under a work budget and state the rule on the page |
| Missing-value handling (`allow_missing`) | yes | yes | — | **out-of-model** — we reject rows with non-numeric/missing cells with an explicit error rather than silently imputing |
| Classification / GLM mode (binomial family) | via wrapper | `glm=` | classification mode | **out-of-model** — this block is a regression tool; listed, not built |
| Smoothed (cubic) hinge output (`smooth`/`Earth` C2 basis) | `smooth` | `earth(..., Use.beta.cache)`-adjacent | — | **out-of-model** — piecewise-linear hinges only; stated on the page |
| Cross-validated pruning (`nfold`, `pmethod="cv"`) | — | yes | — | **out-of-model** — GCV pruning only; the sibling `regression-model-trainer` already ships k-fold CV for linear/ridge/forest |
| Multi-response (`y` matrix) models | yes | yes | — | **out-of-model** — one target column |

Every table-stake above is either in the descriptor or in the out-of-model list; none were
dropped silently.

## UX control patterns competitors ship (and what we do)

The competitors are libraries, not web forms, so the relevant UX comparison is against
tutorial-style parameter walkthroughs and our own sibling tool pages:

- **Degree / term-count are the two knobs every tutorial tunes first** (`degree` × `nprune`
  grids are the canonical MARS example). → both are first-class fields, `max_degree` and
  `nprune` rendered as sliders so the grid is one drag away.
- **Penalty, minspan, endspan, thresh are "advanced" knobs with auto defaults.** → exposed,
  but every one accepts the auto sentinel (`0`) or its documented default so a user never has
  to understand them to get a fit.
- **Presets:** tutorials habitually start from "additive model" then "degree-2 interactions".
  → shipped as `[[example]]` preset chips (piecewise curve, two-way interactions,
  forward-pass-only/no pruning, multi-feature table) so one click reproduces each scenario.
- **Sliders for bounded numerics** (`max_terms`, `nprune`, `max_degree`, `decimals`) and
  `[input.labels]` friendly labels on the `header`/`format` selects, matching
  `least-squares-regression`.
- **Worked example with real numbers** on the page — a knotted series where a straight line
  visibly fails and MARS recovers the breakpoint.

## Deliberate differences

- Knot candidates are thinned deterministically (minspan/endspan, then even spacing under a
  fixed work budget) so a browser tab stays responsive; the rule and the caps
  (5,000 rows / 50 columns / 1,000 prediction rows) are stated on the page rather than
  only in code.
- No randomness anywhere: same input → byte-identical output, unlike the sampling-based
  `fast_K` heuristics.
