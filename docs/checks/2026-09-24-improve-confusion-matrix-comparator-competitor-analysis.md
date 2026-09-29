# confusion-matrix-comparator — competitor analysis (2026-09-24)

Scan run **before** implementation, per `/improve-tool` Phase 2–3. All profiles are paraphrased
from public pages/docs; **no competitor copy, branding or assets were reproduced**.

## Scope of the search

Queries: "compare two confusion matrices online", "confusion matrix calculator per-class metrics",
"model comparison baseline vs candidate confusion matrix diff", "McNemar test calculator compare
two classifiers". The search space is dominated by **single-matrix** calculators; the only
comparison-capable artifact found is a Python library, not a web tool.

## Profiles (top 5 real competitors)

### 1. Kanaries — Confusion Matrix Calculator (`ml.kanaries.net/tools/confusion-matrix-calculator`)
- **Inputs:** two modes — direct TP/FP/FN/TN entry (binary), or pasted `y_true` / `y_pred` label
  columns (multiclass, classes auto-discovered).
- **Metrics:** accuracy, precision, recall, specificity, F1 (binary/micro/macro/weighted), MCC,
  Cohen's kappa, per-class breakdown with support.
- **Output:** live recompute, colour-coded heatmap, per-class bars, CSV download, PNG export,
  JS + scikit-learn code snippets.
- **UX:** local-only processing, long educational FAQ.
- **Comparison of two matrices:** **no** — single-model analysis only.

### 2. induwara.lk — Confusion Matrix Calculator
- **Inputs:** four integer cells (TP/FP/FN/TN), an F-beta weight β, a positive-class label.
- **Metrics:** ~16 — accuracy, precision, recall, specificity, F1, F-beta, balanced accuracy,
  MCC, Cohen's kappa, informedness, NPV, FPR/FNR, FDR, prevalence.
- **Output:** rendered 2×2 grid, values plus the formula each came from; browser-local; no export.
- **UX:** preset examples (balanced / imbalanced / perfect / random), and explicit `undefined`
  labelling for zero-denominator edge cases instead of a misleading 0.
- **Comparison of two matrices:** **no**.

### 3. Sesen AI — F1 Score & Confusion Matrix Calculator
- **Inputs:** four cells, or pasted raw predictions.
- **Metrics:** accuracy, precision, recall, F1, MCC, Cohen's kappa (explicitly pitched around
  class imbalance).
- **Output:** heatmap plus a bar chart placing the metrics side by side.
- **UX:** worked tutorial link, FAQ.
- **Comparison of two matrices:** **no** (the "comparison" is across metrics, not across models).

### 4. `confusionmatrixonline.com` / marcovanetti.com cfmatrix
- **Inputs:** classifier results + ground-truth data.
- **Metrics:** overall accuracy and Cohen's kappa with a Landis & Koch agreement band.
- **Output:** plain numeric readout; no documented export.
- **UX:** minimal single-screen form. (`confusionmatrixonline.com` returned HTTP 526 during the
  scan, so only the sibling/marcovanetti page could be profiled.)
- **Comparison of two matrices:** **no**.

### 5. PyCM (Python library) — the only comparison-capable tool found
- **Inputs:** label vectors *or* a pre-computed matrix (dict/array); a `Compare` API takes
  **several** confusion matrices at once.
- **Behaviour:** aggregates overall + class-level benchmarks into composite scores and **ranks**
  the models, with optional per-class/per-criterion weighting.
- **Metrics:** very wide — per-class TPR/TNR/PPV/NPV/F-scores/MCC, overall accuracy, kappa with
  CI, chi-squared, entropy measures, Landis & Koch / Fleiss benchmark bands.
- **Output:** printed reports, matplotlib/seaborn plots, configurable decimal precision,
  normalisation, one-vs-all, matrix combination for mini-batches.
- **Not a web tool:** requires Python + install; no browser, no paste-and-go.

## Gap analysis → decisions

