## About this tool

Random projection is the cheapest useful dimensionality reduction there is. Instead of
studying your data to find good axes — the way PCA computes eigenvectors — it multiplies the
table by a matrix of **random** numbers and simply relies on a theorem: the
Johnson–Lindenstrauss lemma says that a random linear map into enough dimensions keeps every
pairwise distance almost unchanged, and that "enough" depends only on how many points you have,
never on how wide the table was.

That makes it the tool of choice when a table is far too wide to decompose — thousands of
term-frequency columns, embedding dimensions, sensor channels — and when what you need
preserved is *distances* (for nearest-neighbour search, clustering, duplicate detection) rather
than variance.

Paste your matrix and this reducer does the whole thing in the browser:

- draws a `k × columns` random matrix from one of four families — **Gaussian**, **sparse**,
  **Achlioptas** or **Rademacher** — each scaled so that distances are preserved *in
  expectation*;
- projects every row onto it, giving `k` new coordinates named `RP1`, `RP2`, …;
- **measures what actually happened**: it recomputes pairwise row distances before and after and
  reports the mean, median and maximum distortion, the mean ratio, and how many sampled pairs
  landed inside your `±eps`;
- tabulates the **Johnson–Lindenstrauss minimum dimension** for several tolerances, so the
  trade-off between `eps` and `k` is visible in the same result.

Nothing is uploaded. The random numbers come from a fixed, portable integer stream rather than a
platform generator, so a given **seed** reproduces exactly the same projection on this page, on
the command line and in chat.

### Worked example — eight channels down to three

Paste six observations of eight channels (the first row is read as a header of column names) and
set **Target dimensions** to `3`:

```text
ch1,ch2,ch3,ch4,ch5,ch6,ch7,ch8
1,2,3,4,5,6,7,8
8,7,6,5,4,3,2,1
2,4,6,8,10,12,14,16
0,1,0,1,0,1,0,1
5,5,5,5,5,5,5,5
9,1,8,2,7,3,6,4
```

The report opens with the settings and the honest verdict on quality:

```text
Random projection: 6 rows × 8 columns → 3 dimensions (37.5000% of the input width)
  method        gaussian — dense matrix, entries drawn from N(0, 1/k)
  target dims   3 (set explicitly)
  density       100.0000% non-zero entries
  seed          42
  input columns ch1, ch2, ch3, ch4, ch5, ch6, ch7, ch8

Distance preservation (15 of 15 row pairs measured):
  mean distortion     22.4111%
  median distortion   21.1289%
  max distortion      42.5474%
  mean ratio          1.051128  (projected ÷ original distance)
  within ±10.0000%  3 of 15 pairs (20.0000%)
```

and ends with the projected coordinates:

```text
Projected data (first 6 of 6 rows):
  row           RP1           RP2           RP3
    1    -17.064939     -0.160032     -8.288287
    2    -13.495953      4.349463     -1.436021
    3    -34.129877     -0.320064    -16.576574
    4     -1.341949     -0.556681     -0.113729
    5    -16.978273      2.327462     -5.402393
    6    -20.080918      7.953206     -6.799878
```

A 22% mean distortion is *bad*, and the tool says so rather than hiding it — three dimensions is
simply not enough for a guarantee. The Johnson–Lindenstrauss block in the same report explains
why: even six points want `k ≥ 1535` for a ±10% embedding. Random projection is a large-`k`,
wide-data method, and a six-row toy is the case where it works worst. It is still the right
example to start from, because every number above is exactly reproducible — same seed, same
result, on every surface.

### What it looks like when there is room to work

On a table of 60 rows and 200 columns of random values, raising `k` tightens the embedding in
exactly the way the lemma predicts:

| Target `k` | Mean distortion | Median | Pairs within ±10% |
|---|---|---|---|
| 16 | 15.67% | 13.24% | 672 of 1770 (37.97%) |
| 64 | 6.50% | 5.43% | 1386 of 1770 (78.31%) |
| 128 | 4.82% | 3.96% | 1589 of 1770 (89.77%) |

The `sparse` family at the same `k = 64` lands at 7.51% mean distortion while touching roughly
one entry in fourteen of the matrix — slightly noisier, much less work. That trade is the whole
reason the sparse variants exist.

### Choosing the target dimension

- **A number** (`3`, `64`, `128`) sets `k` directly. This is the usual choice: pick the width your
  downstream index or model wants, then read the distortion figures to see what it cost.
- **A percentage** (`25%`) keeps that share of the input width, which is handy when the same
  setting is applied to tables of different widths.
- **`auto`** derives `k` from the Johnson–Lindenstrauss bound at your `eps`, then clamps it to the
  number of columns you actually have. On a small paste that clamp bites immediately — the bound
  wants more dimensions than the data has — and the report says so.

### Reading the output formats

`text` is the report above and prints the first 20 projected rows. `csv` emits every projected row
as `row,RP1,RP2,…` so it can go straight into a plot or another tool. `json` returns the whole
result: every row, the diagnostics, and the projection matrix. `matrix` returns just the
`k × columns` projection matrix as CSV — that is what you keep if you need to project *new* rows
onto the same axes later, since the matrix plus the seed is the entire model.

### Limits and edge cases

