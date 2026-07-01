## About this tool

The **RAKE Keyword Extractor** finds the most relevant keywords and keyphrases in
any document using **RAKE** — *Rapid Automatic Keyword Extraction*. RAKE is an
unsupervised, language-agnostic algorithm: it needs no training data and no model,
so it runs instantly and entirely in your browser. Nothing you paste is uploaded.

### How RAKE works

1. **Split into candidate phrases.** The text is broken into runs of content words
   at every stopword (the, of, and, …) and punctuation mark. Each run becomes a
   candidate keyphrase.
2. **Score each word.** A word co-occurrence graph is built over the candidate
   phrases. Every word gets a score of *degree ÷ frequency* — words that appear in
   longer phrases and co-occur with many others score higher.
3. **Score each phrase.** A phrase's score is the sum of its member word scores, so
   meaningful multi-word phrases naturally rise to the top.
4. **Rank.** Phrases are sorted by score, highest first.

### Options

- **Max keyphrases** — limit how many results are returned (0 returns every phrase).
- **Max words per phrase** — drop phrases longer than this many words (0 = no limit),
  handy when you only want short, tight keyphrases.

### Good for

- Summarising articles, papers and reports into a handful of key terms.
- Auto-tagging content and generating SEO keywords.
- Quickly seeing what a long document is *about* without reading all of it.

## FAQ

<details>
<summary>Does it work on languages other than English?</summary>

Partially. The RAKE algorithm itself is language-agnostic, but the built-in
stopword list is English (the NLTK-style list RAKE was published with). On
other languages phrases still get split at punctuation and scored, but common
function words won't be filtered out, so expect noisier, longer candidate
phrases.

</details>

<details>
<summary>Why do long phrases dominate the top of the list?</summary>

That's inherent to RAKE: a phrase's score is the *sum* of its member word
scores, and each word's score (degree ÷ frequency) grows when it appears in
longer phrases. If you want short, tag-like keywords, set **Max words per
phrase** to 2 or 3 — longer candidates are dropped before scoring.

</details>

<details>
<summary>What does the score actually mean? Can I compare it across documents?</summary>

The score is the sum of degree÷frequency values for the words in the phrase —
a purely *relative* ranking signal within one document. A score of 9 in one
article and 9 in another don't mean the same thing, so use the ordering (and
gaps between scores), not the absolute numbers.

</details>

<details>
<summary>Why are the extracted phrases all lowercase?</summary>

The text is lower-cased and whitespace-normalized during tokenization so that
"Machine Learning" and "machine learning" count as the same phrase. Words keep
internal apostrophes and hyphens (`don't`, `state-of-the-art`), while any other
punctuation acts as a hard phrase boundary.

</details>
