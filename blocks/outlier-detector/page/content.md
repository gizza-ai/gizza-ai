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
