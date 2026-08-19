## About this tool

A document-term matrix turns a small text corpus into rows (documents) and columns (terms). Each cell is either a count of how many times a term appears in that document or a binary 0/1 flag that says whether the term is present. This is the classic bag-of-words representation used before clustering, similarity checks, feature inspection, and text-mining experiments.

Paste one document per line, or choose JSON input when your documents contain embedded newlines. The builder tokenizes words locally, lowercases by default, can include adjacent word n-grams, filters rare terms with `min_df`, and caps the vocabulary with `max_features`. Columns are sorted by descending document frequency, then alphabetically for stable copy-paste output.

### Worked example

With these three documents:

```text
the cat sat
the dog sat
the cat chased the dog
```

The default CSV output starts with terms that appear in the most documents, followed by rarer terms:

```csv
document,the,cat,dog,sat,chased,__total_terms
doc_1,1,1,0,1,0,3
doc_2,1,0,1,1,0,3
doc_3,2,1,1,0,1,5
```

Set `weighting` to `binary` when you only need presence/absence. Set `ngram_max` to `2` or `3` to include phrases such as `quick fox` alongside individual words.

### Input notes and limits

The lines format treats each nonblank line as one document. The JSON format requires a JSON array of strings, for example `["first document", "second document"]`. The tool accepts up to 10,000 documents and 5,000 output columns. N-grams are limited to lengths 1 through 3 so the matrix remains browser-friendly.

## FAQ

<details>
<summary>What is the difference between count and binary weighting?</summary>

`count` records term frequency within each document, so repeated words produce values above 1. `binary` records only presence, so every nonzero count becomes 1. Binary matrices are useful for set-style similarity and feature flags.

</details>

<details>
<summary>How are terms tokenized?</summary>

Runs of Unicode letters and digits are words. Apostrophes and hyphens stay inside a word when they are between word characters, so `don't` and `state-of-the-art` remain single terms. Other punctuation separates tokens.

</details>

<details>
<summary>What does min_df do?</summary>

`min_df` is the minimum number of documents a term must appear in to become a column. For example, `min_df = 2` removes words that appear in only one document, which is a quick way to reduce noise in a larger corpus.

</details>

<details>
<summary>Why is there no TF-IDF option?</summary>

This tool focuses on transparent document-term matrices: raw counts and binary presence. TF-IDF changes the scale and interpretation of every cell, so it belongs in a separate weighting-oriented text-vectorizer tool.

</details>
