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
