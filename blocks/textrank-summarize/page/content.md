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
