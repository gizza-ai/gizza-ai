# Unicode to Text

Turn escaped Unicode back into the characters it stands for. Paste a string full of `\uXXXX`, `\u{...}`, `\xXX`, `\U00XXXXXX`, `U+XXXX` or HTML numeric references and get clean, readable text out — no need to tell it which notation you used.

## How it works

The decoder scans your input once and recognises every common escape notation at the same time, so a single mixed string decodes correctly. Anything that isn't an escape is left exactly as you typed it.

Recognised notations:

- **`\uXXXX`** — 4-hex JavaScript / JSON / Java escape. Surrogate pairs (e.g. `😀`) are combined into the real astral character.
- **`\u{X..}`** — braced Rust / ES6 escape, 1–6 hex digits (`\u{1F600}`).
- **`\xXX`** — 2-hex byte / Latin-1 escape (`\x41`).
- **`\U00XXXXXX`** — 8-hex Python wide escape (`\U0001F600`).
- **`U+XXXX` / `u+xxxx`** — Unicode code-point notation, 1–6 hex digits.
- **`&#DDDD;`** — HTML decimal numeric character reference.
- **`&#xHHHH;` / `&#XHHHH;`** — HTML hexadecimal numeric character reference.

Invalid code points (such as a lone surrogate) decode to the Unicode replacement character `U+FFFD`. A fragment that merely looks like the start of an escape but never completes a valid one is passed through verbatim, so nothing is silently lost.

## Why use it

- Read JSON, log files or source strings where non-ASCII characters were escaped to `\uXXXX`.
- Recover emoji and accented text from APIs that return escaped Unicode.
- Decode HTML numeric character references copied out of markup.
- Mix notations freely — the same box handles `é`, `U+2764` and `&#128512;` together.

It is the inverse of an escaping / **Text to Unicode** tool. Everything runs locally in your browser; your text is never uploaded.

## FAQ

<details>
<summary>Does it handle emoji and other astral (non-BMP) characters?</summary>

Yes. <code>\u{1F600}</code>, <code>\U0001F600</code>, <code>U+1F600</code>, the surrogate pair <code>😀</code> and the HTML reference <code>&amp;#128512;</code> all decode to 😀.

</details>

<details>
<summary>Do I have to pick which notation my text uses?</summary>

No. The decoder auto-detects all supported notations in one pass, so a string that mixes several of them decodes correctly without any configuration.

</details>

<details>
<summary>What happens to text that isn't an escape sequence?</summary>

It is passed through unchanged. Plain characters, and even incomplete-looking fragments like a stray <code>\u</code> with no hex digits, are left exactly as you typed them.

</details>

<details>
<summary>Will it touch string escapes like <code>\n</code> or <code>\t</code>?</summary>

No. This tool only decodes Unicode code-point notations. Whitespace/string escapes such as <code>\n</code>, <code>\t</code> and <code>\\</code> are left alone for a dedicated string-unescape tool.

</details>

<details>
<summary>What does U+FFFD in the output mean?</summary>

That is the Unicode replacement character. It appears when an escape names a value that is not a valid Unicode scalar (for example a lone UTF-16 surrogate like <code>\uD83D</code> on its own).

</details>
