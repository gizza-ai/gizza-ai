## What this tool does

The logit transform turns a probability into log-odds:

```text
logit(p) = log(p / (1 - p))
```

That is the scale used by logistic regression coefficients, risk models, and many calibration plots. A probability of `0.5` becomes `0`, `0.75` becomes about `1.0986`, and `0.25` becomes about `-1.0986`. The inverse logit (also called sigmoid or expit) turns log-odds back into a probability.

Paste one value or a whole spreadsheet column. The tool accepts decimals such as `0.9` and percentages such as `90%`, can mirror commas/tabs/newlines on output, and can return either just the transformed values or an audit table showing the odds beside each row.

## Worked examples

Probability to logit, rounded to four decimals:

```text
0.5
0.75
0.25
```

becomes:

```text
0.0000
1.0986
-1.0986
```

Inverse logit with the same natural-log base:

```text
0
1
-1
```

becomes:

```text
0.5000
0.7311
0.2689
```

Switch **Output** to **Audit table with odds** when you want to see the middle step. For `0.75`, odds are `3` and the natural-log logit is `1.0986`.

## Limits and behaviour to know about

- Maximum input: **20,000 values**.
- In logit mode, probabilities must be between `0` and `1`. Use a `%` suffix for percentages: `90%`, not bare `90`.
- The logit is undefined at exactly `p=0` and `p=1`. The default is to fail clearly; alternatives can clamp to epsilon, skip, blank, or emit `-Infinity` / `Infinity`.
- Base `e` is the statistical default. Base `2` and `10` are available for workflows that report binary or common-log odds.
- This is a value transform, not a logistic-regression fitter. It does not estimate coefficients, p-values, confidence intervals, or AIC.

## FAQ

<details>
<summary>When should I use logit instead of the probability itself?</summary>

Use logit when you need an unbounded scale for probabilities. Raw probabilities are stuck between `0` and `1`, while log-odds can range from `-Infinity` to `Infinity`, which is why logistic regression models are linear on the logit scale. A positive logit means the event is more likely than not, a negative logit means less likely than not, and `0` means exactly 50%.

</details>

<details>
<summary>Why does the tool reject p=0 and p=1 by default?</summary>

Because `log(p / (1 - p))` has a zero in the ratio at one endpoint and a zero in the denominator at the other. The mathematical limits are `-Infinity` and `Infinity`, but silently emitting infinities can break spreadsheets and downstream models. Choose **Clamp** for the common smoothing fix, or **Infinity** when you explicitly want the mathematical limit.

</details>

<details>
<summary>What is epsilon and how should I choose it?</summary>

Epsilon is only used with **Clamp**. A probability of `0` becomes `epsilon`, and a probability of `1` becomes `1 - epsilon` before the logit is taken. The default `0.000001` is conservative for pasted probabilities. If your data came from counts, choose an epsilon that matches your sample-size convention instead of treating it as universal truth.

</details>

<details>
<summary>Can I paste percentages?</summary>

Yes. Add the percent sign: `10%`, `50%`, `90%`. Bare `90` is rejected in logit mode because it is outside the probability range and might mean either 90 percent or a mistaken odds value. The error message tells you to append `%` or divide by 100.

</details>

<details>
<summary>Does this fit a logistic regression?</summary>

No. This tool transforms values you already have: probabilities to logits, or logits back to probabilities. It does not fit coefficients, compute standard errors, or produce model diagnostics. Use it after a model has produced probabilities/log-odds, or when preparing a column for another workflow.

</details>
