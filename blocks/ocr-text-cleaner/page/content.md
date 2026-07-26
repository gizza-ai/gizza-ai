## About this tool

Optical character recognition is fast, but it is not clean. Text pulled from a
scan, photo, or PDF arrives with a predictable set of **mechanical** errors:
typographic ligatures that never got decomposed (`ﬁ`, `ﬂ`), words split by a
hyphen at the end of a line, capital-I and lowercase-l standing in for each
other, zeros posing as the letter O, columns wrapped mid-sentence, and stray or
missing spaces around punctuation.

This cleaner fixes exactly those errors — deterministically. There is **no
dictionary and no language model**: every transformation is a fixed rule, so the
same input always produces the same output and nothing is silently rewritten
based on a guess about meaning. Toggle each fix on or off, choose how line breaks
are handled, and the cleaned text updates live. Everything runs locally in your
browser; your text is never uploaded.

It pairs well with any OCR engine — run recognition first, then paste the raw
output here to strip the noise before you proofread.

## Worked example

Raw OCR output, with a ligature, a confusable-mangled word, a doubled space, and
spaces on the wrong side of punctuation:

```
The ﬁnal  report says HeIIo ,world .Next
```

With **Expand ligatures**, **Fix confusables**, and **Normalize spacing** on
(the defaults), the tool returns:

```
The final report says Hello, world. Next
```

`ﬁ` became `fi`, `HeIIo` (capital-I confusables) became `Hello`, the double space
collapsed, the space before each punctuation mark moved to after it, and a
missing sentence space was inserted before `Next`.

## Line-break handling

Scanned columns often wrap every physical line, which is wrong once the text is
reflowed. The **Line breaks** control offers three modes:

- **Keep as-is** — only line-ending characters are normalized (`\r\n` → `\n`).
- **Reflow within paragraphs** — single line breaks inside a paragraph become
  spaces, while blank-line paragraph breaks are preserved. This is the usual
  "un-wrap a scanned column" fix.
- **Collapse to one line** — every line break becomes a single space.

Words hyphenated across a line break (`para-` / `graph`) are rejoined into
`paragraph` when **Rejoin words hyphenated across line breaks** is on. A hyphen
followed by an uppercase word is treated as a real compound (`New-` / `York` →
`New-York`), so the hyphen is kept.

## Limits & edge cases

- **Input size:** up to 200,000 characters per run. Larger documents should be
  split into chunks.
- **Confusables are context-based, not dictionary-based.** A digit is only
  treated as an in-word letter when it sits *between* two letters, which protects
  alphanumeric codes like `COVID19`, `MP3`, and `B12`. The pronoun `I`,
  capitalized sentence starts, and all-caps acronyms are left alone.
- **`rn → m` is aggressive and off by default.** With no dictionary it cannot
  tell `rn`-in-`modern` from `rn`-that-should-be-`m`, so enabling it will also
  rewrite real words. Use it only on text you know is affected.
- **Non-Latin text is preserved.** Accents, CJK, and emoji pass through
  untouched; the tool only rewrites the specific Latin confusables and ligatures
  it knows about.
- **Curly-quote / smart-quote straightening is intentionally not included** —
  that is a separate concern from OCR mechanical errors.

## FAQ

<details>
<summary>Does this tool spell-check or correct real words?</summary>

No. It only fixes *mechanical* OCR errors with fixed rules — ligatures,
hyphenation, line breaks, character confusables, and spacing. It has no
dictionary, so it will not insert a missing space between two glued real words
(`thejob`), add a missing apostrophe (`dont`), or correct a genuine misspelling.
That keeps every change predictable and reversible in your head.

</details>

<details>
<summary>Why did it leave "COVID19" and "MP3" alone?</summary>

Confusable fixing is context-aware. A digit is only converted to a letter when
it is surrounded by letters on both sides, and a letter is only converted to a
digit inside a number-dominated token. `COVID19` and `MP3` end in digits that
are not flanked by letters, so they are recognized as codes and left untouched —
while `l00` (letters posing as digits) still becomes `100`.

</details>

<details>
<summary>What is the "rn → m" option and why is it off by default?</summary>

`rn` is one of the most common OCR merge errors: the letter `m` gets misread as
the two letters `r` and `n`, turning `modem` into `rnodem`. Undoing it is
valuable, but with no dictionary the fix is blind — it replaces *every* `rn`,
including the ones inside legitimate words like `modern` or `turn`. Because that
can corrupt good text, it ships off; turn it on only for text you know suffers
from the merge.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The cleaner is compiled to WebAssembly and runs entirely in your browser
tab. Nothing you paste leaves your device, which makes it safe for confidential
scans and works offline once the page has loaded.

</details>
