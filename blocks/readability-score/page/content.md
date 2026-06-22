## About this tool

**Readability score** grades how hard your text is to read using the formulas
editors, teachers and content teams rely on:

- **Flesch Reading Ease** — a 0–100 score where higher means easier (60+ is
  considered plain English).
- **Flesch-Kincaid Grade Level** — the US school grade needed to read the text.
- **Gunning-Fog Index** — years of formal education a first reading should
  require, weighted by "complex" (3+ syllable) words.
- **SMOG Index** — a grade level built from polysyllabic word counts, popular for
  health and safety copy.
- **Coleman-Liau Index** — a grade level based on letters per word and sentences
  per word (no syllable estimate, so it sidesteps syllable-counting error).
- **Automated Readability Index (ARI)** — a grade level from characters per word
  and words per sentence.

It also shows the raw counts behind the scores — words, sentences, syllables and
complex words — plus average words per sentence and syllables per word, so you can
see *why* a passage scores the way it does.

### Privacy

Everything runs **in your browser** via WebAssembly — your text is never uploaded
to a server. You can also run it from the [gizza CLI](/) or inside a gizza chat,
which return the same indices as structured JSON.

### Common uses

- Aim an article, email or landing page at a target reading level.
- Simplify dense paragraphs until the grade level drops.
- Compare drafts to see which reads more easily.

### Notes

The syllable count uses an English heuristic (vowel groups with silent-`e` and
`-le` adjustments), so scores are estimates — accurate enough to compare drafts
and hit a target band, but they can differ by a fraction of a grade from a
dictionary-based counter. SMOG is designed for samples of about 30 sentences.
