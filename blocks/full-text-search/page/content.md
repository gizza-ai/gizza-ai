## About this tool

Full Text Search turns a pasted corpus into a one-shot local search engine. Split documents with `---` lines, blank lines, or form-feed characters, enter a query, and the tool ranks matching documents with BM25 or classic TF-IDF. Results include document numbers, scores, titles, matched terms, and keyword-in-context snippets with hits wrapped in `«…»`.

The query parser supports the search patterns people expect from document search: bare terms, `"quoted phrases"` that must appear adjacent, `-term` exclusions, AND/OR term matching, optional prefix search, English Porter stemming, and stop-word filtering. The first non-blank line of each document is treated as a title and can be boosted with `title_boost`.

This tool is stateless and local. It builds the index from the paste on each run; it does not store or incrementally update an index. For PDFs, DOCX files, or EPUBs, extract the text first with a document extraction tool, then paste the text here.

## Worked examples

Search three policy snippets with BM25:

```text
query: refund
corpus:
Refund policy
Refunds take five business days.
---
Shipping guide
Orders ship within two days.
---
Return labels
Print a return label before requesting a refund.
```

The title match and repeated term push the refund policy document above unrelated shipping content.

Use phrase search plus an exclusion:

```text
query: "refund policy" -shipping
```

Documents must contain the adjacent phrase `refund policy`, and any document containing `shipping` is removed before ranking.

Use prefix search for partial terms:

```text
query: moto oil
prefix: true
```

With prefix search enabled, `moto` can match words such as `motorcycle` while `oil` is scored normally.

## Limits and model fit

- English stemming uses a dependency-free Porter-style stemmer; it is not a multi-language Snowball pipeline.
- No typo tolerance is included. Use the separate fuzzy document search tool when edit-distance matching is the primary need.
- No persisted index is stored. For reusable static-site indexes, use an index-builder workflow instead.
- No semantic/vector search is performed; that would require an embedding model.
- Snippets highlight normalized term hits, not rich HTML fragments.

## FAQ

<details>
<summary>When should I use BM25 instead of TF-IDF?</summary>

BM25 is the recommended default for most document search because it saturates repeated terms and normalizes by document length. TF-IDF is useful when you want a simpler classic score for comparison or when repeated terms should keep contributing more directly.

</details>

<details>
<summary>How are documents separated?</summary>

Choose `dashes` for a line containing three or more hyphens, `blank-line` for one or more empty lines, or `form-feed` for page-separated text from extractors. The first non-blank line inside each separated chunk becomes that document's title.

</details>

<details>
<summary>Does stemming work for every language?</summary>

No. The built-in stemmer targets common English Porter cases, so `running`, `runs`, and `run` can meet. Non-English stemming and language-specific stop-word lists are deliberately out of scope for this pure single-shot tool.

</details>

<details>
<summary>Can this search PDFs or DOCX files directly?</summary>

No. This tool accepts text only. Extract text from binary documents first, then paste the result. Keeping extraction separate avoids duplicating PDF/DOCX parsers and keeps the ranked search model deterministic.

</details>
