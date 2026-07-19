## About this tool

Text Normalizer cleans and canonicalizes text in one configurable pass — ideal for
building search keys, deduplicating records, preparing NLP input, or just tidying
text pasted from PDFs, chat apps, and word processors. Everything runs locally in
your browser through WebAssembly; your text is never uploaded.

The pipeline runs in a fixed, predictable order:

1. **Straighten punctuation** — fold curly quotes, guillemets, en/em dashes, the
   ellipsis glyph, and prime marks to plain ASCII (`"`, `'`, `-`, `...`).
2. **Unicode form** — apply NFC, NFD, NFKC, or NFKD. NFC is the web-recommended
   canonical form; **NFKC** additionally flattens fancy fonts — ligatures (ﬁ → fi),
   full-width and circled/styled letters (𝐇𝐞𝐥𝐥𝐨 → Hello), and fractions.
3. **Strip accents** — decompose letters and drop combining marks (café → cafe,
   naïve → naive).
4. **Case** — lowercase, UPPERCASE, or case-fold. **Case fold** is for caseless
   matching: like lowercase, but it also maps ß → ss so *STRASSE* and *straße* compare equal.
5. **Whitespace** — collapse every run of whitespace to a single space (the default),
   trim the ends only, or collapse each line while keeping line breaks.

Whitespace runs last so it tidies anything the earlier steps introduce. Every option
is independent — turn on only what you need, and the defaults (NFC + collapse
whitespace) already give clean, web-safe text.

### Worked example

Input (NLP-prep preset — NFKC, case fold, strip accents, collapse, straighten punctuation):

```
  Café  “RÉSUMÉ”…
```

Output:

```
cafe "resume"...
```

## FAQ

<details>
<summary>What is Unicode normalization, and which form should I pick?</summary>

Unicode often has several ways to encode the same visible text — for example an
accented `é` can be one code point or an `e` plus a combining accent. Normalization
rewrites text to a single canonical representation so comparisons and searches
behave. Use **NFC** (the default) for general web text, **NFKC** when you also want
to flatten ligatures and fancy-font letters to their plain equivalents, and the
**NFD/NFKD** decomposed forms when a downstream tool expects separated base letters
and marks.

</details>

<details>
<summary>What is the difference between lowercase and case fold?</summary>

Lowercase applies the standard Unicode lowercase mapping. **Case fold** is designed
for caseless *matching* rather than display: it lowercases and additionally expands
a few characters that have no single lowercase form — most notably the German
`ß`, which folds to `ss`. Use case fold when you want *STRASSE*, *Straße*, and
*strasse* to all compare equal; use plain lowercase when you just want readable
lower-case text.

</details>

<details>
<summary>Does this delete punctuation or numbers like an NLP cleaner?</summary>

No. Text Normalizer *normalizes* — it does not destroy content. Turning on
**Straighten fancy punctuation** converts typographic characters (curly quotes,
em dashes, the ellipsis glyph) to their ASCII equivalents, but it never removes
punctuation or digits. If you need to strip punctuation or numbers entirely, that is
a separate destructive cleaning step outside this tool's scope.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The tool is a WebAssembly module that runs entirely in your browser. Your text
never leaves your device — there is no server round-trip, so it is safe for private
or sensitive content.

</details>
