## About this tool

**Sentence Tokenizer** turns plain text into the two layers NLP pipelines need: sentences,
and the tokens inside them. Every token carries its **character span** — a `start` and an
`end` offset into the text you pasted — so a highlight, an annotation or a model prediction
can always be mapped back to the exact characters it came from.

The pipeline is tokenize-then-segment: the scanner walks the text once and emits tokens
with offsets, then the segmenter groups that token stream into sentences. Because
segmentation never re-slices the text, `text[start..end]` always reproduces the token
exactly.

Each token is classified as one of:

- **word** — a word, an abbreviation with its period (`Dr.`), or a contraction piece.
- **number** — including internal separators: `1,000.00`, `2018-11-11`, `12:30`, `3/4`, `3rd`.
- **punct** — terminators, commas, quotes, brackets and dashes; runs like `...` and `!!!`
  collapse into one token.
- **symbol** — currency, maths signs, `@`, `#`, emoji.
- **url** and **email** — kept intact, so their dots never break a sentence.

### Output formats

- **JSON with counts, spans and token types** — the full structure: a `counts` object plus
  every sentence with its span and its tokens.
- **TSV table** — one row per token: sentence number, token number, start, end, type, text.
  Paste straight into a spreadsheet.
- **One token per line** — the flat token stream.
- **One sentence per line, tokens space-separated** — the Treebank-style re-joined form.
- **One sentence per line** — original text, for when you only need segmentation.

### Worked example

Input:

```text
Dr. Green paid $99.99. It works.
```

With **TSV table** output, the first sentence gives:

```text
sentence	token	start	end	type	text
1	1	0	3	word	Dr.
1	2	4	9	word	Green
1	3	10	14	word	paid
1	4	15	16	symbol	$
1	5	16	21	number	99.99
1	6	21	22	punct	.
```

`Dr.` keeps its period and does not end the sentence; `$99.99` splits into a symbol plus a
number, and the final period is the real boundary.

### Boundary rules

The segmenter is deterministic and rule-based — no model, no training data, so the same
input always gives the same output. It keeps these from splitting a sentence:

- Titles and abbreviations: `Dr.`, `Mrs.`, `Prof.`, `Inc.`, `Ltd.`, `e.g.`, `i.e.`, `etc.`
- Numeric prefixes when a number follows: `No. 5`, `Fig. 2`, `Ch. 7`, `Mar. 3`
- Initials and dotted acronyms: `J. R. R. Tolkien`, `U.S.A.`
- Decimals and versions: `$99.99`, `1.2.3`
- List markers: `1. Buy milk`
- Ellipses and quoted speech: `"Stop!" he said.`
- URLs and e-mail addresses

Full-width terminators `。`, `！` and `？` are recognised too, so Chinese and Japanese text
segments correctly.

### Options

- **Line breaks** — `paragraph` (default) treats only a blank line as a boundary; `always`
  ends a sentence at every line break (subtitles, lists); `never` joins wrapped lines.
- **Split contractions** (default on) — Penn Treebank style: `don't` → `do` + `n't`,
  `Anna's` → `Anna` + `'s`. Each piece keeps its true source offsets.
- **Split hyphenated compounds** (default off) — `state-of-the-art` becomes seven tokens.
- **Lowercase token text** — the emitted text is lowercased; offsets still point at the
  original, so nothing is lost.
- **Drop punctuation and symbols** — leaves words, numbers, URLs and e-mails. Boundaries are
  still detected from the punctuation before it is dropped.
- **Extra abbreviations** — add domain terms that must never end a sentence, e.g.
  `Corp., Ltd.`; comma-, semicolon- or space-separated, trailing period optional.

Everything runs locally in WebAssembly. No text is uploaded.

### Limits and edge cases

- Maximum input: **500,000 characters** per run.
- Offsets are **Unicode character** offsets (0-based, `end` exclusive), not byte offsets —
  emoji and accented letters count as one character each.
- Empty or whitespace-only input is an error, and so is a filter combination that removes
  every token (for example dropping punctuation from text that is only punctuation).
- The abbreviation lists are English-oriented. For other languages, add the local
  abbreviations in the **Extra abbreviations** field.
- This is word-level tokenisation. Sub-word / BPE tokens for LLM context budgets are a
  different thing — use a token counter for that.

Also available from the gizza CLI and in chat.

## FAQ

<details>
<summary>What exactly do the start and end offsets refer to?</summary>

They are character positions in the text you pasted, counted in Unicode code points, with
`start` inclusive and `end` exclusive. Slicing the original text from `start` to `end`
returns the token's source characters. Sentences carry the same kind of span. This holds
even when **Lowercase token text** is on, or when a contraction is split — the pieces get
the real spans of their halves, not invented ones.

</details>

<details>
<summary>Why didn't "Dr. Green" split into two sentences?</summary>

`Dr.` is on the built-in never-split list, along with titles, Latin connectives (`e.g.`,
`i.e.`), business abbreviations (`Inc.`, `Ltd.`, `Corp.`) and numeric prefixes such as
`No.` and `Fig.` that only suppress a break when a number follows. Initials (`J. R. R.`),
dotted acronyms (`U.S.A.`), decimals and version numbers are handled by their own rules. If
your text has an abbreviation the list doesn't know, add it in **Extra abbreviations**.

</details>

<details>
<summary>How is this different from a sentence splitter?</summary>

A sentence splitter gives you readable sentences and nothing more. This tool adds the token
layer: every word, number, punctuation mark, symbol, URL and e-mail address as a separate
item with a type and a character span, grouped under the sentence it belongs to. If you only
want one sentence per line, choose the **One sentence per line** format and ignore the rest.

</details>

<details>
<summary>Why is "don't" split into "do" and "n't"?</summary>

That is the Penn Treebank convention, which most English NLP tooling expects: negation
splits as `do` + `n't`, and clitics split as `Anna` + `'s`, `we` + `'ll`, `I` + `'ve`.
Turn off **Split contractions** to keep each contraction as a single token. Words like
`o'clock` are never split either way.

</details>

<details>
<summary>Does it work on languages other than English?</summary>

Word and number scanning is Unicode-aware, so it works on any alphabetic script, and the
full-width terminators `。`, `！` and `？` mean Chinese and Japanese text segments into
sentences correctly. The abbreviation lists, the contraction rules and the capital-letter
heuristic are English-specific, so for other languages expect to add local abbreviations
and to review boundaries that depend on capitalisation.

</details>

<details>
<summary>How much text can I tokenize at once?</summary>

Up to 500,000 characters per run. It is meant for documents, pasted articles and batches of
snippets rather than full-corpus ETL — everything runs in the browser's WebAssembly sandbox,
so very large inputs are better split into chunks or handled by a dedicated pipeline.

</details>
