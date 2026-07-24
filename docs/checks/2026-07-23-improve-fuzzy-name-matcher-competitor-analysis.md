# fuzzy-name-matcher — competitor analysis (2026-07-23)

Tool function: match and deduplicate person or organization names with fuzzy string metrics, producing reviewable groups or candidate pairs.

## Competitors scanned

1. Spreadsheet/data-cleaning fuzzy match add-ons: commonly offer approximate matching between columns or inside one list, threshold controls, and reviewable matched pairs.
2. Record-linkage/entity-resolution libraries: emphasize Jaro-Winkler for names, edit distance, phonetic encodings, normalization, and explainable scores.
3. CRM/list-cleaning dedupe tools: group similar contacts or organizations, pick a canonical record, and provide exportable mapping/audit outputs.
4. Phonetic-name match recipes: Soundex/Metaphone-style comparisons catch names that sound alike despite different spelling.

## Table stakes → decisions

| Capability | In-model? | Where it lands |
| --- | --- | --- |
| Paste a list of names | yes | `names` multiline input |
| Tune similarity threshold | yes | `threshold` slider, 0-100 default 85 |
| Jaro-Winkler for short names | yes | `algorithm = jaro_winkler` default |
| Levenshtein/edit-distance option | yes | `algorithm = levenshtein` |
| Phonetic matching | yes | `algorithm = soundex` |
| Case normalization | yes | `normalize_case` checkbox |
| Ignore honorifics and suffixes | yes | `ignore_titles` checkbox |
| Reviewable groups/canonical mapping | yes | `mode = groups`, table/csv/json outputs |
| Reviewable scored pairs | yes | `mode = pairs` |
| CSV/JSON export | yes | `output` enum |
| Cross-column joins with multi-field records | no | out of model; this tool is single-list name matching, first CSV field only |
| ML/knowledge-graph entity resolution | no | out of model; deterministic local matching only |

## UX controls

Use a multiline textarea for names, select controls for algorithm/mode/output, a threshold slider with loose/strict labels, and checkboxes for case/title normalization. Preset chips cover people dedupe, organization aliases, phonetic names, and CSV mapping.

## Distinctness vs existing blocks

`fuzzy-dedupe` and `cluster-similar-values` are generic value cleaners; this tool is name-specific because it combines Jaro-Winkler, Levenshtein, Soundex, honorific/suffix stripping, duplicate counting, canonical group output, and scored pair review. `fuzzy-doc-search` searches documents rather than deduplicating a list.
