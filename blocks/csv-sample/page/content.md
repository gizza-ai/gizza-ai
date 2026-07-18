## CSV sample

Pull a smaller, representative subset of rows out of a big CSV — for a quick look,
a test fixture, a survey draw, or a spot check. Pick a method, a size, and (for
random draws) a seed. Everything runs in your browser; nothing is uploaded.

### Methods

- **random** — a uniform random draw of rows. Reproducible: the same *seed* always
  produces the same sample; change the seed for a different draw.
- **systematic** — every *k*-th row after a seeded random start, where
  *k ≈ rows ÷ size*. Spreads the sample evenly across the file.
- **top** — the first *N* data rows (like `head`).
- **bottom** — the last *N* data rows (like `tail`).
- **stratified** — split the rows into groups by the *stratify column*, then sample
  each group **in proportion to its size** so every group stays represented.

### Size

Set **sample size** to a row count (e.g. `10`), or set **percent** to a share of the
rows (e.g. `25` for 25%). Percent wins whenever it's above 0. Either way the result is
capped at the number of rows available, so asking for more rows than exist returns them
all.

### Worked example

Input CSV (4 data rows):

```
name,group
Alice,east
Bob,west
Carol,east
Dan,west
```

**Stratified**, size `2`, stratify column `group`, seed `1` → one row from *east* and
one from *west* (proportional to the two equal groups), e.g.:

```
name,group
Alice,east
Bob,west
```

**Top**, size `3` → the header plus the first three rows (`Alice`, `Bob`, `Carol`).

### Limits & edge cases

- The header row (when *first row is a header* is on) is always kept and never counted
  toward the sample size.
- Selected rows are returned in their **original file order** (top/bottom preserve
  order too), so a random sample of a sorted file is still sorted.
- Sampling is **without replacement** — no row appears twice; one sample per run.
- Delimiter is one of comma, tab, semicolon, or pipe; the output uses the same one.
- Rows with a differing number of fields are tolerated.

### FAQ

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The sampling runs locally in your browser with WebAssembly — the CSV never leaves
your machine.

</details>

<details>
<summary>Why does the same input always give the same "random" sample?</summary>

Random and stratified draws use a seeded pseudo-random generator, so results are
**reproducible** — handy for sharing or re-running an analysis. To get a different draw,
change the **seed** value (any whole number).

</details>

<details>
<summary>How does stratified sampling decide how many rows per group?</summary>

It allocates each group a share proportional to its size (largest-remainder rounding),
so the sample keeps the same group balance as the full file. For example, from 90 rows
in group A and 10 in group B, a size-10 stratified sample takes about 9 from A and 1
from B.

</details>

<details>
<summary>Can I sample without a header row?</summary>

Yes — turn *first row is a header* off. Then refer to the stratify column by its
**1-based index** (e.g. `2` for the second column) instead of a name, and every row is
treated as data.

</details>

<details>
<summary>What if I ask for more rows than the file has?</summary>

You get all of them. The size (or percent) is capped at the number of available data
rows, so nothing is duplicated or invented.

</details>
