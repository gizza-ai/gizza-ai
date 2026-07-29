## About this tool

**Smart quotes** (also called *curly* or *typographic* quotes) are the curved quotation
marks used in books, magazines, and polished web copy: `“ ”` for double quotes and `‘ ’`
for single quotes and apostrophes. Code editors, terminals, and plain-text tools instead
use **straight** ASCII quotes: `"` and `'`.

This converter moves text between the two, in either direction:

- **Straight → Curly (educate):** it looks at the character before each straight quote to
  decide whether it *opens* or *closes*. A quote at the start of the text, after a space,
  or after an opening bracket becomes an **opening** mark (`“` / `‘`); a quote after a
  letter or digit becomes a **closing** mark or an **apostrophe** (`”` / `’`). So
  `"Hello," she said. It's a 'test'.` becomes `“Hello,” she said. It’s a ‘test’.`
- **Curly → Straight (straighten):** every curly quote (and prime mark) becomes plain
  ASCII again — exactly what you want before pasting into code, a CSV, or JSON, where a
  stray `”` breaks parsing.

Everything else — accents, CJK, emoji, dashes, and ellipses — passes through untouched.
It all runs locally in your browser: nothing is uploaded.

### Worked example

Input (straight quotes), direction **Straight → Curly**:

```
"Don't," he said, "you dare." Back in '89 the dogs' bowls were 6'4" apart.
```

Output (curly quotes):

```
“Don’t,” he said, “you dare.” Back in ’89 the dogs’ bowls were 6’4” apart.
```

Note `Don’t`, the elided year `’89`, and the plural possessive `dogs’` all get the correct
apostrophe. Turn on **Feet & inches → prime marks** and `6'4"` instead becomes `6′4″`
(true prime marks) rather than curly quotes.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions with a blank line inside each. -->

<details>
<summary>How does it know whether a quote is opening or closing?</summary>

It looks at the character immediately **before** each straight quote. If there's nothing
word-like to the left — the start of the text, a space, or an opening bracket/dash — the
quote **opens** (`“` / `‘`). If the previous character is a letter or digit, it **closes**
(`”` / `’`) or becomes an apostrophe. This is the same heuristic word processors and
SmartyPants use, and it handles `It’s`, `dogs’`, and the elided year `’89` correctly.

</details>

<details>
<summary>Can I convert only double quotes and leave apostrophes alone?</summary>

Yes. **Convert double quotes** and **Convert single quotes / apostrophes** are separate
checkboxes. Un-tick one to leave that kind untouched — for example, curl the `"` in prose
while keeping `'` straight for code identifiers embedded in the text.

</details>

<details>
<summary>What is the feet-and-inches mode?</summary>

With **Feet & inches → prime marks** on (Straight → Curly only), a straight quote directly
after a digit becomes a true **prime** mark instead of a curly quote: `6'` → `6′` (feet /
minutes) and `4"` → `4″` (inches / seconds), so `6'4"` becomes `6′4″`. It's off by default
because most text wants curly quotes there. Straightening always folds prime marks back to
plain `'` and `"` regardless of this setting.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The conversion runs entirely inside your browser via WebAssembly — the text never
leaves your device. The same engine also powers the command-line and chat versions.

</details>

## Limits & edge cases

- **Size cap:** up to **1 MB** of text per conversion (plenty for a long document).
- **English curly set:** it produces the standard EN curly quotes (`“ ” ‘ ’`). Locale
  styles like German `„…“` or French guillemets `« … »` are not generated — use the
  straighten direction if you need to remove them.
- **Ambiguous leading apostrophes:** a single quote at a word boundary followed by a digit
  (e.g. `'89`) is treated as an elided-year apostrophe (`’89`), not an opening quote.
  Genuinely opening single quotes before a number are rare; if you need one, edit the
  result.
- **Scope is quotes (and primes):** dashes, the ellipsis glyph, guillemets, and exotic
  spaces are left as-is. To strip *all* typographic characters to ASCII, use the
  companion **smart-quotes-clean** tool instead.
