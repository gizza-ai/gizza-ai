# Confusion matrix competitor analysis — 2026-09-04

Tool: `confusion-matrix`

## Search

Query: `online confusion matrix calculator precision recall F1 accuracy`

Reviewed results:

| Competitor | Observed table stakes | Fit for gizza model | Decision |
| --- | --- | --- | --- |
| Omni Calculator confusion matrix calculator | Binary TP/FP/TN/FN entry, accuracy, precision, recall, F1, plain formulas/explanations. | In-model for binary counts and metrics; out-of-model for branded educational walkthroughs. | Support binary count matrices and diagnostic metrics; keep explanations original in page copy. |
| PythonAlchemist confusion matrix calculator | Precision, recall, F1, MCC, accuracy, formulas, step-by-step numeric output. | In-model for deterministic metrics; out-of-model for external Python execution or model training. | Include MCC, accuracy, F-score, JSON/CSV output for downstream use. |
| CalcBE confusion matrix calculator | Binary TP, FP, TN, FN, specificity, prevalence, accuracy, precision, recall, F1. | In-model. | Add binary summary with specificity, NPV, false rates, likelihood ratios, prevalence, diagnostic odds ratio, and Wilson intervals. |
| ConfusionMatrixOnline | Multiclass actual/predicted input, accuracy, precision, recall, F1. | In-model. | Support multiclass label lists and multiclass matrices with macro/weighted/micro averages. |
| Google ML classification metrics page | Definitions for accuracy, precision, recall, F1 and threshold/probability discussion. | Metric definitions in-model; threshold tuning, ROC/PR curves, probability calibration out-of-model for this pure text block. | Compute deterministic labels/counts only; document that threshold curves and model training are not included. |

## Table-stakes matrix

| Capability / UX pattern | In model? | Implementation decision |
| --- | --- | --- |
| Two separate actual and predicted label lists | Yes | `actual` required textarea plus optional `predicted` textarea; `input_format=labels` or auto when predicted is filled. |
| Paired actual/predicted table | Yes | `actual` accepts `actual,predicted[,count]`; optional weighted count column. |
| Existing confusion-matrix count grid | Yes | `input_format=matrix`; auto detects square numeric grids when predicted is empty. |
| Binary TP/FP/TN/FN calculations | Yes | Binary summary appears for two-class reports and can be steered with `positive_label`. |
| Multiclass matrix and classification report | Yes | Per-class precision/recall/F-score/support plus macro, weighted, and micro averages. |
| Accuracy, precision, recall, F1 | Yes | Core report. |
| Specificity, NPV, FPR/FNR/FDR/FOR, prevalence | Yes | Binary summary. |
| MCC and Cohen's kappa | Yes | Overall metrics; MCC also in binary summary. |
| Confidence intervals | Yes | Wilson 95% intervals for binary accuracy, precision, recall, specificity, and NPV. |
| Normalized matrix views | Yes | `normalize` enum: none, row, column, all. |
| F-beta option | Yes | `beta` slider 0.1–10; F1 is beta 1. |
| Decimal places and percent display | Yes | `decimals` slider 0–10 and `percent` checkbox. |
| Markdown, plain text, CSV, JSON export | Yes | `format` enum. |
| Preset examples | Yes | Page chips for spam counts, two lists, and matrix grid. |
| ROC/PR curves, threshold selection, probability calibration | No | Requires score arrays and plotting; listed as out-of-scope. |
| Model training or automatic label prediction | No | Gizza block is deterministic; it only evaluates labels/counts supplied by the user. |
| Heatmap image rendering | No for this text block | Could be a future SVG/PNG visualization block; current output remains text/markdown/CSV/JSON. |

## Descriptor/page decisions

- Fixed choices use `Param::enumv`: input format, separator, header, normalization, output format.
- Numeric controls use sliders in page metadata for beta and decimal places.
- The page includes textarea placeholders for both label-list and weighted-table workflows.
- Examples cover a binary count table, two pasted label lists with row normalization, and an existing matrix grid.
- Copy avoids competitor trademarks beyond factual names in this analysis file and avoids site branding under the page directory.