- **Shape.** At least 2 rows and 2 columns; up to 2,000 rows, 1,000 columns and 200,000 cells.
  Target dimensions run from 1 to 256.
- **Separators.** Commas, tabs, semicolons, pipes and runs of spaces all work, so a spreadsheet
  paste, a CSV or a fixed-width dump is fine. Blank lines are skipped.
- **Header detection.** The first row is read as column names only when its tokens are *not* all
  numeric. Column names are echoed in the report and used as the header of the `matrix` output.
- **Every cell must be a finite number.** A missing, blank or non-numeric cell is a located error
  (`row 4, column 2: 'n/a' is not a finite number`), never a silent zero, and a ragged row is
  rejected with both row lengths.
- **Diagnostics are sampled.** All pairs are measured when there are 200 rows or fewer; above that
  20,000 row pairs are sampled deterministically from the seed. Pairs of identical rows have zero
  original distance, so no ratio exists for them — they are counted as skipped.
- **Density** applies only to `sparse` and `achlioptas`. Setting it on a dense family
  (`gaussian`, `rademacher`) is an error rather than a silently ignored field.
- **Not variance-optimal.** For the same `k`, PCA finds better axes; random projection wins on
  speed and on tables too wide to decompose, and its guarantee is about distances, not variance.
- **Rounding.** All reported numbers are rounded to 6 decimal places.

## FAQ

<details>
<summary>How many dimensions do I actually need?</summary>

The Johnson–Lindenstrauss bound in the report is the theoretical answer:
`k = 4·ln(rows) / (eps²/2 − eps³/3)`, which for a ±10% embedding is about 1,535 dimensions for 6
points and 11,841 for a million. Those numbers are famously pessimistic — they are a worst-case
guarantee over every possible dataset, and real data almost always survives far fewer dimensions.
That is exactly why this tool measures the distortion instead of only quoting the bound: set the
`k` your downstream task can afford, look at the mean distortion and the "within ±eps" count, and
raise `k` until those numbers are good enough. Note that the bound depends only on the number of
rows, not on how wide the table is — projecting 500 documents is the same problem whether they
have 5,000 or 50,000 features.

</details>

<details>
<summary>Which random matrix should I choose?</summary>

Start with `gaussian`: it is the classic construction, entries drawn from `N(0, 1/k)`, and it
gives the lowest distortion of the four in practice. Switch to `sparse` when the table is wide —
its default density of `1/√columns` means most entries are zero, so the projection touches a small
fraction of the data for a small penalty in distortion. `achlioptas` is the same family pinned at
density `1/3`, the classic "database-friendly" result, and `rademacher` is a dense matrix of ±
signs, which is the fastest per entry because it needs no multiplication at all. All four are
scaled so that expected distances match; they differ in how much they vary around that
expectation.

</details>

<details>
<summary>Why do I get different numbers than scikit-learn with the same seed?</summary>

Because a seed only means something inside one generator. scikit-learn draws from NumPy's Mersenne
Twister; this tool uses a fixed xoshiro256++ stream so that the page, the command line and the chat
tool agree byte for byte. The *distributions* are the same, so the statistics match — the same
family, density and `k` give the same distortion behaviour — but the individual matrix entries are
a different draw. If you need one specific projection reproduced elsewhere, export it with
`matrix` output and apply that matrix directly rather than re-seeding.

</details>

<details>
<summary>Can I project new rows onto the same axes later?</summary>

Yes. The projection is a plain linear map, so it is fully described by the `k × columns` matrix.
Either re-run with the same seed, method, density, `k` and column count — the matrix depends on
nothing else, so it will be identical — or take the `matrix` output once and multiply new rows by
it yourself. What you must not do is re-run with a different `k` or a different column count and
expect the coordinates to be comparable: those are different random matrices and the two sets of
coordinates live in unrelated spaces.

</details>

<details>
<summary>How is this different from PCA?</summary>

PCA looks at your data and computes the axes that capture the most variance; random projection
ignores the data entirely and picks axes at random. That sounds strictly worse, and for a fixed
`k` it usually is — but PCA needs an eigen-decomposition of a `columns × columns` matrix, which is
cubic in the width and simply not viable at tens of thousands of features. Random projection is a
single matrix multiply, needs no fitting pass, and its accuracy guarantee is about pairwise
distances rather than variance, which is what nearest-neighbour search and clustering actually
depend on. Use PCA when the table is narrow enough to decompose and you want interpretable
components; use random projection when it is not, or when you need a projection in one pass.

</details>

<details>
<summary>What does the "mean ratio" tell me that the distortion figures do not?</summary>

Distortion is `|projected ÷ original − 1|`, so it throws away the direction of the error. The mean
ratio keeps it: a value above 1 means distances came out systematically too long, below 1 too
short. For a correctly scaled projection it should sit near 1 even when individual pairs are far
off, because the scaling makes distance preservation exact in expectation. A mean ratio that
drifts well away from 1 on a large sample is a sign that `k` is small enough for the estimator
itself to be biased upward — the same thing the rising mean distortion is telling you.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The whole computation is compiled to WebAssembly and runs inside your browser tab, so the
matrix you paste never leaves your device, and the tool works with the network switched off. The
same engine backs the command line and the chat tool, and because the random stream is a fixed
integer sequence rather than a platform RNG, all three produce identical numbers from identical
inputs.

</details>
