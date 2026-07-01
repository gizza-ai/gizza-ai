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

## FAQ

<details>
<summary>Is "don't" counted as one word or two?</summary>

One. An apostrophe *between* two word characters — straight `'` or curly `’` —
is kept inside the token, so *don't*, *it's*, and *o'clock* each count as a
single word. A leading or trailing apostrophe (as in `'quoted'`) is treated as
punctuation and stripped.

</details>

<details>
<summary>Does it work on text that isn't English?</summary>

Counting does — tokenization is Unicode-aware, so accented and non-Latin words
(café, naïve, 東京) are tallied correctly. The **stop-word list is English
only**, though: turning on *Ignore common stop words* won't drop *le*, *der*,
or *и*, so for other languages combine it with *Minimum word length* instead.

</details>

<details>
<summary>Two words have the same count — which comes first?</summary>

The one that appeared first in your text. Ties always keep first-seen order,
so running the same input twice gives byte-identical output — handy when you
diff results.

</details>

<details>
<summary>Which casing is shown when "Case sensitive" is off?</summary>

The first casing that occurs in the text. If your document starts a sentence
with `The`, the merged row for the/The/THE displays as `The` — the counts and
percentages are unaffected, only the label.

</details>
