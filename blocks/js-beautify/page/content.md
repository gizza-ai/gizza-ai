## About this tool

**JavaScript Beautifier** re-indents and pretty-prints minified, packed or
obfuscated JavaScript into clean, readable source — without changing what the
code does.

- **One statement per line** and properly indented nested blocks, objects and
  arrays, so you can actually read packed or minified JS.
- **Indent** sets the spaces per nesting level (1–8). Default is 2.
- **Safe by design**: strings, template literals, regular expressions and
  comments are emitted verbatim. Only whitespace and line breaks change, so the
  output is semantically identical to the input — nothing is reordered or dropped.

Everything runs **locally in your browser** via WebAssembly — your code is never
uploaded.

### Handy for

- Reading a minified `bundle.js` or vendor script to understand what it does.
- Inspecting obfuscated or packed snippets pasted from the wild.
- Cleaning up hand-written JS that lost its formatting.

## FAQ

<details>
<summary>Can beautifying break my JavaScript?</summary>

No. The formatter tokenizes the source and emits strings, template literals,
regex literals, and comments byte-for-byte verbatim — only whitespace and line
breaks are changed. Nothing is renamed, reordered, or dropped, so the output is
semantically identical to the input.

</details>

<details>
<summary>Can I indent with tabs instead of spaces?</summary>

Yes — set the indent character to `tab` and each nesting level becomes one tab.
With spaces (the default), the indent width is configurable from 1 to 8 spaces
per level (default 2); values outside that range are clamped.

</details>

<details>
<summary>Will it rename obfuscated variables back to readable names?</summary>

No — this is a formatter, not a deobfuscator. Single-letter or mangled
identifiers stay exactly as they are; what you get is proper line breaks and
indentation so the *structure* becomes readable. Recovering meaningful names
requires source maps or manual analysis.

</details>

<details>
<summary>Is the code I paste sent to a server?</summary>

No. Formatting runs entirely in your browser via WebAssembly, so proprietary or
sensitive scripts never leave your machine. The same formatter is available from
the gizza CLI and in chat.

</details>
