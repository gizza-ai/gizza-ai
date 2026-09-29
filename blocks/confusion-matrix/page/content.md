## About this tool

Confusion Matrix Calculator turns actual labels and model predictions into a full
classification report. Paste two label lists, paste an `actual,predicted` table,
or paste a square grid of counts you already computed. The tool returns the
matrix plus precision, recall, F-score, support, accuracy, balanced accuracy,
Cohen's kappa, Matthews correlation coefficient, and per-class one-vs-rest
counts.

Binary classifiers also get a diagnostic summary for the positive class: TP, FP,
FN, TN, sensitivity, specificity, negative predictive value, false-positive and
false-negative rates, likelihood ratios, diagnostic odds ratio, Youden's J, and
Wilson 95% confidence intervals for the main proportions.

### Worked example

Input table:

```csv
actual,predicted,count
spam,spam,180
spam,ham,20
ham,spam,40
ham,ham,760
```

With positive class `spam` and percent output enabled, the report includes:

```text
accuracy                         94.0000%  92.3529% – 95.3104%
precision (PPV)                  81.8182%  76.1900% – 86.3542%
recall (sensitivity, TPR)        90.0000%  85.0594% – 93.4330%
specificity (TNR)                95.0000%  93.2630% – 96.3069%
f1-score                         85.7143%
```

Use **Normalize matrix** to view counts as row rates, prediction-column rates, or
share of all observations. Use **F-beta weight** above 1 when recall matters more
than precision.

## Limits & edge cases

- Maximum 500,000 weighted observations and 200 distinct labels per run.
- The tool is deterministic and local. It does not train a model, choose a
  classification threshold, draw ROC/PR curves, or inspect probability scores.
- `auto` input detection uses two label lists when the predicted box is filled.
  When it is empty, it detects square numeric grids as matrices and otherwise
  reads an actual/predicted table.
- Undefined metrics are shown as `n/a`; macro and weighted averages treat them
  as zero, matching the common `zero_division=0` reporting convention.
- For a bare two-by-two grid without labels, classes are named `positive` and
  `negative`. Provide **Class order** to use your own names.

## FAQ

<details>
<summary>Can I use this for multiclass classification?</summary>

Yes. Paste any number of class labels up to the 200-label cap. The matrix is
rendered with one row per actual class and one column per predicted class, and
the report includes per-class precision, recall, F-score, support, macro,
weighted, and micro averages.

</details>

<details>
<summary>What input formats are accepted?</summary>

You can paste actual labels and predicted labels as two separate lists, paste a
two-column `actual,predicted` table with an optional count column, or paste a
square confusion-matrix grid of counts. Separators can be auto-detected or forced
to newline, comma, tab, semicolon, pipe, or spaces.

</details>

<details>
<summary>How is the positive class chosen for binary metrics?</summary>

If you provide **Positive class**, that class is used. Otherwise, for two-class
problems the tool prefers common positive names such as `1`, `true`, `yes`,
`positive`, `spam`, `fraud`, or `malignant`; if none match, it uses the second
class in the displayed order.

</details>

<details>
<summary>Why do some metrics show n/a?</summary>

A metric is undefined when its denominator is zero, such as precision for a class
that was never predicted. The report shows `n/a` rather than silently inventing a
zero. Average rows use zero for undefined per-class metrics so they stay
comparable with common classification-report defaults.

</details>
