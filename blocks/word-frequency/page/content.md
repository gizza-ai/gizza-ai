## About this tool

The word frequency counter tallies how often each word appears in your text and
ranks the results from most to least common. It is handy for spotting filler
words in an essay, checking keyword density in web copy, finding the dominant
terms in a transcript, or just getting a quick feel for what a document is about.

Each row of the output is the count, the percentage of all counted words (its
keyword density), and the word, most frequent first. Ties keep the order in
which the words first appeared, so the result is fully deterministic.

### Options

- **Case sensitive** — off by default, so `The` and `the` are grouped together
  (the first casing seen is shown). Turn it on to count each casing separately.
- **Minimum word length** — skip words shorter than this many characters, an
  easy way to drop one- and two-letter noise.
- **Ignore common stop words** — drop frequent English filler words such as
  *the*, *and*, *of*, and *to* so the meaningful terms rise to the top.
- **Top N** — keep only the N most frequent words (leave it at 0 to list every
  distinct word).

### How words are detected

A word is a run of letters or digits, with interior apostrophes preserved so
contractions like *don't* and possessives like *it's* stay intact. Every other
character — spaces, punctuation, line breaks — separates words. Accented and
non-Latin letters are counted too.

Everything runs locally in your browser using WebAssembly. Your text is never
uploaded to a server.
