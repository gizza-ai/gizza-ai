## About this tool

Use fuzzy match when you have one search string and a list of messy candidates, and you want the most plausible matches with a visible score. It is useful for quick data cleanup, command or filename lookup, deduplicating labels before a heavier join, and reviewing near-matches without sending data to a hosted service.

The default `hybrid` mode takes the best signal from exact/contains matches, normalized Levenshtein edit distance, and subsequence-style fuzzy-finder matching. You can switch to `levenshtein` when typos are the main issue, or `subsequence` when users type initials or sparse characters from a filename.

### Worked example

```bash
gizza tool fuzzy-match query=apple candidates=$'apple pie\napplet\nbanana\napply\npineapple' limit=3
```

Example output:

```text
rank  score   candidate  reason
   1   96.7  applet  — subsequence span 5, best contiguous run 5
   2   92.0  apple pie  — contains the query
   3   92.0  pineapple  — contains the query
```

### Limits and edge cases

- Up to 1,000 candidates, 256 characters per candidate, and a 256-character query.
- Scores are 0 to 100. A threshold of 80-90 is strict; the default 0 keeps all candidates before the result limit is applied.
- Blank lines and `#` comments are ignored, and simple list markers such as `-`, `*`, and `1.` are stripped.
- This ranks one list against one query. It is not a two-table fuzzy join, phonetic matcher, accent normalizer, language model, or duplicate-clustering engine.

## FAQ

<details>
<summary>Which algorithm should I use?</summary>

Use `hybrid` for most cases because it combines exact/contains matches, edit distance and fuzzy-finder subsequences. Use `levenshtein` for typo-heavy data such as names. Use `subsequence` for file or command lookup where `fb` should match `fast_build.sh`.

</details>

<details>
<summary>What threshold is best?</summary>

There is no universal threshold. Start with 0 to inspect the ranking, then raise the threshold. Around 70 keeps broad near-matches; 80-90 is better when false positives are costly.

</details>

<details>
<summary>Why do two candidates have the same score?</summary>

Some signals intentionally collapse to the same score, for example strings that contain the query. Ties are sorted by edit distance and then alphabetically so the output is deterministic.

</details>

<details>
<summary>Can this reconcile two CSV files?</summary>

No. This tool ranks a candidate list for a single query. For two-table reconciliation you need blocking, join keys and unmatched-row reporting; use a dedicated fuzzy CSV join workflow instead.

</details>
