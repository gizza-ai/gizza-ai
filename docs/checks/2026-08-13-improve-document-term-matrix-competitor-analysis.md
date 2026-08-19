# document-term-matrix — competitor analysis (2026-08-13)

Scan run for `blocks/document-term-matrix`, per `.claude/skills/create-next-tool/SKILL.md`. Competitor behavior is paraphrased only; no competitor copy, branding, or assets are reused.

## Viability check

- Duplicate scan: nearby blocks include `frequency-distribution`, `count-line-frequency`, `word-frequency`-style text counters, `naive-bayes-text-classifier`, and `rake-keywords`. Those count tokens or classify text, but do not export a document-by-term matrix with rows per document and configurable vocabulary filtering. Not a semantic duplicate.
- Model fit: tokenization, n-gram expansion, document-frequency sorting, and CSV/TSV/JSON rendering are deterministic pure-Rust operations with bounded memory. No ML model or network dependency is needed. In model.

## Competitors reviewed

| # | Tool | Shape | Relevant table-stakes |
|---|------|-------|-----------------------|
| 1 | scikit-learn CountVectorizer | Reference API for bag-of-words matrices | Raw count and binary output, lowercasing, word tokenization, n-gram range, minimum document frequency, max feature count, deterministic vocabulary. |
| 2 | R text-mining / quanteda document-feature matrix tools | Statistical text-mining workflow | Documents become rows, features become columns; common controls include case handling, n-grams, document-frequency trimming, and sparse-matrix-friendly exports. |
| 3 | Online bag-of-words / text vectorizer tools | Direct web competition | Paste multiple documents, choose word counts or binary vectors, export CSV-like tables. Most have simpler tokenization and fewer documented limits. |
| 4 | Spreadsheet pivot/count workflows | Practical alternative | Users want a rectangular table they can paste into a spreadsheet, often with a total column and stable column order. |
| 5 | NLP notebook snippets | Developer comparison point | Common examples show JSON/CSV output, feature caps, and n-gram ranges so the resulting matrix is not too wide. |

## Table-stakes → decision

| Table-stake | Fit | Where it landed |
|---|---|---|
| Multiple documents | In model | `documents`, accepting JSON array or one nonblank line per document |
| Auto-detect input shape | In model | `input_format = auto/json/lines` |
| Count and binary cells | In model | `weighting = count/binary` |
| Lowercase by default, optional case-sensitive mode | In model | `case_sensitive` boolean |
| Word tokenization that ignores punctuation | In model | Core tokenizer keeps Unicode alnum plus internal apostrophes/hyphens |
| N-gram range | In model | `ngram_min` / `ngram_max`, limited to 1-3 |
| Minimum document frequency | In model | `min_df` |
| Feature cap | In model | `max_features` 0-5000 |
| Spreadsheet export | In model | `output = csv/tsv` |
| Structured export | In model | `output = json` with terms, document frequencies, totals, and matrix |
| Row totals | In model | `include_totals` |
| Deterministic vocabulary order | In model | Sort by descending document frequency, then term ascending |
| Stated limits | In model | 10,000 docs, 5,000 terms, n-grams up to 3 |

## Considered, not built

- TF-IDF weighting: useful but changes the scale and needs a separate set of smoothing/normalization choices. This tool stays a transparent DTM builder.
- Sparse Matrix Market output: good for large corpora, but gizza page/CLI users usually need CSV/TSV/JSON first; sparse output is a separate developer-oriented export.
- Stop-word lists and stemming: language-specific and easy to get wrong without bundled dictionaries. Users can pre-clean text with other gizza tools.
- File upload of CSV/PDF/DOCX corpora: this pure page works on pasted text; existing extraction tools can prepare input upstream.
- Topic modeling, clustering, embeddings: out of scope for the pure deterministic model and better served by separate ML tools.
