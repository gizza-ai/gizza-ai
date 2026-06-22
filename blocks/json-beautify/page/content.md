## About this tool

**JSON Beautifier** reformats minified or messy JSON into clean, readable text —
with the indentation you choose — and **validates** it along the way.

- **Indent** sets the spaces per level (1–8). Set **indent to 0** to *minify*
  instead: the whole document collapses to one compact line.
- **Validation built in**: if the JSON is malformed, you get the parser's exact
  message (with line/column) instead of broken output.
- **Key order is preserved** — objects come out in the same order you wrote them,
  not alphabetically sorted.

Everything runs **locally in your browser** via WebAssembly — your data is never
uploaded.

### Handy for

- Making an API response or config readable.
- Minifying JSON before embedding it somewhere size-sensitive.
- Quickly checking whether a blob is valid JSON and where it breaks.
