## About this tool

Draw a fixed-size random sample from a large pasted list, log, CSV export, or other line-oriented dataset. Reservoir sampling is designed for streams: it keeps only `k` records in memory and decides, as each record arrives, whether that record belongs in the sample.

The draw is uniform without replacement, so every record has the same chance of appearing and no record is returned twice. The seeded PRNG makes results reproducible: the same data and seed produce the same sample, while a different seed gives a different draw.

### Worked example

Paste a log-like list:

```
GET /home 200
GET /pricing 200
POST /signup 500
GET /docs 200
GET /home 304
POST /login 401
GET /blog 200
GET /home 200
DELETE /account 204
GET /pricing 500
```

Set `k=4`, leave `algorithm=l`, and keep `order=input` for a sample shown in original line order. Turn on `stats` to append the number of records scanned, the sample size, the inclusion probability, the algorithm, and the seed used for the draw.

### Limits and edge cases

- `k` must be between **1 and 1,000,000**. If the dataset has fewer records than `k`, every record is returned.
- Input is split on lines. One line equals one record; commas inside a line are not parsed as columns.
- `skip_empty=true` ignores blank and whitespace-only lines before sampling.
- `header=true` treats the first line as a header: it is never sampled, and it is echoed above `lines` or `numbered` output.
- `algorithm=l` is the skip-based algorithm; `algorithm=r` is the classic one-random-draw-per-record algorithm. Both are uniform, but they produce different seeded samples.
- The RNG is deterministic for reproducibility, not cryptographic security. Do not use it for lotteries with legal or security requirements.

## FAQ

<details>
<summary>What is reservoir sampling for?</summary>

It is for choosing a fixed-size uniform sample when you cannot or do not want to hold the whole dataset in memory. The algorithm reads records one at a time and keeps only the reservoir of selected records.

</details>

<details>
<summary>Is every line equally likely to be selected?</summary>

Yes. For a dataset with `N` eligible records and sample size `k`, each record has probability `k / N` of appearing in the final sample. Turn on `stats` to see that probability for your run.

</details>

<details>
<summary>Why are there two algorithms?</summary>

Algorithm R is the classic textbook version and is easiest to audit: after the first `k` records, every new record gets one random draw. Algorithm L skips ahead to the next replacement and uses far fewer random draws on large streams while preserving uniformity.

</details>

<details>
<summary>How do I sample a CSV while keeping the header?</summary>

Paste the CSV rows as text, set `header=true`, and choose `format=lines` or `format=numbered`. The first row is excluded from the random draw and copied to the top of the output.

</details>

<details>
<summary>Can I reproduce the same sample later?</summary>

Yes. Keep the same input text, options, and `seed`. Change only the seed when you want a fresh draw from the same dataset.

</details>