| # | Gap (from the scan) | Verdict | Where it landed |
|---|---|---|---|
| 1 | No browser tool compares **two** matrices entrywise | **in-model — the core differentiator** | `matrix_a`/`matrix_b`, the `Entrywise delta` grid, every metric reported as A / B / Δ |
| 2 | Per-class precision/recall/F1 with support (Kanaries, PyCM) | in-model | per-class delta table, `Support A/B` columns |
| 3 | Macro / weighted / micro averages (Kanaries) | in-model | overall table rows, each with a Δ |
| 4 | MCC + Cohen's kappa (all four calculators) | in-model | overall table rows (multiclass MCC + kappa) |
| 5 | Balanced accuracy, specificity (induwara) | in-model | balanced-accuracy row; binary block adds specificity when K = 2 |
| 6 | F-beta weight β (induwara) | in-model | `beta` param, 0.1–10; the per-class table header names the β actually used |
| 7 | Accepts pasted `y_true`/`y_pred` labels, not just a grid (Kanaries, Sesen, PyCM) | in-model | `input_format = matrix \| labels \| table \| auto` — a K×K grid, `actual,predicted` pairs, or `actual,predicted,count` triples, per matrix |
| 8 | Explicit `undefined` for zero-denominator cells (induwara) | in-model, adapted | per-class rates use the scikit-learn 0.0 convention **and** the report appends a footnote naming every class whose precision/recall/F-score was undefined — neither a silent 0 nor an unusable `n/a` in a delta column |
| 9 | Preset examples (induwara) | in-model | four `[[example]]` chips: regression, rare-class win, label-paste, JSON export |
| 10 | Ranking / "which model wins" composite (PyCM `Compare`) | in-model, scoped | `Verdict` line + `Biggest movers` section (top improvements/regressions by class and by cell) rather than an opaque composite score |
| 11 | Sort per-class rows by biggest mover | in-model (beyond every competitor) | `sort_by = class \| f1_delta \| precision_delta \| recall_delta \| support \| regression` |
| 12 | Significance of an accuracy difference (McNemar calculators) | **in-model, honestly scoped** | two-proportion z-test + CI on the accuracy difference (`significance`, `confidence_level`). **A paired McNemar test is impossible from two confusion matrices** — it needs the per-item agreement table — so the report and the page say so instead of computing a wrong number |
| 13 | Heatmap / bar chart / PNG export (Kanaries, Sesen) | **considered, rejected** | the page renders a text/markdown result surface; the entrywise delta grid + signed counts carry the same information, and a canvas renderer would need bespoke page JS against the shared generator |
| 14 | Code snippets for the same computation (Kanaries) | **considered, rejected** | the page already generates a runnable CLI example from the schema; a second hand-written snippet would drift |
| 15 | Composite benchmark bands (Landis & Koch, Fleiss — PyCM) | considered, rejected | interpretation bands for kappa are contested and add little to a *delta* report |
| 16 | Multi-model ranking of 3+ matrices (PyCM `Compare`) | **out-of-model for now** | the tool's contract is a pairwise diff; N-way would need a variable-arity input the page form can't express |
| 17 | Cloud batch / experiment tracking, accounts, saved runs | **out-of-model** | gizza is browser-local, no account, no server |

## What we ship that nobody in the scan does

- Entrywise `B − A` delta grid over the full K×K matrix, with the largest cell shifts called out.
- Every headline metric as a triple (A, B, Δ) rather than two reports the user must diff by eye.
- Per-class regression ranking, so a model that gains accuracy while destroying a minority class
  is visible in one line.
- Honest treatment of significance: an unpaired accuracy test, with the paired-test limitation
  stated rather than papered over.
- Four output formats (markdown / aligned text / CSV / JSON) and full CLI + deep-link parity.

Sources: [Kanaries](https://ml.kanaries.net/tools/confusion-matrix-calculator) ·
[induwara.lk](https://induwara.lk/tools/confusion-matrix-calculator) ·
[Sesen AI](https://sesen.ai/classification-metrics-calculator) ·
[marcovanetti cfmatrix](https://marcovanetti.com/pages/cfmatrix/) ·
[PyCM](https://www.pycm.io/doc/index.html)
