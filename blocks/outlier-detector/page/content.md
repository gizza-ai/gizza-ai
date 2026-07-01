## About this tool

**Outlier detection** flags the values in a list of numbers that sit unusually far
from the rest — the readings you'd want to inspect, clean, or explain before
trusting a summary statistic. This tool applies the two methods most commonly
taught and used:

- **Z-score (standard-score) method.** A value is flagged when its distance from
  the mean, measured in **sample standard deviations** (÷ N−1), exceeds the
  threshold (default **3**). Best for roughly bell-shaped data; sensitive to the
  very outliers it looks for, so a few extreme points can mask others.
- **Modified z-score (MAD) method.** A robust variant that replaces the mean and
  standard deviation with the **median** and the **median absolute deviation
  (MAD)**: a value is flagged when **|0.6745·(x − median) / MAD|** exceeds the same
  threshold. Because the median and MAD aren't dragged around by extreme points,
  this method flags outliers that can mask each other under the classical z-score.
- **IQR method (Tukey's fences).** A value is flagged when it falls below
  **Q1 − k·IQR** or above **Q3 + k·IQR**, where IQR = Q3 − Q1 and the multiplier
  **k** defaults to **1.5** (the classic Tukey boxplot rule). Distribution-free
  and resistant to extreme values.

Every method reports the flagged values together with their **position (index)** in
your list, plus the underlying numbers — mean and standard deviation for the z-score
method, median and MAD for the modified z-score method, and the quartiles and fences
for the IQR method — so you can see exactly why each point was flagged.

Enter the numbers separated by spaces, commas, semicolons, or newlines. Tune the
z-score threshold and the IQR multiplier to make detection stricter (lower) or more
lenient (higher).

### Privacy

Everything runs **in your browser** via WebAssembly — your data is never uploaded.
Also available from the [gizza CLI](/) and in chat (which return the flagged values
and the supporting statistics as structured JSON).

## FAQ

<details>
<summary>Why do the three methods flag different values?</summary>

Because they make different assumptions. The classical z-score uses the mean
and standard deviation, both of which are pulled toward extreme points — so a
big outlier can inflate the standard deviation enough to hide itself or
others (masking). The modified z-score (median + MAD) and the IQR fences are
robust to this, so on skewed or contaminated data they typically flag more of
the genuinely extreme points. Disagreement is a signal, not a bug.

</details>

<details>
<summary>Why does the z-score method report nothing for my data?</summary>

Three common reasons: the sample has fewer than 2 values (the sample standard
deviation, computed with ÷ N−1, is undefined), all values are identical (zero
spread), or the outlier itself inflated the standard deviation past the
threshold. Similarly, the modified z-score flags nothing when the MAD is 0 —
i.e. more than half your values are identical.

</details>

<details>
<summary>Will the quartiles match what numpy or my textbook gives?</summary>

Quartiles use the linear-interpolation method — numpy's default — and the
z-scores use the sample standard deviation, matching
`scipy.stats.zscore(ddof=1)`. Textbooks that use a different quartile rule
(there are several) can produce slightly different fences on small samples.

</details>

<details>
<summary>How do the two tuning knobs interact?</summary>

The **z threshold** (default 3, must be > 0) is shared by both the classical
and the modified z-score methods. The **IQR multiplier k** (default 1.5)
only affects Tukey's fences — 1.5 is the classic boxplot rule, and raising it
to 3 flags only "far out" points. Lower values make every method stricter.

</details>
