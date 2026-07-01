## Encode / decode text in your browser

One tool for the common text codecs — pick a scheme and a direction. Everything
runs locally in your browser; your text is never uploaded.

### Schemes

- **base64** — standard Base64 (RFC 4648).
- **hex** — lowercase hex of the UTF-8 bytes (decode tolerates spaces).
- **binary** — 8-bit per byte, space-separated.
- **url** — percent-encoding (every non-alphanumeric byte is escaped).
- **rot13** — the classic letter rotation (symmetric: encode = decode).
- **morse** — A–Z, 0–9 and common punctuation; letters separated by spaces and
  words by ` / `.

### Notes

- Decoding expects valid input for the chosen scheme and that the bytes form
  valid UTF-8 text; otherwise you'll get a clear error.
- ROT13 ignores the direction (it's its own inverse).

## FAQ

<details>
<summary>Can it decode URL-safe Base64 (with <code>-</code> and <code>_</code>)?</summary>

No — the base64 scheme is the **standard RFC 4648 alphabet** with `+`, `/`, and
`=` padding. A URL-safe variant will fail with an "invalid base64" error; replace
`-` with `+` and `_` with `/` (and restore any trimmed `=` padding) before
decoding.

</details>

<details>
<summary>Why does decoding fail even though my input looks right?</summary>

Two conditions must hold: the input has to be valid for the chosen scheme, *and*
the decoded bytes must form valid UTF-8 text. Scheme-specific gotchas: hex must
have an even number of digits (spaces between bytes are fine), and binary must be
space-separated groups of at most 8 bits each. Binary decoded from arbitrary data
that isn't text will fail the UTF-8 check by design.

</details>

<details>
<summary>Which characters does the Morse codec support?</summary>

A–Z (case-insensitive — everything is uppercased), digits 0–9, and common
punctuation like `. , ? ' ! / ( ) & : ; = + - _ " $ @`. Any other character
stops encoding with a clear "no Morse representation" error. In the output,
letters are separated by single spaces and words by ` / `; decoding accepts the
same layout.

</details>

<details>
<summary>Why does ROT13 produce the same output whether I pick encode or decode?</summary>

Rotating the alphabet by 13 twice gets you back where you started, so ROT13 is
its own inverse — the direction setting is deliberately ignored. Only A–Z letters
are rotated (preserving case); digits, punctuation, and non-Latin characters pass
through unchanged.

</details>
