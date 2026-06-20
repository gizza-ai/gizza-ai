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

**Is my text uploaded?** No — it's processed locally in your browser with
WebAssembly.
