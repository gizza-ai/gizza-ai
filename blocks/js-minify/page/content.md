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

## FAQ

<details>
<summary>Does it rename variables like Terser or UglifyJS?</summary>

No. This is a token-aware whitespace/comment minifier, not a name-mangler:
identifiers are never renamed, and no code is reordered or dropped. The output
is a bit larger than a mangling minifier would produce, but it is guaranteed
to behave identically to the input — which is the point for hand-checked
scripts and vendor files.

</details>

<details>
<summary>How do I keep a license or copyright banner?</summary>

Banners survive comment removal automatically: any `/*! … */` block comment,
or one containing `@license` or `@preserve`, is treated as important and kept
even when "Remove comments" is on. Untick "Keep license/banner comments" if
you really do want *every* comment stripped.

</details>

<details>
<summary>Can removing line breaks break code that relies on ASI?</summary>

No. The minifier knows which source line breaks are significant for
automatic semicolon insertion — for example the one right after a bare
`return` — and emits those as real newlines instead of collapsing them, so
the program's meaning never changes.

</details>

<details>
<summary>Why did a division sign end up with a space around it?</summary>

To keep tokens unambiguous: `a / b` can't be joined into `a/b` when the next
character would form `//` or `/*`, and a `/` after certain tokens would start
a regex literal. The minifier inserts the minimum whitespace needed to keep
every token meaning what it did before.

</details>
