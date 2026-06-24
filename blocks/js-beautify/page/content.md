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
