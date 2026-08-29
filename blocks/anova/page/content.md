## About this tool

This one-way ANOVA calculator compares the means of two or more independent groups. Paste raw observations in long form (`group,value`), a wide spreadsheet-style table with one column per group, or published summary rows (`name,n,mean,sd`). The calculation runs locally and returns the group means, sums of squares, degrees of freedom, mean squares, F statistic, right-tail p-value, and a plain-language decision at your chosen alpha.

It also reports practical follow-ups that many ANOVA calculators leave separate: eta-squared and omega-squared effect sizes, Welch's unequal-variance ANOVA, Brown-Forsythe Levene variance check, and optional pairwise comparisons with Tukey HSD/Tukey-Kramer, Fisher's LSD, Bonferroni, or Holm adjustment.

### Worked example

Input data in wide format, with one group per column:

```text
5,5,8
6,7,11
9,9,13
9,10,13
11,11,14
```

With alpha `0.05`, four decimals, and summary output, the report includes:

```text
One-way ANOVA
groups: 3
observations: 15
grand mean: 8.0000

F(2, 12) = 3.7371, p = 0.0547
critical F at alpha 0.0500 = 3.8853
result: p >= alpha 0.0500 -> fail to reject the null hypothesis
```

Choose **Tukey HSD** when you need pairwise comparisons after the omnibus test, **JSON** when another script needs the statistics, or **Markdown tables** when you want to paste the ANOVA table into a report.

## Limits & edge cases

- Maximum input size is 200,000 numeric values and 1,000 groups.
- The model is one-way ANOVA: one factor, independent groups. It does not run two-way, repeated-measures, mixed-effects, MANOVA, or ANCOVA designs.
- Parametric ANOVA assumes independent observations, roughly normal residuals within groups, and similar variances. Welch's ANOVA and Levene's test are included to help judge variance sensitivity, but they do not replace study design checks.
- Summary-statistics input cannot compute Levene's test or min/max/sum because the original observations are not available.
- If every value inside each group is identical, the within-group variance is zero and the F statistic is undefined.
- Auto-detection treats most two-column `label,value` data as long format, but numeric group labels can be ambiguous. Set **Input format** and **Header row** explicitly when needed.

## FAQ

<details>
<summary>What data layout should I paste?</summary>

Use **wide** format when each group is already a spreadsheet column. Use **long** format when each row has a group label and one value, such as `Control,6`. Use **summary** format only when you have `name,n,mean,sd` for each group instead of the raw observations.

</details>

<details>
<summary>Does a significant ANOVA say which groups differ?</summary>

No. The omnibus F test only says at least one group mean differs. Pick a post-hoc method, usually Tukey HSD for all pairwise comparisons, to see the pair table with adjusted p-values and confidence intervals.

</details>

<details>
<summary>When should I look at Welch's ANOVA?</summary>

Look at Welch's ANOVA when group variances are unequal or group sizes are unbalanced. The regular ANOVA table uses a pooled within-group mean square; Welch's test reweights groups by their variances and can be more reliable under heteroscedasticity.

</details>

<details>
<summary>Can I use this for repeated measurements from the same subjects?</summary>

No. Repeated-measures data violates the independent-groups model used here. Use a repeated-measures ANOVA or mixed model outside this tool when the same subject, batch, or unit contributes multiple rows across conditions.

</details>
