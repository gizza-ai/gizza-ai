## About this tool

**JavaScript Minifier** shrinks JavaScript by removing everything that doesn't
affect how the code runs — extra whitespace, line breaks and comments — to make
files smaller and faster to download.

- **Smaller output**: collapses indentation and unnecessary spaces, drops blank
  lines, and (by default) strips line and block comments.
- **Safe by design**: it's token-aware, not a text hack. Strings, template
  literals and regular expressions are emitted verbatim, identifiers are never
  renamed, and nothing is reordered or dropped — so the minified code is
  semantically identical to the input.
- **ASI-aware**: line breaks that matter for automatic semicolon insertion (for
  example right after `return`) are preserved, so meaning never changes.
- **Remove comments** can be turned off if you want to keep license headers or
  inline notes.

Everything runs **locally in your browser** via WebAssembly — your code is never
uploaded.

### Handy for

- Shrinking a hand-written script before shipping it to production.
- Quickly compressing a snippet to paste somewhere with a size limit.
- Stripping comments and whitespace from a config or vendor file.
