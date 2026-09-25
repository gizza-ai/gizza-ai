## About this tool

Use this ROC AUC calculator when you have one continuous score per observation and a binary outcome label. Paste `score,label` rows from a model validation set, or paste the scores and labels as two separate columns, and the tool sweeps every observed cutoff to compute the area under the ROC curve, the chosen operating point, and the confusion-matrix metrics at that point.

The AUC is computed with the Mann-Whitney rank formula, so tied scores count as half a win, and it is cross-checked against the trapezoidal area under the ROC curve. The report also includes a DeLong standard error, a 90%, 95% or 99% confidence interval, a z-test against AUC = 0.5, the Gini coefficient, and a Brier score when every score is already in the 0–1 probability range.

By default the recommended cutoff maximizes Youden's J (`sensitivity + specificity - 1`). You can switch to F1, accuracy, closest-to-top-left, or a false-negative cost ratio, and you can enter your own threshold to see sensitivity, specificity, FPR, PPV, NPV, accuracy, F1, MCC, TP, FP, TN and FN there too.

### Worked example

Input:

```text
score,label
0.91,1
0.83,0
0.77,1
0.60,0
0.55,1
0.41,0
```

With the default settings, the report shows the AUC, a DeLong interval, the best cutoff by Youden's J, an optional ASCII ROC curve, and a compact threshold table. Set **Output** to CSV for a spreadsheet-friendly summary, or JSON when you want to feed the full sweep into another tool.

### Limits and edge cases

- Binary outcomes only. If you provide more than two labels, set **Positive class label** to score one class against all others.
- At most 20,000 observations are accepted.
- Both classes must be present; AUC is undefined for a single-class sample.
- DeLong intervals can collapse to zero width for perfectly separated or fully tied data; the AUC is still reported.
- Scores may be probabilities, percentages such as `90%`, or arbitrary risk scores. The Brier score is reported only when all scores are in the 0–1 range.

## FAQ

<details>
<summary>What is ROC AUC measuring?</summary>

ROC AUC is the probability that a randomly chosen positive example receives a higher score than a randomly chosen negative example, with tied scores counted as half. An AUC of 0.5 is chance ranking, 1.0 is perfect separation, and values below 0.5 usually mean the score direction is reversed.

</details>

<details>
<summary>Which threshold should I use?</summary>

The default Youden cutoff balances sensitivity and specificity. Choose F1 when positives are rare and precision matters, choose the cost option when a false negative has a known cost relative to a false positive, or enter your own threshold to audit an existing production cutoff.

</details>

<details>
<summary>Can I use text labels instead of 0 and 1?</summary>

Yes. Common labels such as `yes/no`, `case/control`, `disease/healthy`, `fraud/legit` and `positive/negative` are detected automatically. If your labels are custom or there are more than two labels, fill **Positive class label** with the event class you want to score.

</details>

<details>
<summary>Why is the confidence interval missing or very narrow?</summary>

The DeLong standard error is zero when the validation set is perfectly separated or every pair is tied in the same way. In that case the interval collapses to the AUC itself. Use a larger or less perfectly separated validation sample for a more informative interval.

</details>
