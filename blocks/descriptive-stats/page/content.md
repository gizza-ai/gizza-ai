## About this tool

**Descriptive statistics** summarises a list of numbers with the measures you'd
reach for in a stats class or a quick data check:

- **Centre:** mean, median, mode (the most frequent value(s) — or none if all are
  unique)
- **Spread:** variance and standard deviation — both **population** (÷N) and
  **sample** (÷N−1) — plus range and the **interquartile range (IQR)**
- **Position:** min, max, first quartile (Q1) and third quartile (Q3)
- **Totals:** count and sum

Enter the numbers separated by spaces, commas, semicolons, or newlines. Quartiles
use the common linear-interpolation method (the numpy default).

### Privacy

Everything runs **in your browser** via WebAssembly — your data is never uploaded.
Also available from the [gizza CLI](/) and in chat (which return the values as
structured JSON).

## FAQ

<details>
<summary>Why do I get two different standard deviations?</summary>

The tool reports both conventions: **population** standard deviation divides by
N (use it when your numbers are the whole population) and **sample** standard
deviation divides by N−1 (use it when they're a sample of something larger).
With fewer than 2 numbers the sample figures are undefined and shown as blank.

</details>

<details>
<summary>My quartiles don't match Excel — which method is used?</summary>

Q1, the median and Q3 use **linear interpolation** on the sorted data — the
same method as numpy's default `percentile`. Excel's `QUARTILE.INC` matches
it, but `QUARTILE.EXC` and some textbooks use different conventions, so small
differences on short lists are expected and not a bug.

</details>

<details>
<summary>What does "mode: none" mean, and can there be several modes?</summary>

If no value appears more than once there is no mode, so the tool says *none*
rather than picking one arbitrarily. When several values tie for the highest
frequency, all of them are listed (a multimodal dataset).

</details>

<details>
<summary>How precise are the results?</summary>

Values are computed in 64-bit floating point and rounded to **6 decimal
places** for display. Inputs must be finite numbers — `NaN`, `inf` or stray
text produce an error naming the offending token rather than a silent skip.

</details>
