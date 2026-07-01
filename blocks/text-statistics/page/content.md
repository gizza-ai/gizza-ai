## About this tool

**Text statistics** breaks down any text into the numbers writers and editors care
about:

- **Words** and **characters** (with and without spaces)
- **Sentences** (runs ending in `.`, `!`, or `?`)
- **Paragraphs** (blocks separated by blank lines) and **lines**
- **Reading time** (≈200 words/minute) and **speaking time** (≈130 words/minute)
- **Average word length** and **average words per sentence** (quick readability
  signals)

### Privacy

Everything runs **in your browser** via WebAssembly — your text is never uploaded
to a server. You can also run it from the [gizza CLI](/) or inside a gizza chat,
which return the same numbers as structured JSON.

### Common uses

- Check an essay, article, or social post against a word/character limit.
- Estimate how long a script will take to read aloud.
- Get a fast readability sense from sentence length and word length.

## FAQ

<details>
<summary>How are words counted — what about hyphens and numbers?</summary>

A word is any run of characters between whitespace, so `state-of-the-art` counts
as **one** word and `3.14` counts as a word too. The *average word length* stat
is smarter: it counts only letters and digits, ignoring attached punctuation like
a trailing period, so it reflects real word size.

</details>

<details>
<summary>How does the sentence count handle "?!", "..." and missing final periods?</summary>

A sentence ends at a run of `.`, `!`, or `?` — so `?!` and `...` each close a
single sentence, not two or three. Text that trails off without any terminator
still counts as one final sentence, which means a one-line note like
`hello world` reports 1 sentence rather than 0.

</details>

<details>
<summary>What reading speed do the time estimates assume?</summary>

Reading time is words ÷ **200 wpm** (average silent reading) and speaking time is
words ÷ **130 wpm** (a comfortable presentation pace), each rounded to one
decimal minute. If you speak faster or slower, scale accordingly — the word count
is the reliable number.

</details>

<details>
<summary>What's the difference between paragraphs and lines?</summary>

**Lines** are physical newlines; **paragraphs** are blocks of text separated by
at least one blank line. A hard-wrapped paragraph of five lines therefore counts
as 5 lines but 1 paragraph — handy to know when your editor wraps text
automatically.

</details>
