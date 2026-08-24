# decision-tree-classifier — competitor analysis (2026-08-21)

Scan run BEFORE implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4. All findings
are paraphrased observations of publicly documented behaviour — no competitor copy, branding, or
trademarked wording is reproduced or reused in the block, its page, or its docs.

## Duplicate / viability check (done first)

`ls blocks/ | grep -iE 'decision|tree|classif|regress|cluster|naive|bayes|forest|train'` surfaced four
near neighbours; each was read before deciding to build:

| Existing block | Why it is NOT this tool |
| --- | --- |
| `regression-model-trainer` | Regression only — numeric target, R²/RMSE/MAE, OLS/ridge/random forest. `grep -c 'rule\|accuracy\|confusion\|gini\|entropy'` on its core returns **0**: no classification target, no impurity criterion, no accuracy/confusion matrix, and no rule extraction. Its forest is an averaging ensemble, deliberately not interpretable. |
| `naive-bayes-text-classifier` | Text input (labelled documents → bag of words), not a tabular feature table; probabilistic, produces no splits or rules. |
| `data-clusterer` | Unsupervised (KMeans/DBSCAN/hierarchical) — no labelled target. |
| `stepwise-feature-selection` / `correlated-feature-pruner` / `train-test-split` | Pre-modelling helpers over a table; none fit a classifier. |

Viability: fully in the current gizza model. The tool *trains* a small tree from user-supplied data
with deterministic pure-Rust arithmetic (same class as `regression-model-trainer`); it ships no
pre-trained weights and needs no ML runtime. Zero dependencies beyond the core crate itself.

## Competitors reviewed

1. **scikit-learn `DecisionTreeClassifier` + `export_text` / `export_graphviz`** — the de-facto
   reference implementation (CART). Documented constructor defaults: `criterion='gini'`
   (also `entropy`/`log_loss`), `splitter='best'`, `max_depth=None`, `min_samples_split=2`,
   `min_samples_leaf=1`, `min_impurity_decrease=0.0`, `max_features=None`, `class_weight=None`
   (or `balanced`), `ccp_alpha=0.0`, `random_state=None`. Exposes `feature_importances_`
   (impurity-decrease shares) and a plain-text tree renderer; `export_graphviz` emits DOT.
2. **PlanetCalc "Decision tree builder" (calculator 8443)** — the closest *browser, paste-a-table,
   no-signup* competitor. Accepts a semicolon-separated CSV whose first row is the attribute labels
   followed by the class label; builds an ID3-style tree using information gain; renders the tree
   diagram; ships a classic weather/"play" demo dataset. Categorical attributes only, multi-way
   splits, no tunable stopping rules, no accuracy report.
3. **Weka `J48` (C4.5)** — documented flags: `-C 0.25` confidence factor for pruning, `-M 2` minimum
   instances per leaf, `-U` unpruned, `-B` binary splits only for nominal attributes, `-R` reduced-error
   pruning with `-N 3` folds, `-S` subtree raising, `-A` Laplace smoothing, `-Q 1` seed. Output is a
   text tree plus a training/holdout evaluation summary with a confusion matrix.

(A fourth angle, `chefboost`, was skimmed for the rule-extraction convention — it emits the tree as
if/then rules rather than a diagram, confirming rules-as-primary-output is a real expectation.)

## Table stakes → decision

