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
