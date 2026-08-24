## About this tool

`repeated-word-remover` deletes the words you accidentally typed twice. It is the class of typo a spell-checker never flags, because every word in `I think the the cat sat down` is spelled correctly — only the *pair* is wrong.

The scan is deliberately **adjacent-only**. Two identical words next to each other are a candidate; a word that simply reappears later in the sentence is left alone, because that is normal English, not a typo. Within a run of repeats the **first occurrence always wins**, so its capitalisation, indentation and the punctuation around it survive untouched.

### Worked example

Input:

```
I think the the cat sat on on the mat.
```

Cleaned output:

```
I think the cat sat on the mat.
```

Switch **Result view** to *Marked-up changes* and the original text comes back with each deleted copy struck through, so you can check the edit before you accept it:

```
I think the ~~the~~ cat sat on ~~on~~ the mat.
```

Switch it to *Audit report* and you get counts plus a line and column for every spot found.

### Repeats that are meant to be there

Plenty of correct English doubles a word: *He had had enough*, *the fact that that happened*, *what it is is simple*, *a long long time ago*. These are protected out of the box by the **Never collapse these words** list, which starts as:

```
had, that, is, do, no, very, long, many, far, ha, blah, bye, night, so, chop, tut, yum
```

Add your own words to it, or clear the list entirely if you want every repeat collapsed regardless.

### Options and limits

- **Result view** — `clean` returns the fixed text, `marked` returns the original with deletions struck through in markdown, `report` returns an audit with before/after word counts, the percentage saved, and one `line, col` entry per spot.
- **Match case exactly** — off by default, so `The the cat` is caught and becomes `The cat`. Turn it on when a capitalised sentence opener must never merge with the lower-case word before it.
- **Catch repeats split by a line break** — on by default. A line ending in `the` followed by a line starting with `the` is the single commonest real doubled word in scanned and hard-wrapped text. A *blank* line is a paragraph break and never bridges a repeat.
- **Allow commas and brackets between the words** — off by default. Turn it on to collapse `well, well now`. Sentence-enders `.` `!` `?` and dashes never bridge a repeat in either setting, because they separate two deliberate uses of the word.
- **Also collapse repeated numbers** — off by default, so a pasted table row like `2024 2024` keeps both columns.
- **Minimum word length** — ignore repeats shorter than N characters. A floor of 3 leaves `I I` and `a a` alone while still fixing `the the`. Range 1–20.
- Hyphenated compounds count as one word, so `well-known well-known` is caught. Straight and curly apostrophes are treated as the same character, so `don't don’t` collapses.
- The input limit is 200,000 bytes per run. Split larger documents.

## FAQ

<details>
<summary>Why did it leave "had had" and "that that" in my text?</summary>

Because those are grammatical. `He had had enough` uses the past perfect, and `the fact that that happened` uses `that` as a conjunction and then as a determiner. Both words are in the **Never collapse these words** list by default. Remove them from the list — or clear it — if you want them collapsed anyway.

</details>

<details>
<summary>Will it remove a word that appears twice in the same sentence?</summary>

No. Only *adjacent* repeats are collapsed. In `the cat sat on the mat`, the second `the` is separated by other words, so it is normal English and stays. If you want whole-line or whole-list deduplication instead, that is a different job — use a duplicate-line or list-dedupe tool.

</details>

<details>
<summary>Can I see what would change before applying it?</summary>

Yes. Set **Result view** to *Marked-up changes*. You get your original text back with every copy that would be deleted wrapped in markdown strikethrough (`~~the~~`), so nothing is removed until you decide. *Audit report* goes further and lists the line and column of each spot along with before/after word counts.

</details>

<details>
<summary>Does it handle a word doubled across a line break?</summary>

Yes, and that is on by default. Text that was hard-wrapped or run through OCR often ends one line with a word and starts the next with the same word. Turn **Catch repeats split by a line break** off if you want repeats confined to a single line. A blank line between the two words is treated as a paragraph break and is never collapsed.

</details>

<details>
<summary>What happens to my capitalisation, spacing and punctuation?</summary>

The first occurrence in a run is kept exactly as you wrote it, and everything from the end of that word to the end of the run is deleted. So `The the cat` becomes `The cat`, indentation on a list item is preserved, and the trailing punctuation after the last copy stays attached.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The same Rust core runs locally in the WebAssembly page and in the CLI, so your text is processed in your browser or terminal and never sent to a server.

</details>
