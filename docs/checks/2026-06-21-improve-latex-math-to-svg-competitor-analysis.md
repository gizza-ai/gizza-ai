# latex-math-to-svg — competitor analysis (2026-06-21)

## Tool

Renders a LaTeX **math** expression into a standalone, scalable SVG with **no TeX
install** — a pure-Rust typesetter (own tokenizer → parser → box-model layout →
SVG primitives), so it runs on every gizza backend including the chat Service
Worker. Surfaces: **chat + CLI** (image-bytes output → no standalone page, like the
QR / chart tools). Params: `latex` (required), `color` (optional CSS colour).

## Top competitors surveyed

| # | Tool | Engine | Formats | Notable features |
|---|------|--------|---------|------------------|
| 1 | latex2image (joeraut.com) | MathJax/server TeX | PNG, JPG, SVG | scale presets, transparent bg, auto `align*` wrap |
| 2 | CodeCogs equation editor | server TeX | PNG, SVG, GIF | URL-as-API, font/colour/size, inline editor |
| 3 | TexRender (texrender.com) | server | PNG, SVG | colour, background, scale, "high quality" |
| 4 | viereck.ch LaTeX-to-SVG | MathJax 4 | SVG | broad TeX coverage, live preview |
| 5 | tex.verbeek.se | KaTeX | SVG, PNG | offline after load, AI "plain English → math" |

(No competitor copy, branding, or trademark was used — feature themes only.)

## Capability diff & gap ranking (fit-to-model)

**Closed / present in our build**

- **Math-mode coverage**: fractions (`\frac`), super/subscripts with `{}` groups,
  roots (`\sqrt`, `\sqrt[n]`), big operators with limits (`\sum \int \prod \oint`
  …), Greek (lower + upper, variants), relations/arrows, binary operators,
  scalable `\left…\right` delimiters, named functions (`\sin \log \lim` …),
  explicit spacing (`\,` `\;` `\quad`), `\mathbb`-style sets (`\R \Z \N` …).
- **Colour customization** (`color` param) — matches CodeCogs/TexRender colour.
- **Scalable vector output** — SVG with a correct `viewBox`, so it scales to any
  size without DPI choices (an SVG is resolution-independent by construction, so a
  competitor's "DPI/scale" knobs are moot for the vector format we emit).
- **Robustness**: unknown commands degrade to upright text instead of erroring, so
  the tool always returns something usable; balanced-brace / empty-input errors are
  reported clearly. Colour input is sanitized (no style-attribute breakout).
- **Privacy / offline**: runs entirely on-device (pure Rust, no server round-trip),
  unlike the server-TeX competitors.

**Out of model (NOT built — documented, not attempted)**

- **PNG/JPG raster export** — gizza's image-bytes surface emits one MIME; SVG is the
  right vector primitive and any consumer can rasterize it. A second raster format
  would need a font-rasterizer dependency and a second output path; deferred.
- **Full document/text mode** (`\begin{align*}`, multi-line `\\`, matrices/`array`,
  `\text{}` paragraphs, custom packages) — this is a *math-fragment* renderer, not a
  TeX engine. Multi-line environments and `\begin{...}` blocks are explicitly out of
  scope.
- **Transparent vs. custom background fill** — output is currently transparent (no
  background rect), which is the most flexible default; a `background` param could be
  added later but isn't a capability gap for the common embed use case.
- **AI "plain English → LaTeX"** (tex.verbeek.se) — that's an LLM feature; in gizza
  the chat model already composes the `latex` argument, so it's covered at the
  surface above this tool rather than inside it.

## Visual / UX verification

Rendered 5 expressions to PNG via headless Chromium and inspected:

- `x = \frac{-b \pm \sqrt{b^2-4ac}}{2a}` (quadratic formula) — fraction bar, nested
  sqrt over a scripted radicand, ± — correct.
- `\sum_{i=1}^{n} i^2 = \frac{n(n+1)(2n+1)}{6}` — sum with above/below limits, nested
  fraction — correct.
- `\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}` — integral limits, exponent,
  explicit thin space, sqrt of π — correct.
- `\alpha + \beta \leq \gamma \neq \delta \to \Omega` — Greek + relations + arrow —
  correct.
- `\left(\frac{a+b}{c}\right)^n` — auto-scaled parentheses around a fraction with a
  superscript on the closing delimiter — correct.

## Test matrix run

- `cargo test --workspace` — 13 tests pass (12 core layout/parse/error + 1 chat-schema
  drift guard).
- `wafer build` — OK, validates + instantiates in wasm32-wasip1 (342.6 KiB block).
- `gizza tool latex-math-to-svg …` — CLI renders quadratic formula, colour param,
  and reports the unbalanced-brace error path; appears in `gizza list`.
- Headless-Chromium visual render of 5 SVGs — all well-formed and legible.
- Page: **N/A** — image-bytes output tools have no standalone page (same as the QR /
  chart tools); chat + CLI are the supported surfaces.
