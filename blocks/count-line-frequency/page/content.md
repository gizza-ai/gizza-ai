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
