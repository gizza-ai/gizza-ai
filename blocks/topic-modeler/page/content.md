## About this tool

Topic modelling finds recurring themes across a collection of documents without labels. This tool fits a small LDA (Latent Dirichlet Allocation) model directly from the text you paste, then reports each learned topic's top words and each document's mixture of topics. It is designed for meeting notes, support tickets, survey responses, research snippets, and other short corpora where you want a quick local map of repeated themes.

The model is deterministic for a given seed. It tokenises text, lowercases words, removes optional English stopwords plus any stopwords you add, prunes short tokens, and runs collapsed Gibbs sampling over the document-word matrix. No text is uploaded and no pretrained model is downloaded.

### Worked example

Paste four short documents separated by blank lines:

```text
Butter flour sugar and oven heat make a crisp pastry.
Baking dough with butter and sugar creates a golden crust.

Compiler modules check function signatures and return types.
Type errors appear when the module function returns the wrong value.
```

Set `topics = 2`, keep the default seed, and run the report output. You should see one topic whose words lean toward baking terms and one whose words lean toward compiler/module terms, followed by a document-mixture section that shows each document's strongest topic.

### Output formats

- `report` gives a readable summary: corpus size, effective priors, ranked topic labels, top words with weights, and document mixtures.
- `json` returns the full model with topics, word probabilities, document previews, and mixture weights.
- `csv` returns a topic-keys table followed by a document-topic matrix, matching the shape many topic-modelling CLIs produce.

### Limits and edge cases

This browser-safe implementation caps the corpus at 300 documents, 25,000 kept tokens, and 20,000 vocabulary terms. Very tiny corpora can produce unstable topics; use a fixed seed and try a few topic counts before treating the result as a real pattern. For PDFs, DOCX, EPUB, HTML, or transcripts, extract text with a separate tool first and paste the plain text here.

## FAQ

<details>
<summary>Is this the same as a hosted NLP topic-modelling service?</summary>

It uses the same broad LDA idea, but it is intentionally smaller and local. There are no uploads, accounts, dashboards, coherence plots, word clouds, or saved projects. The result is a quick topic word list plus a document-topic matrix you can copy elsewhere.

</details>

<details>
<summary>How many topics should I choose?</summary>

Start small. For a short pasted corpus, try 2–5 topics and increase only if the word lists merge unrelated themes. Too many topics on too little text usually creates duplicate or noisy topics.

</details>

<details>
<summary>What does alpha do?</summary>

Alpha controls how mixed each document is. Lower alpha makes each document prefer fewer topics; higher alpha allows each document to blend more topics. Leave `alpha` at `0` to use the common MALLET-style automatic value `50 / topics`.

</details>

<details>
<summary>Can I use non-English text?</summary>

Yes, if the text is whitespace-tokenised, but the built-in stopword list is English only. Turn off English stopwords or paste your own comma/space-separated stopword list for the language you are analysing.

</details>

<details>
<summary>Why did changing the seed change the topic words?</summary>

LDA sampling starts from random topic assignments. The seed makes that randomness reproducible. If a topic only appears for one seed, it may be weak; stable themes tend to reappear across nearby settings and seeds.

</details>
