## About this tool

`few-shot-text-classifier` labels text from a small set of examples that you provide at run time. Paste one labeled example per line, such as `billing,invoice charge refund`, then paste the document or list of lines you want classified. The tool builds a deterministic local similarity model from that support set and returns the predicted label, confidence, per-label scores, shared terms, and nearest example.

This is useful for quick triage and lightweight labeling: support tickets, feedback themes, short content moderation queues, routing emails into buckets, or testing whether a proposed label taxonomy is separable before you train a larger model. It does not call an API, download embeddings, or learn a persistent model. Every run uses only the examples in the **Labeled examples** box.

### Worked example

With these examples:

```text
billing,invoice charge refund
billing,subscription payment failed
support,password reset login issue
support,account locked cannot sign in
sales,pricing quote enterprise plan
sales,demo request for buying team
```

and this text:

```text
I cannot sign in after resetting my password.
```

the default centroid + cosine + TF-IDF settings predict `support`, show the support score above the other labels, and list terms such as `password` and `sign` as the explanation. Switch **Classify text as** to **One document per non-blank line** to batch-label a list and choose **CSV** when you want one result row per input line.

### Options and limits

- **Labeled examples** must contain at least two distinct labels. Each non-empty line is split into `label` and `text` using tab, comma, pipe, or colon; **Auto** chooses the separator that appears on the most lines. Lines beginning with `#` are ignored. The examples input is capped at 1 MiB, 5,000 examples, and 200 labels.
- **Decision method** controls how label scores are built. **Label centroids** averages each label's examples and is the steadier default. **k nearest examples** lets the closest `k` examples vote by similarity. **Best single example** lets one very close example win for its label.
- **Similarity metric** can be cosine, dot product, inverted Euclidean distance, or Jaccard term overlap. Cosine is usually best for text of uneven length. Jaccard compares term sets only, so it ignores the weighting option.
- **Feature weighting** can be TF-IDF, raw term frequency, or binary presence. TF-IDF downweights words that appear across many examples; binary helps when every input is very short.
- **Feature analyzer** can use word n-grams or character n-grams. Character n-grams with length 3-5 are handy for typos, short names, and languages that do not use spaces between words.
- **Lowercase**, **Strip accents**, **Remove English stop words**, **Sublinear term frequency**, and **Minimum example frequency** tune how the vocabulary is prepared before scoring.
- **Minimum confidence** reports `uncertain` instead of the top label when the winner's vote share is below the threshold. Confidence is a relative vote share across labels, not a calibrated probability.
- **Labels to show** controls the score table; `0` lists every label. **Show explanation terms** adds the strongest shared terms and nearest example.
- **Text to classify** is capped at 256 KiB. Batch mode classifies up to 1,000 non-blank lines.
- This is a lexical similarity classifier, not a semantic embedding model. It will not infer that `refund` and `reimbursement` match unless similar words appear in your examples. Add representative examples, use character n-grams for typo tolerance, or lower the confidence threshold when labels are intentionally broad.

## FAQ

<details>
<summary>How many examples do I need per label?</summary>

Two examples per label is the practical minimum; three to eight per label is a better starting point. Include the words and phrases you expect to see at classification time. If a label covers several different topics, use **k nearest examples** or add examples for each subtopic so one centroid is not trying to average unrelated language.

</details>

<details>
<summary>Is this the same as an embedding or LLM classifier?</summary>

No. It uses local lexical features — word or character n-grams with TF-IDF, term frequency, or binary weights — and similarity scoring. That makes it fast, deterministic, private, and transparent, but it does not understand synonyms or paraphrases unless your examples contain overlapping features. For semantic generalisation, use a real embedding or model workflow outside this pure local tool.

</details>

<details>
<summary>Why did it return `uncertain`?</summary>

`uncertain` means the best label did not clear the **Minimum confidence** threshold, or the input shared no vocabulary with the support set. Lower the threshold to always return the top label, add examples that contain the input vocabulary, or inspect the label score table to see which labels were close. Confidence is the label's share of the vote weight for this support set, not a probability calibrated on held-out data.

</details>

<details>
<summary>Can I paste CSV examples?</summary>

Yes for simple two-column data. Choose **Comma** as the separator, or leave **Auto** if comma is the dominant separator. The first separator on each line divides the label from the text, so the text may contain more separators after that point. One layer of surrounding double quotes is stripped and doubled quotes are unescaped, which covers common spreadsheet pastes, but this is not a full RFC 4180 CSV importer with multi-line quoted cells.

</details>

<details>
<summary>Does any text leave my machine?</summary>

No. The classifier core is compiled to WebAssembly for this page and runs in the browser, and the same Rust code is used by the local CLI. There is no network request, model download, remote training job, or saved model. Refreshing the page forgets the support set.

</details>
