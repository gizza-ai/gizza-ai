## Cluster similar values

Real-world columns are full of the "same" value written many ways — `New York`,
`new york`, `New  York`, `Nwe York`. This tool **fuzzy-clusters** the values in
one column so those near-duplicates land in the same group, then proposes a
**canonical form** (the most frequent original) for each cluster. Feed it a CSV
or a plain list, one value per line. Everything runs in your browser; nothing is
uploaded.

### How it works

Each value is compared with a normalized **Levenshtein** edit-distance ratio
scored 0–100. Two values join the same cluster when their similarity is at or
above the **threshold**. Before comparing, values are optionally folded to ignore
letter **case** and extra **spacing**, so those differences don't count against a
match. The most frequent original in each cluster becomes its canonical
suggestion.

### Worked example

Input (one value per line), threshold `70`:

```
New York
new york
New  York
Nwe York
Boston
```

The four New-York variants (a typo, a case change, a double space) cluster
together with canonical **New York**, while **Boston** stays on its own. The
Mapping table then gives you every `original → canonical` pair to apply back to
your data.

### Options

- **Column** — for a CSV, the header name (with *First row is a header* on) or a
  1-based index. Blank uses the only column, which is what a newline list is.
- **Delimiter** — comma, tab, semicolon, pipe, or any single character (ignored
  for a plain list with no separators).
- **Threshold (0–100)** — higher is stricter: `100` merges only values that are
  identical after normalization; lower merges looser matches. `85` is a good
  start; drop toward `70` to catch more typos, raise it if unrelated values merge.
- **Ignore case / Ignore extra spacing** — fold those differences before
  comparing (on by default).
- **Output** — *markdown* (clusters plus a mapping table), *csv* (a flat
  `cluster,original,canonical,count` mapping you can join back), or *json*
  (structured clusters with per-value counts).

### FAQ

<details>
<summary>Which value becomes the canonical suggestion?</summary>

The most frequent original value in the cluster (ties break toward the one seen
first). It's kept exactly as it appears in your data — casing and spacing intact —
so you can trust it as the "correct" spelling to standardize on.

</details>

<details>
<summary>How do I turn a threshold into "how many typos are allowed"?</summary>

The score is `(1 − edits ÷ length) × 100`, where length is the longer of the two
values. So for an 8-character value, a threshold of `85` tolerates about one edit
(insert, delete, or substitute a character); `75` tolerates about two. Shorter
values need a lower threshold to merge, since a single edit is a bigger fraction.

</details>

<details>
<summary>Does it change my data or just report clusters?</summary>

It only reports. You get the clusters and a mapping table — nothing in your input
is rewritten. Use the CSV mapping (`original → canonical`) to apply the merges in
your spreadsheet or script.

</details>

<details>
<summary>How is this different from removing duplicate rows?</summary>

Plain de-duplication only removes rows that match *exactly*. This finds values
that are *nearly* the same — typos, casing, and spacing differences — which exact
matching misses, and suggests a single canonical spelling to standardize them to.

</details>

<details>
<summary>What are the limits?</summary>

It clusters one column at a time and compares values by character edits, so it's
ideal for names, cities, companies, and tags — not for matching semantically
equal but differently-worded phrases. Blank cells are skipped. Greedy clustering
compares each value to its cluster's most-frequent seed, so extremely large,
noisy columns may group slightly differently than an exhaustive pairwise pass.

</details>

<details>
<summary>Is my data uploaded?</summary>

No — it's processed locally with WebAssembly and never leaves your browser.

</details>
