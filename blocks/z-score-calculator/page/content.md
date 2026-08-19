## About this tool

Z-Score Calculator converts between raw values, z-scores, normal-curve probabilities, and critical values. In the default mode, enter a mean `μ`, a standard deviation `σ`, and one or more raw scores. The output shows each score's z-score, percentile, left-tail probability, right-tail probability, and two-tailed p-value.

A z-score is the number of standard deviations a value is from the mean: `z = (x - μ) / σ`. Positive z-scores are above the mean, negative z-scores are below it, and `z = 0` is exactly at the mean. When **Sample size (n)** is greater than 1, the denominator becomes the standard error `σ / sqrt(n)`, which is the form used for testing a sample mean against a known population mean.

The other modes cover common lookup-table workflows: convert z-scores back to raw values, turn left-tail probabilities into critical z values, calculate the area between two bounds, or standardize a pasted dataset by deriving its own mean and standard deviation. Numbers may be separated by spaces, commas, semicolons, or newlines.

### Worked example

For an IQ-style scale with mean 100 and standard deviation 15, a score of 130 gives:

```text
x = 130
z = 2
percentile = 97.724987%
left tail P(X < x) = 0.977249868052
right tail P(X > x) = 0.0227501319482
two-tailed p = 0.0455002638964
```

That means 130 is two standard deviations above the mean, about the 97.7th percentile under the normal model. Use **Decimal places** to control display precision; very tiny tail probabilities keep significant digits rather than rounding all the way to zero.

This is a calculator for the normal-distribution arithmetic only. It does not decide whether your data are actually normal, and it does not replace study-specific statistical judgment.

## FAQ

<details>
<summary>What is the difference between z-score, percentile, and p-value?</summary>

The z-score is the standardized distance from the mean. The percentile is the left-tail area under the normal curve, so `z = 0` is the 50th percentile and `z ≈ 1.96` is about the 97.5th percentile. A p-value is a tail probability used for a hypothesis test; the two-tailed p-value reported here is `2 * min(left tail, right tail)`.

</details>

<details>
<summary>When should I set sample size n above 1?</summary>

Use `n > 1` when the value you entered is a sample mean, not a single observation, and you know the population standard deviation. The standard error is `σ / sqrt(n)`, so the same distance from the population mean becomes more unusual as the sample size grows. Leave `n = 1` for ordinary single-score z-scores.

</details>

<details>
<summary>What does critical mode expect?</summary>

Critical mode expects left-tail probabilities between 0 and 1. For example, `0.975` returns about `1.959964`, the familiar two-sided 95% cutoff, and `0.025` returns the matching negative value. It is the inverse of the standard normal CDF, not a percent string; enter `0.975`, not `97.5`.

</details>

<details>
<summary>How is dataset mode different from a full normalization tool?</summary>

Dataset mode derives the mean and standard deviation from the numbers you paste, then reports z-scores plus normal-curve probabilities for those values. It is intentionally narrow: min-max scaling, robust scaling, CSV column selection, and bulk feature preprocessing belong to dedicated normalization tools. Turn on **sample** when you want the sample standard deviation, dividing by `N - 1`.

</details>

<details>
<summary>What limits should I know about?</summary>

The input accepts up to 10,000 numbers. Standard deviation must be greater than zero, `n` must be at least 1, and decimal places are limited to 0 through 12. Between mode requires exactly two bounds, and critical mode rejects probabilities at or outside 0 and 1 because the corresponding z values are infinite.

</details>
