## About this tool

Fuzzy Dedupe removes **near-duplicate** rows and lines that a plain, exact de-duplication
leaves behind — the same value re-entered with a typo, different casing, or extra spacing
(`New York`, `new york`, `New  York`, `Nwe York`). It compares rows with a normalized
Levenshtein edit-distance ratio and merges any that are at least as similar as your
**threshold** (0–100), keeping one representative per group.

Paste a CSV or a plain list with one value per line. For a CSV you can match on specific
**columns** (by header name or 1-based index) while still keeping the full row, so records are
de-duplicated on the field that matters (a company name, an email) even when other columns
differ. Choose which row survives each group — the **first** seen, the **longest** (most
information), or the **most_frequent** exact spelling — and get back the cleaned data, just the
rows that were removed, or a JSON report of every group and the run's stats.

Everything runs locally in your browser through WebAssembly: your data is never uploaded, and
there are no size limits beyond your own machine. It's the de-duplication counterpart to
clustering — where clustering proposes a canonical mapping, this hands you the cleaned dataset
directly.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. One <details> per
     question; write real Q&A, not these TODOs. -->

<details>
<summary>How is this different from removing exact duplicate rows?</summary>

Exact de-duplication only merges rows that are byte-for-byte identical (optionally after
folding case and spacing). Fuzzy Dedupe also merges rows that are merely **similar** — typos
like `Nwe York`, or a name entered as `Acme Inc` and `Acme  Inc`. Two rows merge when their
similarity is at least the **threshold** you set (0–100). Set the threshold to 100 and it
behaves like exact de-dup on the normalized values.

</details>

<details>
<summary>What does the threshold mean, and what value should I use?</summary>

The threshold is a similarity percentage from a normalized edit-distance ratio: 100 means
identical, and lower values allow more difference. Start around **85** for tight matching of
typos and near-misses, and lower it (say **70**) if you also want to catch bigger variations.
Higher = stricter (fewer merges, safer); lower = looser (more merges, riskier). Use the
**removed** or **json** output first to review what would be dropped before trusting a run.

</details>

<details>
<summary>Which row is kept out of each near-duplicate group?</summary>

That's the **keep** setting. `first` keeps the earliest row in input order (predictable and
stable). `longest` keeps the row with the most characters, which usually preserves the most
complete record. `most_frequent` keeps the exact value that appears most often in the group —
handy when the correct spelling is also the most common one. The removed rows are still
available via the **removed** or **json** output.

</details>
