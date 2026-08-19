## About this tool

Principal component analysis rotates a table of correlated variables into a new set of
uncorrelated axes — the principal components — ordered so the first one captures as much of
the spread in the data as possible, the second as much of what is left, and so on. It is the
standard first move for dimensionality reduction, for spotting which measurements move
together, and for compressing a wide table into two or three plottable coordinates.

Paste your matrix and this calculator does the whole decomposition in the browser: it centers
each column, standardizes it by default so the analysis runs on the **correlation matrix**,
diagonalizes that matrix with the Jacobi eigenvalue algorithm, and reports

- the **eigenvalue** of every component (the variance along that axis) with its proportion,
  percentage and cumulative share of the total variance;
- a text **scree plot**, so the drop-off you pick the component count at is visible without a chart;
- how many components you need for 90%, 95% and 99% of the variance, plus the **Kaiser count**
  (components with an eigenvalue above 1);
- the **loadings** — each original variable's weight in each component, which is what you read to
  name a component;
- the **scores** — every observation projected onto the components, ready to scatter-plot.

Nothing is uploaded; the maths runs entirely in your browser as WebAssembly, so the same numbers
come out on the page, in the CLI and in chat.

### Worked example — body measurements

Paste six people measured on three variables (the first row is read as a header of column names):

```text
height,weight,age
170,65,30
180,80,42
165,59,25
175,72,35
190,95,50
160,54,22
```

The report opens with

```text
PCA on 6 observations × 3 variables (correlation matrix — columns standardized to unit variance)

Explained variance:
  component   eigenvalue   proportion    percent   cumulative
  PC1           2.992685     0.997562   99.7562%     0.997562
  PC2           0.004537     0.001512    0.1512%     0.999074
  PC3           0.002778     0.000926    0.0926%            1
```

PC1 holds 99.76% of the variance, and its loadings — `height 0.577`, `weight 0.578`,
`age 0.577` — are near-identical and all positive. That is the signature of a single "size"
factor: in this sample, taller people are also heavier and older, so one number per person
replaces three with almost no loss. The scores column then ranks the six observations along
that axis, from `-2.01` to `+2.69`.

### Worked example — keeping the top 2 components

Set **Components to report** to `2` on exam marks in four subjects:

```text
maths,physics,english,history
78,74,55,58
92,95,61,60
55,52,80,84
61,58,88,90
84,88,64,62
49,45,91,95
```

PC1 takes 94.24% and PC2 5.64%, and PC1's loadings split by sign — `maths -0.505`,
`physics -0.496` against `english 0.492`, `history 0.507`. A component whose loadings oppose
each other like this is a **contrast**: it measures the science-versus-humanities tilt of each
student rather than overall ability. The percentages are always computed against the full set
of components, so trimming the report to 2 does not inflate them.

### Covariance vs correlation

Leave **Standardize columns to unit variance** ticked when the columns are in different units —
otherwise the variable with the biggest raw numbers simply wins. Untick it to run PCA on the
covariance matrix, which is what you want when every column is already in the same unit and the
differences in magnitude are real signal. Compare `0.1,240 / 0.3,610 / 0.2,395 / 0.5,980 /
0.4,760` with and without the box: unstandardized, PC1's loadings are `0.00054` and `1` — the
second column's variance of ~85,420 buries the first one entirely.

### Reading the output formats

`text` is the formatted report and prints the first 20 score rows. `csv` emits just the scores as
`row,PC1,PC2,…` so you can paste them straight into a plotting tool. `json` returns the full
result — every score row, plus the column means and sample standard deviations used to center
and scale, which are exactly what you need to project new observations onto the same components
later.

### Limits and edge cases

- **Shape.** At least 2 rows and 2 columns; up to 20,000 observations and 100 variables. Every
  row must have the same number of columns — a ragged row is an error, not a silent truncation.
- **Separators.** Commas, tabs, semicolons and runs of spaces all work, so a paste from a
  spreadsheet, a CSV or a fixed-width dump is fine. Blank lines are skipped.
- **Header detection.** The first row is treated as column names only if its tokens are *not* all
  numeric. Fill in **Column names** to override it, or to name columns in a headerless file.
