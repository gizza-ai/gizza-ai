## About this tool

Compare one query vector against a whole list of candidates and get them back ranked nearest
first. Paste embeddings from a model, feature rows from a spreadsheet, or plain coordinates —
numbers may be separated by commas, spaces, tabs or semicolons, and JSON-style brackets are
accepted and ignored. Everything is scored in your browser; no vector leaves the page.

Most online similarity calculators compare exactly two vectors. This one does nearest-neighbour
ranking: give it a query plus one candidate per line, and it returns the top `top_k` matches with
every companion metric beside the ranking score, so you can see at a glance whether cosine and
Euclidean agree about the winner.

### Worked example

Query `3, 2, 1` against three labelled candidates:

```
apple: 1, 2, 3
banana: 3, 2, 1
cherry: -3, -2, -1
```

With `metric=cosine` and `show_all_metrics=true` the ranking is `banana`, `apple`, `cherry`.
`banana` is identical to the query, so its cosine similarity is `1.000000`, its dot product is
`14.000000` and every distance is `0.000000`. `apple` points in a different direction but has the
same magnitude: cosine `0.714286` (that is `10 / 14`), dot product `10.000000`, Euclidean distance
`2.828427`, Manhattan `4.000000`. `cherry` is the exact opposite of the query, so cosine bottoms
out at `-1.000000` while its Euclidean distance is the largest at `7.483315`.

Switch `metric` to `euclidean` and the sort direction flips automatically — distance metrics rank
lowest-first, similarity metrics rank highest-first. Set `output=csv` or `output=json` when the
ranking feeds another script rather than your eyes.

### Limits and edge cases

- Up to **2000 candidate vectors** of up to **8192 dimensions** per run, which covers the common
  embedding widths (384, 768, 1024, 1536, 3072).
- Every candidate must have exactly as many values as the query; a mismatch is reported with the
  offending line number and label rather than being silently padded.
- Blank lines and lines starting with `#` are skipped, so you can comment a vector list.
- A label is the text before the first `:` on a line. Unlabelled lines get `v1`, `v2`, … A numeric
  prefix such as `1:2` is treated as data, not a label.
- Cosine similarity and cosine distance are undefined for a zero-magnitude vector; the run fails
  with the specific vector named instead of returning a misleading `0`. Other metrics still work on
  zero vectors, and the companion cosine column shows `undefined` for them.
- `normalize=true` scales the query and every candidate to unit length first, which makes dot
  product identical to cosine similarity — and rejects zero vectors for the same reason.
- Ties keep their input order, so a stable ranking is reproducible run to run.
- `decimals` (0–12) affects display only; scoring is always done in double precision.

## FAQ

<details>
<summary>Which metric should I use for text embeddings?</summary>

Cosine similarity is the usual default for text embeddings because it measures direction and
ignores magnitude, so a long document and a short one about the same topic still score as similar.
Use Euclidean distance when the magnitudes are meaningful, such as physical measurements or image
features. Many embedding models already emit unit-length vectors, in which case cosine similarity
and dot product rank candidates identically.

</details>

<details>
<summary>What is the difference between cosine similarity and cosine distance?</summary>

They carry the same information in opposite directions. Cosine similarity runs from `1` (identical
direction) through `0` (orthogonal) to `-1` (opposite), and higher is better. Cosine distance is
`1 - similarity`, so it runs from `0` to `2` and lower is better. Pick `cosine_distance` when you
are matching the convention of a vector database that reports distances.

</details>

<details>
<summary>How does Hamming distance work on vectors that are not binary?</summary>

It counts how many coordinates differ. Because exact float equality is fragile, set
`hamming_tolerance` to the largest difference you want to treat as "the same": with a tolerance of
`0.1`, the vector `1.05, 2, 3` scores a Hamming distance of `0` against `1, 2, 3`. The default
tolerance of `0` means exact numeric equality, which is what you want for genuinely binary or
integer vectors.

</details>

<details>
<summary>What input formats does the vector list accept?</summary>

Anything that reduces to numbers separated by punctuation. `[1, 2, 3]`, `1 2 3`, `1;2;3` and
`(1, 2, 3)` all parse to the same vector, and brackets, braces, quotes, commas and semicolons are
treated as separators. That means you can paste a JSON array straight from an API response, or a
row copied out of a spreadsheet, without reformatting it first. One vector per line.

</details>

<details>
<summary>Can it search millions of vectors like a vector database?</summary>

No — this is an exact brute-force scan over a list you paste, capped at 2000 vectors. That is the
right shape for checking a retrieval result, debugging an embedding, or ranking a small candidate
set by hand. Corpora large enough to need approximate indexes such as HNSW or IVF also need a real
vector database; the scores here are exact, so they are useful for verifying what one returns.

</details>

<details>
<summary>Does it turn text into vectors for me?</summary>

No. It expects numbers that you already have — from an embedding model, a feature pipeline, or a
measurement. Embedding text requires a learned model, which this page deliberately does not load so
that it stays fast and fully offline.

</details>
