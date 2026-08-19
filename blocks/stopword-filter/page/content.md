## Remove stop words from text

Stop words are the high-frequency filler of a language — *the*, *and*, *of*,
*is* — that carry grammar but almost no meaning. Search engines, keyword
extractors, word clouds and text classifiers usually drop them before they do
anything else. Paste your text, pick a language, and this tool returns the same
text with the filler taken out. It all runs locally in your browser with
WebAssembly; the text never leaves your machine.

### Worked example

Paste:

```
This is a test of the emergency broadcast system.
```

With the **English** list selected, the result is:

```
test emergency broadcast system.
```

Five of the nine words (*this, is, a, of, the*) were stop words. Switch
**Output** to *Summary statistics* and the same text reports the counts instead:

```
Total words: 9
Removed: 5 (55.56%)
Kept: 4
Distinct stop words removed: 5
```

Switch it to *Removed words + counts* and you get an audit list, most frequent
first — for `the cat and the dog and the bird`:

```
3	the
2	and
```

### Options

- **Built-in stop-word list** — English, Spanish, French, German, Italian,
  Portuguese, Dutch, or Russian. Pick **None** to skip the built-in list
  entirely and filter with only your own words.
- **Extra words to remove** — your own terms, on top of the built-in list.
  Separate them with commas, semicolons, spaces, or newlines. Handy for boilerplate
  that is filler in *your* domain: a brand name, `lorem`, a recurring column
  header.
- **Words to keep** — a protection list. Anything here survives even if the
  built-in list contains it. `not`, `no` and `without` are the classic case:
  they are stop words by frequency but they invert the meaning of a sentence,
  so sentiment work usually wants them kept.
- **Case sensitive** — off by default, so `The`, `the` and `THE` all match a
  list entry. Turn it on to remove only exact-case matches, which lets you strip
  a lowercase `it` while leaving the acronym `IT` alone.
- **Also strip punctuation** — off by default, so sentences stay readable. Turn
  it on to get a bare token stream for an NLP pipeline. Line breaks are kept
  either way.
- **Output** — the cleaned text, the list of removed words with counts, or a
  summary of how much was dropped.

### How matching works

Matching is **whole-word and tokenizer-based**, never a substring replace. `the`
never touches the `the` inside `theatre`, and `a` never eats the `a` in `apple`.
Contractions stay a single token, so `don't` matches the list entry `don't`
rather than being split into `don` and `t`. Accented and non-Latin words are
handled the same way, which is why the Russian list works on Cyrillic text.

After a word is removed, the space it left behind is repaired: the output reads
`the cat sat.` → `cat sat.`, not `cat sat .` with a stranded space before the
full stop. Line breaks and paragraph structure are preserved.

### Good for

- **SEO** — stripping filler before you look at keyword density, or cleaning a
  list of search queries down to their content words.
- **NLP preprocessing** — producing the token stream a bag-of-words model,
  TF-IDF index, or naive-Bayes classifier expects.
- **Word clouds and tag lists** — so the biggest word isn't *the*.
- **Shortening text** — squeezing a long note toward a telegram-style summary.

### Limits and edge cases

- Input is capped at **200,000 characters** (roughly 30,000 words). Longer text
  is rejected with a clear error rather than being silently truncated.
- An **unknown language code** is an error that lists the accepted values; an
  unknown output view behaves the same way.
- **Empty text** is not an error — you get an empty result and zero counts.
- Stop-word removal is **lossy and not reversible**. It destroys grammar on
  purpose, so don't run it on text you still need to read as prose, on code, or
  on anything where `not` and `no` matter unless you add them to the keep list.
- The lists are the standard closed-class vocabulary of each language
  (articles, prepositions, pronouns, auxiliaries) — they are deliberately
  conservative. Add domain filler through **Extra words to remove** rather than
  expecting the built-in list to cover it.
- There is no stemming or lemmatisation here: `running` is not reduced to `run`.
  Stop-word removal and stemming are separate steps in a pipeline.

## FAQ

<details>
<summary>Which languages have a built-in stop-word list?</summary>

Eight: English, Spanish, French, German, Italian, Portuguese, Dutch and
Russian. Each list is embedded in the tool, so nothing is downloaded at run
time. If your language isn't there, set the list to **None** and paste your own
words into **Extra words to remove** — that path works for any language,
including ones written in a non-Latin script.

</details>

<details>
<summary>Will removing "the" also break words like "theatre" or "another"?</summary>

No. The text is split into words first and each whole word is compared against
the list, so `the` only ever matches a standalone `the`. `theatre`, `another`
and `them` are untouched. This is the main difference from doing a
find-and-replace in a text editor, which happily eats the middle of words.

</details>

<details>
<summary>How do I stop "not" and "no" from being removed?</summary>

Put them in **Words to keep**. That list wins over everything else, including
the built-in list and your own custom words. It matters more than it sounds:
`the food was not good` becomes `food good` without a keep list — the opposite
of what the sentence said. For any sentiment or intent work, keep the
negations.

</details>

<details>
<summary>Can I see exactly which words were removed?</summary>

Yes — set **Output** to *Removed words + counts*. You get one line per distinct
stop word with the number of times it was dropped, most frequent first, which
is the quickest way to sanity-check a custom list before running it over a
whole corpus. *Summary statistics* gives the totals instead: words in, words
removed, the percentage, and how many distinct stop words were involved.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The filter is compiled to WebAssembly and runs inside your browser tab, so
the text you paste never leaves your device. The page keeps working offline
once it has loaded.

</details>

<details>
<summary>Does this also do stemming or lemmatisation?</summary>

No — that's a different job. Stop-word removal drops whole words from a fixed
list; stemming rewrites the words that remain (`running` → `run`) so that
inflected forms collapse together. In a typical pipeline you filter stop words
first and stem afterwards, using a dedicated stemmer.

</details>