- **Constant columns.** A column with zero variance cannot be standardized (it would divide by
  zero), so standardized runs reject it by name — drop the column or switch to covariance PCA,
  which handles it fine.
- **More variables than observations.** Allowed, but at most `n − 1` components carry real
  variance; the rest come back at essentially zero and are noise, not structure.
- **Component signs are arbitrary.** Flipping every loading and every score in a component gives
  an equally valid answer, so a convention is fixed: each component is flipped, if needed, so its
  largest-magnitude loading is positive. Other packages may show the opposite sign — the analysis
  is identical.
- **Rounding.** All reported numbers are rounded to 6 decimal places; eigenvalues that should be
  exactly 0 can appear as tiny values in unstandardized runs.

## FAQ

<details>
<summary>Should I standardize my columns, or use the covariance matrix?</summary>

Standardize (the default) whenever the columns are measured in different units or on wildly
different scales — height in cm next to income in dollars, say. PCA maximises variance, so without
standardizing, the column with the largest raw numbers dominates the first component for a reason
that has nothing to do with structure. Use the covariance matrix (untick the box) when every
column is already in the same unit and you *want* the bigger-varying ones to count more — repeated
measurements of the same quantity, spectra, or prices in a single currency.

</details>

<details>
<summary>How many components should I keep?</summary>

There is no single right answer, so the report gives you the three usual criteria at once. The
**cumulative variance** rule keeps enough components to reach a target — the report states how
many you need for 90%, 95% and 99%. The **Kaiser criterion** keeps components with an eigenvalue
above 1, i.e. those explaining more than one variable's worth of variance; it only makes sense on
standardized data, so it is only printed there. The **scree plot** shows the bar lengths dropping
off, and you keep the components before the elbow. When the three disagree, prefer the one that
matches what you are doing next: two components if you are going to scatter-plot, the 95% count if
you are compressing before another model.

</details>

<details>
<summary>What is the difference between loadings and scores?</summary>

Loadings describe the **variables**: each component's loading vector says how much each original
column contributes to that axis, and that is what you read to name a component ("all positive and
similar" = a size or overall-level factor; "positive on some columns, negative on others" = a
contrast between two groups). Scores describe the **observations**: each row's coordinates in the
new component space. You plot scores (PC1 on the x-axis, PC2 on the y-axis) to see clusters and
outliers, and you consult loadings to explain what the axes of that plot mean.

</details>

<details>
<summary>Why are my loadings' signs the opposite of what R or scikit-learn shows?</summary>

Eigenvectors are only defined up to a sign: negate a whole component — every loading and every
score — and it still describes the same axis with the same eigenvalue. Different libraries make
different arbitrary choices. This tool fixes the sign so that each component's largest-magnitude
loading is positive, which keeps the output stable across runs and platforms. If you compare with
another package and every number in a component is negated, nothing is wrong; the two answers are
the same analysis.

</details>

<details>
<summary>Do the eigenvalues have to add up to something in particular?</summary>

On standardized data, yes: the correlation matrix has 1 on its diagonal, so the eigenvalues sum to
the number of variables. Three columns give a total variance of exactly 3, and an eigenvalue of
1 means that component carries the same variance as a single standardized variable — which is
where the Kaiser rule comes from. On unstandardized (covariance) data the total is instead the sum
of the columns' sample variances, so it is in the square of whatever unit your data is in.

</details>

<details>
<summary>Can I use PCA on categorical data or data with missing values?</summary>

Not directly. This tool needs a complete numeric matrix: every cell must parse as a finite number,
and a row with a missing value is an error rather than being quietly dropped. Decide how to handle
gaps before you paste — delete those rows, or impute them (column mean imputation is the usual
quick fix, though it shrinks the variance you are about to decompose). Categorical columns need to
be encoded as numbers first, and one-hot dummies interact badly with variance-based methods;
correspondence analysis or a factor-analysis-for-mixed-data method is the better fit there.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The analysis is compiled to WebAssembly and runs inside your browser tab, so the matrix you
paste never leaves your device. That also means it works with the network off, and that the page,
the command line and the chat tool all produce byte-identical numbers from the same input — the
algorithm is deterministic with no random initialization.

</details>
