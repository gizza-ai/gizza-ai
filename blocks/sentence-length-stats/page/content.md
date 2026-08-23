## About this tool

**Sentence Length Stats** measures the rhythm of prose by counting the words in every sentence. It
reports the sentence count, total words and characters, average and median length, shortest and
longest sentence, standard deviation, a 0–100 variety score, long-sentence percentage, adjacent
sentences with near-identical lengths, and a five-band distribution from very short to very long.

Use it when editing web copy, documentation, scripts, newsletters, UX text, or any draft where you
want a quick view of pacing. Short sentences can add punch. Long sentences can carry nuance. The
useful signal is often the mix.

The splitter is rule-based and deterministic. It handles common abbreviations, initials, decimals,
and numbered list prefixes without treating every period as a sentence break. Your text is processed
locally in the browser.

### Example

Input:

```text
Short sentences land fast. Longer sentences can carry nuance, context, and rhythm when they are placed carefully. Mix both.
```

With the long-sentence threshold set to `12`, the report starts with:

```text
Sentences: 3
Words: 18
Average length: 6.0 words
```

and includes a distribution table plus the longest sentences list.

### Options

- **Line break handling** — choose whether newlines are ordinary spaces, only blank-line paragraph
  breaks, or sentence breaks on every line. The last mode is useful for subtitles, bullet lists, and
  one-line-per-row exports.
- **Long sentence threshold** — word count at or above which a sentence is counted as long. The
  default is 25; try 20 for strict plain-language editing.
- **Longest sentences to list** — how many of the longest sentences to show with sentence number and
  a short snippet. Set `0` to omit the section.
- **Extra abbreviations** — add domain-specific abbreviations that should not end a sentence, such as
  `dept`, `approx`, or `ing`.

### Limits and edge cases

- Text is capped at the same 500,000-character limit as the sentence splitter block.
- Word counts use the splitter's token rules, so punctuation and quotes are not counted as words.
- The variety score is not a grammar grade. It is a compact pacing signal based on the spread of
  sentence lengths.
- A single-sentence input has no adjacent pairs and no meaningful variety score, so those fields are
  reported as not applicable.
- Sentence detection is heuristic. Add custom abbreviations when your domain uses short forms that
  look like sentence endings.

## FAQ

<details>
<summary>What is a good average sentence length?</summary>

There is no universal target. Web and help copy often benefits from averages around 12–18 words,
while technical or narrative prose can be longer. The average is best read together with the longest
sentence and the distribution: one very long sentence can hide behind a reasonable average.

</details>

<details>
<summary>What does the variety score mean?</summary>

It summarizes how much sentence lengths vary. Very similar sentence lengths score low and can feel
monotonous. A stronger mix of short, medium, and long sentences scores higher. It is a pacing hint,
not a pass/fail writing rule.

</details>

<details>
<summary>When should I set newlines to “always”?</summary>

Use **Every line is a sentence** for subtitles, transcript exports, bullet-like notes, or datasets
where each row is meant to stand alone even without terminal punctuation. For normal prose, leave the
default paragraph mode on.

</details>

<details>
<summary>Why did an abbreviation split into a sentence?</summary>

The splitter knows common forms such as `Dr.`, `Mr.`, `e.g.`, initials, decimals, and numbered list
prefixes. If your text has domain-specific abbreviations, add them in **Extra abbreviations** without
the trailing period, for example `dept, approx, ing`.

</details>

<details>
<summary>Does this replace a readability score?</summary>

No. Readability formulas combine word and sentence lengths into a grade-like number. This tool keeps
the sentence-length signals separate so you can see the distribution, the longest sentences, and the
pacing pattern directly.

</details>
