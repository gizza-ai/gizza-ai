# markdown-to-slides — competitor analysis (2026-06-22)

Tool: convert a Markdown document into a single self-contained, navigable HTML
slide deck. Surfaces: chat skill block, CLI, standalone page. Pure-Rust
(pulldown-cmark + ammonia), runs entirely client-side / in-sandbox.

## Top competitors

1. **Marp** (marp.app) — Markdown presentation ecosystem; Marpit framework →
   HTML/CSS deck, PDF, PPTX. Directives (`marp: true`, per-slide front-matter),
   built-in themes, image background syntax, math, CLI + VS Code extension.
2. **Slides.com Markdown converter** — paste Markdown, live preview, exports to
   the Slides.com reveal.js-based editor. Slide separators, speaker notes.
3. **SlideSpeak / Presenti / Edraw AI** — AI "Markdown → editable PPTX/Google
   Slides" generators; cloud upload, account, AI re-layout. Output is an
   editable deck, not a standalone file.
4. **Sharayeh** — zero-install web, paste Markdown → PPTX; syntax-highlighted
   code, Mermaid diagrams, LaTeX, tables, images.
5. **partageit/markdown-to-slides** (CLI, npm) — converts a `.md` to a
   self-contained HTML slideshow file (closest in spirit); `---` slide
   separators, themes, watch mode.
6. **Pandoc** — `pandoc -t revealjs/slidy/s5/dzslides` → HTML slides; powerful
   but a CLI install with framework dependencies.

## Capability diff vs. this tool

| Capability | Marp | Slides.com | AI tools | partageit | This tool |
|---|---|---|---|---|---|
| `---` slide separators | yes | yes | n/a | yes | **yes** (`---`/`***`/`___`) |
| Single self-contained HTML file | partial | no (cloud) | no | yes | **yes** |
| No install / no account / no upload | no (CLI/app) | account | account | no (npm) | **yes** (browser/CLI) |
| Private / offline | depends | no | no | yes | **yes** |
| Light + dark theme | yes | yes | yes | themes | **yes** |
| Keyboard + click + swipe nav | yes | yes | n/a | partial | **yes** |
| Slide counter + progress bar | yes | yes | n/a | partial | **yes** |
| Hash deep-link to a slide (`#3`) | yes | yes | n/a | varies | **yes** |
| Print-to-PDF (one slide/page) | yes (export) | yes | yes | varies | **yes** (`@media print`) |
| GFM tables / task lists / strike | yes | yes | yes | partial | **yes** |
| Output sanitized (safe to share) | n/a | n/a | n/a | no | **yes** (ammonia) |

## Gaps considered

In-model, closed in this build:
- **Theme choice** — added `theme` (light/dark) param.
- **Document title** — added `title` param (browser tab).
- **Navigation parity** — keyboard (arrows/PgUp-Dn/Space/Home/End), click edges,
  touch swipe, counter, progress bar, hash deep-link, print-to-PDF: all present.
- **Robust separator parsing** — accepts `---`, `***`, `___` (3+), drops blank
  slides from consecutive/leading/trailing separators, and does NOT mistake a
  GFM table delimiter row (`|---|---|`) for a separator.

Out-of-model (NOT built — require a model, network, binary export, or a stateful
editor; out of scope for a pure client-side tool):
- **PPTX / Google Slides export** (SlideSpeak, Slides.com, Sharayeh) — needs a
  PPTX writer / cloud; this tool targets a portable HTML file. (A separate
  HTML→PDF/print path already exists via the browser.)
- **AI auto-layout / theme generation** (Presenti, SlideSpeak) — needs an LLM.
- **Mermaid diagrams / LaTeX math rendering** (Sharayeh) — needs bundled JS
  renderers (mermaid.js / KaTeX); would defeat the no-dependency, sanitized,
  single-file goal. Deferred.
- **Per-slide front-matter / directives, speaker notes** (Marp) — a larger
  authoring surface; deferred. The `---` separator and standard Markdown cover
  the common case.

## Note

Per skill rules: no competitor copy, branding, or trademarks were used; the deck
chrome (CSS/JS) is original. Out-of-model features are listed, not built.

## Sources

- [Marp](https://marp.app/)
- [Slides.com Markdown to Presentation](https://slides.com/tools/markdown-to-presentation)
- [SlideSpeak](https://slidespeak.co/free-tools/markdown-to-presentation)
- [Sharayeh — Markdown to Slides](https://sharayeh.com/en/tools/markdown-to-slides)
- [partageit/markdown-to-slides](https://github.com/partageit/markdown-to-slides)
- [Presenti AI](https://presenti.ai/markdown-to-presentation/)
- [Opensource.com — Markdown slide generators](https://opensource.com/article/18/5/markdown-slide-generators)
