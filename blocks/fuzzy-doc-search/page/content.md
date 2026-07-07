## About this tool

Fuzzy Text Search finds the passages in your text that best match what you are looking
for — **even when the words are misspelled**. Paste a document (or several, one after
another), type a query, and get back the matching snippets **ranked by relevance**, with
the matched words highlighted in «guillemets». It is a browser-local search box for a
single body of text: no index to build, no upload, no sign-up.

Unlike a plain find-in-page, it tolerates typos (via edit distance), matches partial words,
can require **all** query words or **any** of them, and sorts the strongest matches to the
top so you read the best passages first.

### Worked example

Paste this text:

```
Shipping policy: orders are dispatched within two days.
Send returns to our adress on file and include the receipt.
Refunds are processed within five working days.
Contact support if your package is delayed or lost.
```

Search for `address` with **Fuzziness = 1**. The document only contains the misspelling
`adress`, but it is one edit away, so you get:

```
1 match for "address"

#1  line 2  score 70
Send returns to our «adress» on file and include the receipt.
```

The score is 70 (a one-typo fuzzy match) rather than 100 (an exact match), and the matched
word is highlighted. Try the **Match all words** preset to require every query word in the
same snippet, or switch **Rank by** to sentence or paragraph to change how the text is cut
into snippets.

### How ranking works

Each snippet gets a 0–100 score: an exact word match scores highest, a prefix/substring
match a little lower, and a fuzzy (typo) match lower still — the more of your query words a
snippet contains, the higher it ranks. Ties are broken by position, so earlier matches come
first.

### Limits and edge cases

- **Plain text and markdown only** (markdown is plain text — just paste it). To search a
  **PDF, DOCX or EPUB**, extract its text first with the
  [Document Text Extract](/tools/document-text-extract/) or
  [PDF Extract Text](/tools/pdf-extract-text/) tool, then paste the result here.
- **Multiple documents:** paste them one after another into the box — ranking spans the whole
  paste. There is no separate multi-file upload (everything runs in one browser tab).
- **Fuzziness is capped at 3** typos per word, and **Max results at 50** — values above the
  caps are clamped.
- Very short terms (one character) are not fuzzy-matched, to avoid matching almost everything.
- Practical for text up to a few megabytes — it all lives in memory in your tab.

## FAQ

<details>
<summary>What does "fuzzy" search actually mean here?</summary>

It means typo tolerance. Each word in your query is compared to the words in the text using
**edit distance** (the number of single-character insertions, deletions, or substitutions to
turn one word into the other). With **Fuzziness = 1** a word matches if it is at most one edit
away — so `color` finds `colour`, and `adress` finds `address`. Set **Fuzziness = 0** for
exact (and prefix) matches only.

</details>

<details>
<summary>Can I search a PDF or Word document?</summary>

Not directly — this tool searches text you paste, and a PDF/DOCX is a binary file. Extract the
text first with [Document Text Extract](/tools/document-text-extract/) (PDF, DOCX, EPUB) or
[PDF Extract Text](/tools/pdf-extract-text/), then paste the extracted text into the box here.
Plain-text and markdown files can be pasted as-is.

</details>

<details>
<summary>What is the difference between "Any word" and "All words"?</summary>

With **Any word (OR)** a snippet is a hit if it contains at least one of your query words —
useful for broad searches. With **All words (AND)** a snippet must contain every query word to
be a hit — useful when you want passages that mention several things together, like
`package delayed`.

</details>

<details>
<summary>Why does "Rank by" offer line, sentence, and paragraph?</summary>

It controls how the text is cut into snippets before ranking. **Line** treats each line as one
snippet (best for logs, lists, or config), **Sentence** splits on `.`/`!`/`?`, and
**Paragraph** splits on blank lines (best for prose). The location shown with each result
(`line 2`, `sentence 5`, …) is numbered in whichever unit you pick.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The search runs entirely in your browser via WebAssembly. Your text never leaves the page —
there is no server call, no account, and nothing is stored.

</details>
