## About this tool

Paste a LaTeX math expression and get back **MathML** — the W3C standard for
mathematics on the web. Unlike a rendered image, MathML stays *text*: it's read
correctly by screen readers, can be selected, copied and searched, scales with
the surrounding font, and drops straight into HTML pages, EPUB e-books, DocBook
and Office documents.

Write the expression in math mode with **no surrounding `$`** — for example
`\frac{a}{b}`, `x^2`, `\sqrt{x+1}`, `\sum_{i=1}^{n} i`, `\alpha + \beta`, or
`\left(\frac{a}{b}\right)`. The converter outputs a single
`<math xmlns="http://www.w3.org/1998/Math/MathML">…</math>` element.

### What it supports

- Fractions (`\frac`, `\binom`), roots (`\sqrt`, with optional index)
- Superscripts and subscripts, including grouped `{…}` exponents
- Greek letters, blackboard/bold/calligraphic fonts (`\mathbb`, `\mathbf`, …)
- Big operators with limits (`\sum`, `\prod`, `\int`) and relations/arrows
- Scalable delimiters via `\left…\right`
- Matrix and align environments, and explicit spacing

### Options

- **Display mode** — `block` (default) emits a centred standalone equation
  (`display="block"`); `inline` flows the equation within a line of text.
- **Pretty-print** — indent the output with one element per line for readable,
  diff-friendly markup. Off by default for compact single-line output.

Everything runs locally in your browser via WebAssembly — your expressions are
never uploaded.
