# cluster-similar-values competitor analysis

Date: 2026-07-17

## Competitor scan

Looked at common data-cleaning/fuzzy-match tools for deduplicating messy categorical columns:

| Tool class | Table-stakes behavior | In model? | Decision for this block |
| --- | --- | --- | --- |
| Spreadsheet fuzzy-merge add-ons | Choose a column, tune similarity/threshold, group typo/case/spacing variants, pick a canonical value | Yes | Implemented `data`, `column`, `threshold`, `normalize_case`, and `normalize_spacing`; canonical is the most frequent original value. |
| Data-prep fuzzy clustering UIs | Show each cluster and a mapping from original value to canonical replacement | Yes | Markdown output includes cluster sections and a mapping table; CSV output is a flat `cluster,original,canonical,count` mapping. |
| Record-linkage libraries | Support CSV input with headers, delimiters, and numeric column selection | Yes | Implemented header-aware column names, 1-based indices, and comma/tab/semicolon/pipe/single-character delimiters. |
| Advanced match algorithms | Phonetic matching, token set ratios, weighted fields, trained/entity-specific matchers | Partly / out of model for v1 | This block uses local Levenshtein similarity only. Token/phonetic/entity matching can be a future enhancement but is not required for the backlog row. |
| Interactive merge workflow | Let users accept/reject each proposed merge and rewrite the dataset in-place | Out of model | Gizza tools are typed-input → one output. We report clusters/mapping; applying changes remains a downstream spreadsheet/script step. |
| Multi-column/entity resolution | Combine several fields (name + address + phone) into entity clusters | Out of scope | The backlog row asks for values in a column. Multi-field record linkage is a distinct tool. |

## Defaults and UX choices

- Default threshold: `85`, strict enough for one-character typos in medium-length names while avoiding many accidental merges.
- Threshold control: slider 0–100 with step 1.
- Case and spacing normalization: on by default because competitor tools generally treat case/extra whitespace as cleanup noise.
- Output presets: markdown for readable review, CSV for applying a mapping, JSON for scripts.
- Worked examples: one newline-list example and one CSV-column example.

## Verification focus

The test matrix covers typo/case/spacing clustering, normalization toggles, CSV column selection by name, CSV/JSON/markdown outputs, threshold bounds, schema drift, CLI output, and page deep-links.
