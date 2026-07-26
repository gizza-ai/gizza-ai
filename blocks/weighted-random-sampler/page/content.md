## About this tool

**Weighted Random Sampler** draws a random subset of rows from a dataset where some
rows should appear more often than others. Each row carries a **weight** (a
non-negative number in a column or JSON key), and the sampler picks rows with
probability **proportional to that weight** — a row with weight 5 is five times as
likely to be drawn as a row with weight 1. It works on **CSV** text or a **JSON
array of objects**, and every draw is **reproducible**: the same seed and inputs
always produce the same sample.

Use it to build weighted lotteries and giveaways, to over- or under-sample classes
when preparing training data, to pick survey respondents proportional to a
population weight, or to simulate a weighted draw for testing.

### Worked example

Input CSV (weights in the `weight` column):

```
name,weight
Alice,5
Bob,1
Carol,3
Dan,1
```

Drawing **2 rows without replacement**, `seed=42`, gives:

```
name,weight
Alice,5
Carol,3
```

Alice and Carol carry the most weight, so with this seed they win the draw. Change
the seed for a different sample, or turn **replacement** on to let a heavy row be
drawn more than once (then the sample size can exceed the number of rows).

### With vs. without replacement

- **Without replacement** (default) — each row is drawn at most once, so the sample
  size can't exceed the number of rows that have a positive weight. Rows come back
  in their original order. Uses the Efraimidis–Spirakis weighted scheme.
- **With replacement** — each of the `n` draws is independent, so the same row can
  appear several times and `n` may be larger than the dataset. Rows come back in
  draw order.

### Limits & edge cases

- Weights must be **non-negative numbers**. A row with weight `0` is never drawn; a
  non-numeric weight is an error that names the offending row.
- If **all** weights are zero, there is nothing to sample and the tool errors.
- Without replacement, asking for more rows than have a positive weight is an error.
- CSV: the header row (when enabled) is preserved and used to resolve a named weight
  field; the chosen delimiter is used for both input and output. JSON: the input must
  be an array of objects and the output is a JSON array of the selected objects.
- Everything runs locally in your browser — the dataset never leaves your machine.

## FAQ

<details>
<summary>How are the weights turned into probabilities?</summary>

Each row's chance of being drawn is its weight divided by the sum of all weights.
So in `A=5, B=1, C=3, D=1` (total 10), A has a 50% chance per draw, C 30%, and B
and D 10% each. You don't need the weights to add up to 1 or 100 — any
non-negative numbers work, and they're normalized for you.

</details>

<details>
<summary>What's the difference between sampling with and without replacement?</summary>

**Without replacement** (the default) draws each row at most once, so you can't
ask for more rows than exist, and the result keeps the input order. **With
replacement** treats every draw independently, so the same row can be picked
multiple times and you can draw more rows than the dataset contains — useful for
bootstrap resampling or weighted dice-style draws.

</details>

<details>
<summary>Is the sample reproducible?</summary>

Yes. Sampling uses a small deterministic PRNG seeded by the **seed** field, so the
same dataset, size, and seed always produce the exact same rows. Change the seed to
get a different draw while keeping everything else fixed.

</details>

<details>
<summary>Can I use it with JSON instead of CSV?</summary>

Yes. Set **Format** to `json` and paste a JSON array of objects, then give the
**weight field** as the object key holding the weight (e.g. `w`). The result is a
JSON array containing the full selected objects. Weights may be JSON numbers or
numeric strings like `"2.5"`.

</details>

<details>
<summary>What happens to rows with a zero or missing weight?</summary>

A weight of `0` means the row is never selected, but it stays in your dataset and
counts toward nothing. A missing weight field or a non-numeric value is treated as
an error that tells you which row is at fault, so you can fix the data rather than
get a silently skewed sample.

</details>
