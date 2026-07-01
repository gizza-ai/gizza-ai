## About this tool

**Count line frequency** tallies how many times each line (or value) appears in
your text and ranks them from **most to least frequent** — the browser equivalent
of `sort | uniq -c | sort -rn`.

Paste a list — log lines, survey answers, tags, URLs, words — and get back each
distinct value with its count, plus the number of distinct values and the total.

- **Case sensitive** (default on): turn off to group `Apple`/`apple` together (the
  first-seen casing is shown).
- **Trim whitespace** (default on): ignore leading/trailing spaces; blank lines are
  always skipped.

### Privacy

Everything runs **in your browser** via WebAssembly — your text is never uploaded
to a server. You can also run it from the [gizza CLI](/) or inside a gizza chat
(which return the counts as structured JSON).

### Common uses

- Find the most common error in a log dump.
- Tally votes / survey responses / tags.
- Spot duplicates and how often they repeat.

### FAQ

<details>
<summary>How are ties ordered when two values have the same count?</summary>

Ranking is by count, highest first; values with equal counts keep the order they were first seen in your input. So if `b` appears before `a` and both occur twice, `b` is listed first.

</details>

<details>
<summary>What does turning off "Case sensitive" actually show?</summary>

Lines are grouped by their lowercased form, so `Apple`, `apple`, and `APPLE` count as one value — and the result displays the first casing it encountered (`Apple` in that example).

</details>

<details>
<summary>Are blank lines counted?</summary>

No — blank lines are always skipped, even with **Trim whitespace** off. Turning trim off only makes `x` and `  x  ` count as different values; a line of pure whitespace still counts as blank when trim is on.

</details>

<details>
<summary>Can I get the counts as data instead of text?</summary>

Yes. The page shows a `count → value` table, but the same tool via the gizza CLI or chat returns structured JSON with each entry's `value` and `count`, plus `distinct` and `total` figures.

</details>
