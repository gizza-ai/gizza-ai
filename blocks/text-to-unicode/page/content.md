# Text to Unicode

Break any text into its Unicode scalar values and inspect each character in detail.

## Inputs

- **Text** — the string to analyze. Every character (including emoji, accents, and invisible control characters) is listed.
- **Output format** — `table` for an aligned ASCII grid, or `json` for an array of objects you can feed into code.

## What you get per character

- **Char** — the rendered glyph (control characters and spaces are shown with a visible placeholder so the table stays aligned).
- **Code Point** — the `U+XXXX` notation.
- **Dec** — the decimal value of the code point.
- **Escape** — the `\u` escape sequence (`\uXXXX` for the Basic Multilingual Plane, `\u{XXXX}` for astral code points such as emoji).
- **UTF-8** — the UTF-8 bytes in hexadecimal.
- **Name** — the official Unicode character name.

Handy for spotting look-alike or invisible characters, debugging text-encoding problems, and writing escape sequences for source code. Everything runs locally in your browser.

## FAQ

<details>
<summary>Why does a single emoji show up as several rows?</summary>

The tool lists one row per Unicode scalar value, and many emoji are sequences
of several code points. A thumbs-up with a skin tone, for example, is
`U+1F44D` followed by the modifier `U+1F3FD`, and family emoji are joined
with invisible `U+200D` (ZERO WIDTH JOINER) characters — each part gets its
own row, which is exactly how you spot them.

</details>

<details>
<summary>Why do some escapes look like A and others like \u{1F600}?</summary>

Code points up to `U+FFFF` (the Basic Multilingual Plane) fit the classic
four-digit `\uXXXX` form. Anything above that — emoji, many historic scripts —
can't, so the tool emits the braced `\u{XXXX}` form used by Rust and modern
JavaScript. The UTF-8 column shows the same character as its raw bytes
(4 bytes for astral code points).

</details>

<details>
<summary>What are the · and ␠ symbols in the Char column?</summary>

They're display placeholders, not the actual characters: control characters
are shown as `·` and a plain space as `␠` so the ASCII table stays aligned.
The real identity is in the other columns — a control character with no
official Unicode name is labeled `<control>` in the Name column.

</details>

<details>
<summary>Can I get the breakdown as data instead of a table?</summary>

Yes — switch the output format to `json` to get an array of objects with
`char`, `codePoint`, `decimal`, `escape`, `utf8` and `name` keys, ready to
feed into a script. The default `table` format is the aligned ASCII grid.

</details>
