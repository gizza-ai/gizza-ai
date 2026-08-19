## About this tool

**Sentence Splitter** breaks a block of text into its individual sentences —
one per line, ready to paste into a spreadsheet, a translation memory, a
subtitle file, or a review pass over a long document.

Splitting on every `.` gets it wrong almost immediately. This tool uses a
rule-based boundary detector that leaves these alone:

- **Titles and abbreviations** — `Dr.`, `Mrs.`, `Prof.`, `e.g.`, `i.e.`, `vs.`, `No. 5`
- **Initials** — `J. R. R. Tolkien`
- **Decimals, money and versions** — `3.14`, `$99.99`, `1.2.3`
- **List markers** — `1. Buy milk` stays with its item
- **Ellipses and quoted speech** — `Wait... really?`, `"Stop!" he said.`

A period followed by a lowercase word is treated as mid-sentence, and the
full-width terminators `。`, `！` and `？` are recognised too.

Worked example — paste this in:

```
Dr. Green paid $99.99 for it. It arrived on Mar. 3 and works fine.
```

and with the default settings you get:

```
Dr. Green paid $99.99 for it.
It arrived on Mar. 3 and works fine.
```

**Output format** picks how the list is rendered: one sentence per line,
a numbered list, a blank line between sentences, or JSON — which adds a total
`count` plus an `index`, `words` and `characters` figure per sentence.

**Line breaks** decides what a newline means in your input. `Only a blank line
ends a sentence` (the default) is right for wrapped prose. `Never end a
sentence` joins hard-wrapped lines back together and relies on punctuation
alone. `Always end a sentence` treats every line as its own sentence, which is
what you want for lists, subtitle cues, and text with no punctuation at all.

**Minimum sentence length** drops fragments shorter than the given number of
characters, and **Extra abbreviations** lets you add your own never-split
terms — handy for company suffixes (`Corp.`, `Ltd.`) or a domain's jargon.

Everything runs locally in your browser as WebAssembly: your text is never
uploaded. Input is capped at 500,000 characters and the minimum-length filter
at 10,000.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown renders. -->

<details>
<summary>How does it know that "Dr." is not the end of a sentence?</summary>

It carries a built-in list of titles and abbreviations that never end a
sentence (`Mr.`, `Mrs.`, `Dr.`, `Prof.`, `Rev.`, `e.g.`, `i.e.`, `cf.`, `vs.`
and more), plus a second list that only suppresses a break when a number
follows (`No. 5`, `Fig. 2`, `Vol. 3`, `Ch. 7`). On top of that, a period
between two digits is never a boundary, a single letter before a period is
read as an initial, and a lowercase word after any terminator means the
sentence is still going.

</details>

<details>
<summary>My text uses an abbreviation the tool splits on. Can I fix it?</summary>

Yes — put it in **Extra abbreviations**. Entries can be separated by commas,
semicolons or spaces, the trailing period is optional, and matching is
case-insensitive, so `Corp., Ltd, Inc.` all work. They are merged into the
built-in never-split list for that run.

</details>

<details>
<summary>What is the difference between the three line-break modes?</summary>

`Only a blank line ends a sentence` (default) treats a single newline as a
space and a blank line as a hard break — correct for normal paragraphs.
`Never end a sentence` ignores newlines entirely, so hard-wrapped text is
rejoined and only punctuation splits it. `Always end a sentence` breaks on
every newline, which is the right choice for bullet lists, subtitle cues, or
any text where each line is already one sentence.

</details>

<details>
<summary>Does it use an AI model?</summary>

No. It is a deterministic rule-based detector compiled to WebAssembly, so the
same input always produces exactly the same output, offline and instantly.
The trade-off is that a genuinely ambiguous case — an unknown abbreviation
followed by a capitalised word — can still be split in the wrong place. Adding
that abbreviation to **Extra abbreviations** fixes it.

</details>

<details>
<summary>Can I get sentence and word counts?</summary>

Choose the **JSON** output format. You get `{"count": N, "sentences": [...]}`
where each sentence carries its 1-based `index`, the `text`, a `words` count
(whitespace-separated) and a `characters` count (Unicode characters).

</details>

<details>
<summary>Does it work on languages other than English?</summary>

Partly. The abbreviation lists are English, but the core rules — terminator
runs, closing quotes and brackets, decimals, lowercase continuation — apply to
any Latin-script text, and the full-width terminators `。`, `！` and `？` used
in Chinese and Japanese are split correctly without needing a following space.
Languages whose sentence boundaries do not rely on those marks are out of
scope.

</details>
