## About this tool

**Z-score normalize** standardizes a list of numbers so they can be compared on a
common scale — a routine first step for statistics, data analysis, and machine
learning feature scaling. It offers four common scaling methods:

- **Z-score (standardization)** — subtracts the mean and divides by the standard
  deviation, so the output has **mean 0 and standard deviation 1**. Each result is
  the *standard score* (how many standard deviations a value sits above or below
  the mean). By default it uses the **population** standard deviation (÷N),
  matching scikit-learn's `StandardScaler` and the NumPy default; tick the box to
  use the **sample** standard deviation (÷N−1) instead.
- **Min-max scaling** — linearly rescales values into the **0–1 range**, so the
  smallest value maps to 0 and the largest to 1. Useful when you need a bounded
  range rather than a zero-centred distribution.
- **Max-abs scaling** — divides every value by the largest absolute value, so the
  output stays in **−1 to 1** while preserving signs. It is useful for sparse or
  already-centred data where zeros should remain zeros.
- **Robust scaling** — subtracts the median and divides by the **interquartile
  range (Q3−Q1)**. It is less sensitive to outliers than mean/std-dev scaling.

Paste your numbers (separated by spaces, commas, semicolons, or newlines), choose
a method, and the tool returns the transformed values in the original order, plus
the parameters it used (mean and standard deviation, min/max, max abs, or
median/IQR).

## How to use it

1. Paste or type your numbers into the **Numbers** box.
2. Choose the **Method** — `z-score`, `min-max`, `max-abs`, or `robust`.
3. For z-score, tick **Use sample standard deviation (÷N−1)** if your data is a
   sample rather than the full population.
4. Read off the normalized values, one per line.

## Which scaling method should I use?

- Use **z-score** when you care about how far each value is from the mean in
  standard-deviation units (e.g. detecting outliers, or feeding features to models
  that assume roughly zero-centred inputs like SVMs, logistic regression, or PCA).
- Use **min-max** when you need values in a fixed, bounded range (e.g. 0–1 for a
  neural-network input layer, or a progress/intensity value).
- Use **max-abs** when signs matter and zeros should stay zero, especially for
  sparse numeric features.
- Use **robust** when outliers would distort the mean and standard deviation; it
  anchors around the median and IQR instead.

## Population vs. sample standard deviation

The **population** standard deviation divides the summed squared deviations by *N*;
the **sample** standard deviation divides by *N−1* (Bessel's correction), giving an
unbiased estimate when your numbers are a sample drawn from a larger population.
scikit-learn's `StandardScaler` and NumPy's `std()` default to the population form,
so that is the default here.

## Common uses

- **Feature scaling** before training a machine-learning model.
- Computing **standard scores (z-scores)** for exam grades, test results, or
  benchmarks.
- **Outlier detection** — values with a large absolute z-score are unusual.
- Rescaling metrics into a common **0–1** or **−1 to 1** range for dashboards or
  comparisons.

## Privacy

Everything runs locally in your browser via WebAssembly. Your numbers are never
uploaded to a server.
