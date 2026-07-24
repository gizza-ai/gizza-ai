## What this tool does

Paste a **minified or messy** HTML, CSS, JavaScript, or JSON snippet and get it
back cleanly **re-indented and line-broken**. Leave the language on **auto** to
detect it from the snippet, or pick one explicitly when auto-detect guesses
wrong. Choose how many spaces to indent per nesting level, or switch to tabs.

Everything runs locally in your browser. For HTML, CSS, and JavaScript only the
**whitespace and line breaks** change — nothing is renamed, reordered, or
dropped. JSON is parsed, validated, and re-serialized with **key order
preserved**.

## Languages

- **HTML** — a forgiving tokenizer indents nested elements. Void tags
  (`<br>`, `<img>`, …) don't nest, and the contents of `pre`, `textarea`,
  `script`, and `style` are preserved verbatim.
- **CSS** — indents rule blocks and nested at-rules (`@media { … }`),
  normalizes `prop: value;` spacing, and preserves string literals and
  `/* comments */`.
- **JavaScript** — a token-aware pretty-printer re-indents blocks, statements,
  and object literals without changing what the code does.
- **JSON** — parsed and re-serialized. Invalid JSON is reported as an error
  instead of being silently mangled.

## Worked example

Input (minified JSON):

```json
{"name":"gizza","tags":["a","b"],"nested":{"x":1}}
```

Language: `auto` · Indent: `2` spaces

Output:

```json
{
  "name": "gizza",
  "tags": [
    "a",
    "b"
  ],
  "nested": {
    "x": 1
  }
}
```

Switch **Indent character** to `tab` (or raise the space count to 4) to change
the indentation width; switch **Language** to `css`, `html`, or `javascript` to
format those instead.

## Limits and edge cases

- This is a **formatter**, not a linter, minifier, or transpiler — it only
  re-whitespaces the input (JSON is re-serialized).
- Auto-detect is a best-effort heuristic. CSS vs JavaScript can be ambiguous;
  pick the language explicitly if the guess is wrong.
- JSON must be valid — trailing commas, comments, and single quotes are not
  standard JSON and produce an error. HTML, CSS, and JS are formatted
  best-effort even if slightly malformed.
- The contents of HTML `pre`, `textarea`, `script`, and `style` are left
  untouched, so code inside `<script>` is not itself re-indented.
- Comments and string contents are preserved, not reflowed. Long lines are not
  wrapped to a maximum width.
- Spaces-per-level is clamped to 1–8; tabs ignore the spaces setting.

## FAQ

<details>
<summary>Does formatting change what my code does?</summary>

No. For HTML, CSS, and JavaScript only whitespace and line breaks change —
nothing is renamed, reordered, or removed. JSON is parsed and re-serialized with
the original key order preserved, so the data is identical.

</details>

<details>
<summary>What happens if the language auto-detect guesses wrong?</summary>

Set the **Language** field to `html`, `css`, `javascript`, or `json` instead of
`auto`. Auto-detect reliably recognizes HTML (a leading `<`) and valid JSON, but
CSS and JavaScript can look similar, so choose one explicitly when needed.

</details>

<details>
<summary>Can I indent with tabs instead of spaces?</summary>

Yes. Set **Indent character** to `tab` and each nesting level is indented with a
single tab. The spaces-per-level number is ignored in tab mode. For spaces,
choose any width from 1 to 8.

</details>

<details>
<summary>Why did my JSON fail to format?</summary>

The JSON formatter validates the input first, so anything that isn't standard
JSON — trailing commas, `//` comments, single-quoted strings, or unquoted
keys — is reported as an error rather than being guessed at. Fix the reported
issue, or pick a different language if the snippet isn't actually JSON.

</details>

<details>
<summary>Is my code uploaded anywhere?</summary>

No. The formatter runs entirely in your browser via WebAssembly. Your snippet
never leaves your machine and there are no network requests.

</details>
