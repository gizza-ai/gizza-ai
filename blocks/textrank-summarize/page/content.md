## About this tool

**TextRank summarizer** condenses a long passage into its most important
sentences. It builds a graph where each sentence links to others it shares words
with, runs **PageRank** over that graph, and keeps the highest-scoring sentences —
the same idea Google used to rank web pages, applied to sentences.

- **Extractive:** the summary is made of **real sentences from your text**,
  verbatim and in their original order — nothing is paraphrased or invented.
- **No AI model:** it's a deterministic graph algorithm, so the same input always
  gives the same summary, instantly, with no model download.
- Choose how many sentences you want in the summary.

### Privacy

Everything runs **in your browser** via WebAssembly — your text is never uploaded
to a server. You can also run it from the [gizza CLI](/) or inside a gizza chat.

### Best for

Articles, reports, meeting notes, and transcripts — anything with several
sentences where you want a quick TL;DR built from the original wording.

## FAQ

<details>
<summary>How many sentences can the summary contain?</summary>

Anywhere from 1 to 50 (the default is 3). If your text has no more sentences
than you asked for, it's returned whole — the tool never invents content to
pad a summary out.

</details>

<details>
<summary>How are sentence boundaries detected?</summary>

A sentence ends at `.`, `!` or `?` followed by whitespace or the end of the
text. Because a decimal point like `3.14` isn't followed by a space, it
doesn't split — but abbreviations such as "e.g. " or "Dr. Smith" *will* end a
sentence, which can occasionally fragment one. Trailing text without final
punctuation still counts as a sentence.

</details>

<details>
<summary>Will the summary sentences be reworded?</summary>

Never. TextRank is extractive: it scores the sentences you wrote (connecting
ones that share content words, after removing stopwords, then running
PageRank) and returns the top scorers verbatim, in their original document
order. Same input, same summary — there's no model and no randomness.

</details>

<details>
<summary>Why does it favor certain sentences?</summary>

Sentences that share vocabulary with many other sentences act like highly
linked pages in PageRank — they accumulate score. Similarity is normalized by
sentence length (log-scaled), so a long rambling sentence doesn't win just by
containing more words.

</details>
