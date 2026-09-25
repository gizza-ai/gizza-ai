## About this tool

Use this t-test calculator when you have one or two sets of numeric observations and need a transparent significance test you can copy into a report. It runs the one-sample t-test, the paired-samples t-test, the pooled two-sample Student test, and Welch's unequal-variance two-sample test. Paste raw rows from a spreadsheet, long `group,value` rows, or published summary statistics as `name,n,mean,sd`.

The report includes the sample sizes, means, standard deviations, standard errors, the estimated mean or mean difference, the t statistic, degrees of freedom, exact p-value, critical t at your alpha, confidence interval, Cohen's d, Hedges' g, observed power, and an equal-variance check for independent two-sample tests. The `auto` test choice uses one-sample for one column and Welch for two samples because Welch is usually safer when the variances are not known to be equal.

### Worked example

Paste this paired before/after table and keep `test = paired`, `format = wide`, `delimiter = comma`, and `header = yes`:

```text
before,after
12,10
14,11
11,10
15,12
13,12
16,13
```

The calculator treats each row as one matched pair and tests the mean of `before - after` against `mu = 0`. With the default two-tailed test at `alpha = 0.05`, the output reports the paired mean difference, the standard error of the differences, `t(5)`, the p-value, the confidence interval, and effect sizes based on the standard deviation of the differences.

### Input formats

- One column of numbers: one-sample test against `mu`.
- Two side-by-side columns: two independent samples, Welch by default, or matched pairs when `test = paired`.
- Long rows like `Control,6` and `Treatment,13`: independent samples grouped by label.
- Summary rows like `Control,12,5.2,1.9235`: use this when a paper gives `n`, mean, and sample standard deviation. If you only have standard error, convert first with `sd = sem × sqrt(n)`.

### Limits and edge cases

The input cap is 200,000 values. A t-test needs variation: a sample with zero standard deviation cannot produce a meaningful t statistic. Paired tests need equal-length raw columns unless you supply one column or one summary row of pre-computed differences. The calculator does not drop outliers or run a normality test automatically; check the data-generation process and remove observations only when you can justify that choice before testing. For three or more groups, use an ANOVA workflow instead of running many pairwise t-tests without a multiple-comparison plan.

## FAQ

<details>
<summary>Should I choose Student's two-sample test or Welch's test?</summary>

Use Welch's test unless you have a strong reason to assume equal population variances. It keeps fractional degrees of freedom and is more robust when the sample variances or sample sizes differ. The pooled Student test is still available for planned analyses that explicitly assume equal variances, and the output includes a variance-ratio check to help you document that assumption.

</details>

<details>
<summary>What does a left-tailed or right-tailed test change?</summary>

A two-tailed test asks whether the estimate is different from `mu` in either direction. A right-tailed test asks only whether it is greater than `mu`; a left-tailed test asks only whether it is less than `mu`. Pick a one-tailed direction before looking at the data. The calculator adjusts the p-value and reports a one-sided confidence bound for those runs.

</details>

<details>
<summary>Can I use summary statistics instead of raw data?</summary>

Yes for one-sample, pooled two-sample, and Welch tests. Use one or two rows shaped as `name,n,mean,sd`. A paired test can use a single summary row only when that row describes the paired differences themselves; two separate before/after summaries do not contain the pairing information, so the paired standard error cannot be recovered.

</details>

<details>
<summary>Why does the tool report both Cohen's d and Hedges' g?</summary>

Cohen's d standardizes the mean difference so effects can be compared across measurement scales. Hedges' g applies a small-sample bias correction to d. The output names the standardizer because one-sample, paired, pooled, and Welch tests use different denominators, and those choices can change the numeric effect size.

</details>