| Capability | Seen in | In model? | Decision |
| --- | --- | --- | --- |
| Paste a delimited table, header row optional | all 3 | yes | `data` + `header=auto\|yes\|no`; comma/tab/semicolon/pipe/whitespace auto-detected (PlanetCalc is semicolon-only — ours is a superset) |
| Choose the class/target column | sklearn, Weka | yes | `target` = `last`/`first`/1-based index/header name (PlanetCalc hard-codes "last column") |
| Choose a feature subset | sklearn, Weka | yes | `features` (blank = every non-target column) |
| Split criterion gini / information gain | sklearn (gini, entropy), PlanetCalc (gain) | yes | `criterion = gini \| entropy \| gain_ratio` — CART, ID3 and the C4.5 gain-ratio correction, matching the three algorithm families in the backlog row |
| Binary vs multi-way splits on categorical features | Weka `-B`, PlanetCalc (multi-way) | yes | `splits = binary \| multiway` |
| Numeric features with threshold splits | sklearn, Weka | yes | numeric columns auto-detected; midpoint threshold search (PlanetCalc handles categorical only) |
| `max_depth` | sklearn, Weka (indirect) | yes | `max_depth` (1–20, default 5 — bounded so a pasted table can't blow up a browser tab) |
| `min_samples_split` | sklearn | yes | `min_samples_split` (default 2) |
| `min_samples_leaf` / `-M` | sklearn, Weka | yes | `min_samples_leaf` (default 1) |
| Pruning knob | sklearn `min_impurity_decrease`/`ccp_alpha`, Weka `-C` | partly | `min_gain` (pre-pruning by minimum impurity decrease). Cost-complexity (`ccp_alpha`) and C4.5 confidence-factor *post*-pruning are listed out-of-model below |
| Class weighting for imbalanced data | sklearn `class_weight='balanced'` | yes | `class_weight = none \| balanced` |
| Human-readable if/then rules | chefboost, implied by sklearn `export_text` | yes | **headline output** — numbered `IF … AND … THEN class` rules with support and purity |
| Text tree rendering | sklearn `export_text`, Weka | yes | box-drawing tree in the text report |
| Feature importance | sklearn `feature_importances_` | yes | normalised impurity-decrease shares |
| Accuracy + confusion matrix | Weka | yes | training accuracy and confusion matrix, plus an optional hold-out |
| Hold-out evaluation | Weka (percentage split), sklearn (`train_test_split`) | yes | `test_split` 0–0.5 + `seed` (deterministic shuffle) |
| Predict new rows | sklearn `.predict` | yes | `predict` param — paste unlabelled rows, get class + leaf confidence + the rule that fired |
| Graphviz DOT export | sklearn `export_graphviz` | yes | `format = dot` |
| Machine-readable output | sklearn (objects), Weka (files) | yes | `format = text \| json \| csv \| dot` |
| Reproducibility | sklearn `random_state`, Weka `-Q` | yes | `seed` (the split shuffle is the only randomness; tree fitting itself is fully deterministic) |
| Rendered tree *image* / interactive diagram | PlanetCalc, Weka GUI | no | **out of model** — the page renders one text output; DOT export is the honest substitute (paste into any Graphviz renderer) |
| Cost-complexity pruning (`ccp_alpha`) and C4.5 confidence-factor / reduced-error / subtree-raising post-pruning | sklearn, Weka | no | **out of model for this build** — needs a full prune-path or a validation-fold pruning loop on top of the fitted tree; the pre-pruning knobs (`max_depth`, `min_samples_split`, `min_samples_leaf`, `min_gain`) cover the same overfitting control with far less machinery. Listed, not built. |
| `max_features` / random splitter | sklearn | no | **out of model here by design** — randomised split selection belongs to an ensemble; `regression-model-trainer` already ships the random-forest style. A single tree here is always the best-split (`splitter='best'`) tree so the printed rules are stable. |
| Cross-validation folds | Weka `-x` | no | **not built** — `test_split` gives an honest hold-out; k-fold on a classifier would report an averaged accuracy with no single tree to print, which conflicts with the rules-first purpose. |
| Missing-value surrogate splits | Weka/C4.5 fractional instances | no | **out of model** — rows with missing values in a selected column are dropped and the count is reported, matching `regression-model-trainer`'s convention. |

## UX control patterns adopted

- PlanetCalc and Weka both ship a ready-made demo dataset so the tool is non-empty on arrival —
  mirrored as `[[example]]` preset chips (a small categorical weather-style table, a numeric-threshold
  table, and a rules-plus-prediction run). The datasets are freshly written for this repo, not copied.
- sklearn's knobs are numeric spinners; the page renders `max_depth`, `min_samples_split`,
  `min_samples_leaf`, `min_gain`, `test_split` and `decimals` as `kind = "slider"` mirrors, and uses
  `[input.labels]` to give the `criterion`/`splits`/`class_weight`/`header`/`format` enums friendly
  option text while keeping the canonical values.
- Every text/number field carries a placeholder showing a real value.

## Where ours is ahead

Three criteria (gini / information gain / gain ratio) and both split shapes in one tool, mixed
numeric+categorical features, rules **and** a text tree **and** DOT in one run, a prediction surface
for new rows, and a fully local pure-Rust/WASM run — no upload, no signup, no Python.
