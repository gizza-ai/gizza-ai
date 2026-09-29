# roc-auc-calculator — competitor analysis (2026-09-24)

Scan run **before** implementation, per the create-next-tool recipe. All observations are
paraphrased from public tool pages; no competitor copy, branding, or trademarks are reproduced.
Out-of-model items are listed for the record, not built.

## Scope check (not a duplicate)

`blocks/confusion-matrix` already exists, but it consumes **hard labels at one fixed decision
point** (actual vs predicted class) and reports Youden's J for that single point. A ROC/AUC tool
consumes **continuous scores** and sweeps every candidate cutoff to produce a curve, an area, and
a recommended threshold. Different input type, different computation, different output. Built.

## Competitors reviewed

| # | Tool | Input | Headline outputs |
|---|------|-------|------------------|
| 1 | r-statistics.co ROC/AUC calculator | two pasted columns (outcome + score); whitespace/comma/semicolon separated | AUC with DeLong CI, three optimal thresholds (Youden, F1, cost-weighted), Brier score, confusion matrix at a draggable threshold, calibration deciles |
| 2 | StatsCalculators.com ROC curve generator | spreadsheet-style grid; choose label column + score column | ROC plot, AUC, Youden threshold, sensitivity, specificity, PPV, NPV, accuracy, per-point thresholds |
| 3 | MetricGate ROC-AUC calculator | binary outcome column + numeric score column | AUC, DeLong CI at a selectable confidence level, z-test vs AUC = 0.5, Youden threshold with sensitivity/specificity/PPV/NPV/F1, TP/FP/FN/TN, plain-language AUC band |
| 4 | CalcBE ROC AUC calculator | one `score,label` pair per line, optional positive-label override | AUC, best threshold, sensitivity/specificity at it, positive class, sample count, and a full threshold table (threshold ≥, sens, spec, FPR, TP, FP, TN, FN) |

## Table stakes → decision

| Capability | Seen in | Decision |
|---|---|---|
| `score,label` rows pasted as one block | 1, 4 | **in-model** → `input_format = pairs` |
| Two separate columns/lists (scores, labels) | 1, 2 | **in-model** → `input_format = columns`, second `labels` field |
| Column order varies (`score,label` vs `label,score`) | 1, 4 | **in-model** → `column_order` auto/score_label/label_score |
| Mixed separators (comma, tab, semicolon, pipe, whitespace) | 1, 2 | **in-model** → `separator` enum with auto-detect |
| Header row tolerated | 2 | **in-model** → `header` auto/yes/no |
| Non-numeric labels (yes/no, case/control, disease/healthy) + positive-label override | 2, 3, 4 | **in-model** → `positive_label`, plus auto-detection of common positive tokens |
| AUC, tie-correct | all | **in-model** → Mann-Whitney/midrank AUC (ties count as ½), cross-checked against the trapezoidal area over the curve |
| DeLong confidence interval | 1, 3 | **in-model** → fast DeLong via midranks, `confidence_level` 90/95/99 |
| z-test / p-value vs AUC = 0.5 | 3 | **in-model** → z from the DeLong SE, two-sided normal p |
| Optimal threshold by Youden's J | all | **in-model** → default `optimize = youden` (also the backlog spec) |
| Alternative criteria (F1, cost-weighted, closest-to-top-left, accuracy) | 1 | **in-model** → `optimize` enum + `cost_ratio` (FN:FP) |
| Metrics at the chosen cutoff: sens, spec, FPR, PPV, NPV, accuracy, F1, TP/FP/TN/FN | 1–4 | **in-model** → full summary block |
| Threshold table across cutoffs | 1, 4 | **in-model** → `table_rows` cap, evenly sampled, optimal row always kept and starred |
| Evaluate one user-chosen cutoff | 1 (slider) | **in-model** → optional `threshold` field reporting the same metric set at that cutoff |
| Brier score / calibration measure | 1 | **in-model** → reported when every score is in 0–1 |
| Plain-language AUC band | 3 | **in-model** → original wording over the conventional 0.7/0.8/0.9 bands |
| Decimal-place control | 4 | **in-model** → `decimals` slider |
| Preset example loaders (fraud, screening, ideal, coin-flip) | 1, 4 | **in-model** → `[[example]]` chips |
| Copy result / shareable URL / reset | 1, 4 | **already platform** → generator ships Copy, Reset, and query-param deep links |
| ROC curve visual | 1, 2, 4 | **in-model, reduced** → optional fixed-width ASCII plot (`plot`); an interactive SVG chart is out-of-model here |
| Multiple output formats (markdown/text/CSV/JSON) | — | **in-model** (gizza family invariant) |
| Percent-formatted rates | — | **in-model** → `percent` |

## Considered, rejected

- **Calibration decile / reliability table** (r-statistics). In-model to compute, but it is a second
  table on an already large output and the headline calibration number (Brier score) is reported.
  Declined for schema/output bloat.
- **Draggable threshold marker + live histogram** (r-statistics). The interaction needs bespoke
  per-tool page JS; the declarative equivalent — a `threshold` field that reports the same metrics
  at any cutoff — covers the underlying need.
- **CSV file upload with a column picker** (2). The paste box plus `column_order`/`header`/
  `separator` handles pasted spreadsheet columns without a file-picker UI.

## Out of model (listed, not built)

- Interactive/zoomable ROC and precision-recall plots, score histograms by class (needs a charting
  runtime the generic page driver does not have).
- DeLong two-model comparison test (needs two score columns; would double the input surface).
- Bootstrap confidence intervals (thousands of resamples; the closed-form DeLong interval is the
  one competitors report anyway).
- Accounts, saved datasets, server-side export — gizza tools are browser-local and account-free.

## Stated limits we will publish on the page

- Binary problems only (multiclass ROC is out of scope — competitor 4 states the same boundary).
- Maximum 20,000 observations.
- Both classes must be present; AUC is undefined with only positives or only negatives.
