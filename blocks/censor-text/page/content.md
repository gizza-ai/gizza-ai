## Censor text

Mask out words in any text. Give a comma-separated list of words to redact — or
leave it blank to use a built-in common-profanity list — and each match is
replaced with a mask character. It all runs in your browser; nothing is uploaded.

### Options

- **Words** — the terms to redact (case-insensitive). Blank uses the built-in
  list.
- **Mask character** — what to replace each masked character with (default `*`).
- **Whole words only** — on by default, so `ass` won't censor `class`. Turn it
  off to also mask matches inside larger words.

### Good for

- Scrubbing names, secrets, or sensitive terms out of a snippet before sharing.
- A quick profanity filter for user-generated text.

### FAQ

<details>
<summary>Is my text uploaded?</summary>

No — it's processed locally in your browser with
WebAssembly.

</details>

<details>
<summary>Does the masked output reveal how long the censored word was?</summary>

Yes — each character of a match is replaced one-for-one with the mask
character, so `damn` becomes `****`. That keeps the text readable, but it does
preserve word length; if that matters, edit the result before sharing.

</details>

<details>
<summary>Can I censor multi-word phrases?</summary>

Yes. Entries in the word list are separated by commas, so an entry can contain
spaces — e.g. `credit card, John Smith` masks the whole phrase wherever it
appears (case-insensitively).

</details>

<details>
<summary>What if I enter more than one mask character?</summary>

Only the first character is used. Typing `##` or `#x` still masks with `#`;
leaving the field blank falls back to the default `*`.

</details>
