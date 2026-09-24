# fuzzy-match competitor analysis — 2026-09-24

## Sources checked

| Source | Observed table-stakes capabilities | Fit for this block |
| --- | --- | --- |
| RapidFuzz / FuzzyWuzzy-style libraries | Multiple string metrics, similarity scores, ranked extraction, limits and score cutoffs. | In model: normalized edit-distance scoring, ranked results, score threshold, result limit, JSON/CSV automation output. Out of model: broad metric catalog and token-set/token-sort variants. |
| fzf / fuzzy finder patterns | Query characters may match in order, sparse initials are useful, contiguous spans and compact matches should rank higher. | In model: subsequence algorithm that rewards compact spans and contiguous runs. Out of model: interactive terminal UI, live navigation, filesystem integration. |
| Data matching / fuzzy VLOOKUP tools | Thresholds, candidate review, similarity scores, near-matches, CSV/JSON input and unmatched reporting. | In model: one-query candidate ranking, thresholds, scores and CSV/JSON output. Out of model: two-table joins, blocking, join columns, unmatched-row reports. |
| MessyMatch / Datablist-style web tools | Browser-side matching, typo/case/order tolerance, review-oriented scores and data-cleanup workflows. | In model: local browser execution, case-insensitive default, score explanations. Out of model: phonetic matching, accent normalization, word-order token matching, account/project workflows. |

## Decisions for this implementation

- Inputs: require `query` plus `candidates`; candidates accept multiline text or a single comma-separated line.
- Algorithms: expose fixed-choice `hybrid`, `levenshtein`, and `subsequence` to cover typo matching and fuzzy-finder lookup without importing a heavy dependency.
- Ranking: sort by score descending, then edit distance, then candidate text for deterministic ties.
- Controls: threshold slider/number, result limit slider/number, case-sensitive checkbox, include-reasons checkbox, and output format enum.
- Output: implement text for review, CSV for spreadsheets, and JSON for automation.

## Explicit out-of-model items

- Two-table fuzzy joins, blocking/indexing, unmatched-row reports and full reconciliation workflows.
- Phonetic algorithms, accent folding, locale-specific collation and word-order token-set ratios.
- Interactive terminal/file picker UI and hosted project sharing.

## Verification focus from the scan

Tests should cover typo/edit-distance ranking, subsequence/fuzzy-finder behavior, thresholds, non-default case-sensitive and include-reasons checkboxes, CSV/JSON output, the candidate-count cap, and a comma-separated secondary input form.
