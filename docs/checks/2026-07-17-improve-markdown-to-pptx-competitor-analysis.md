# markdown-to-pptx — competitor analysis (2026-07-17)

Built new. Scan done BEFORE implementing so the descriptor ships the in-model table-stakes from
the start. All notes are **paraphrased** — no competitor copy, branding, or trademarks reproduced.

## Tools scanned (top real, reachable converters)

1. **Marp / Marpit** (`marp.app`) — the reference open-source Markdown → deck ecosystem (CLI +
   VS Code). Splits pages on the horizontal ruler `---`. Built-in themes (default / gaia /
   uncover), CSS theming, directives for size/pagination. Exports PPTX/PDF/HTML. Note: Marp
   PPTX slides are rasterized *images*, not editable text boxes.
2. **MD Editor** (`app.mdedit.ai`) — paste or upload Markdown, download a `.pptx` in seconds.
   Renders headings, bullet lists, tables. Paste-in editor UX; "adjust settings if available".
3. **markdowntoolbox** (`markdowntoolbox.com`) — type/paste/upload Markdown, download a PPTX.
   Minimal-option, one-click converter.
4. **Sharayeh** (`sharayeh.com`) — no-signup paste → PPTX. Advertises code blocks with
   highlighting, tables, images, and (AI extras) Mermaid/LaTeX.
5. **SlideSpeak / MagicSlides / Manus** — AI decks: analyze headings/lists and *choose* layouts
   (title / two-column / image), auto-generate visuals and speaker notes. Cloud + account/AI.

## Slide-splitting strategies observed
- Split on the `---` thematic break (Marp).
- Split on headings — "each H1/H2 starts a new slide"; some tools auto-detect `---` **or** H1 **or**
  H2 and pick the best strategy.
- H3+ becomes in-slide sub-content.

## Table-stakes → decision

| Capability | Competitors | Our decision |
| --- | --- | --- |
| Split on headings | most | **in-model** — `split_level` = `h1` / `h2` / `both` (default `both`) |
| Split on `---` rule | Marp, auto tools | **in-model** — a `---`/`***`/`___` thematic break always forces a slide break too |
| First heading → slide title | all | **in-model** — the slide's opening heading becomes the title placeholder |
| Bullet lists (nested) | all | **in-model** — `-`/`*`/`+` → bullets, indentation → nesting levels (to 5) |
| Ordered lists | most | **in-model** — `1.` → auto-numbered (arabic-period) bullets |
| Bold / italic / inline code | all | **in-model** — `**b**`, `*i*`, `` `code` `` → run formatting (mono font) |
| Fenced code blocks | Marp, Sharayeh | **in-model** — ``` ``` ``` → monospace verbatim paragraphs |
| Blockquotes | most | **in-model** — `>` → italic paragraph |
| Tables | MD Editor, Sharayeh | **partial (in-model)** — flattened to tab-separated text rows (real PPTX table grid deferred) |
| Sub-headings (below split level) | all | **in-model** — rendered as a bold paragraph inside the slide |
| Theme (light / dark) | Marp themes | **in-model** — `theme` = `light` / `dark`, drives the deck color scheme |
| Aspect ratio 16:9 / 4:3 | Marp size directive | **in-model** — `aspect_ratio` = `16:9` (default) / `4:3` |
| Deck title metadata | most | **in-model** — `title` → `docProps/core.xml` + download filename |
| **Editable** PPTX (real text boxes) | MD Editor et al. | **in-model & better than Marp** — we emit native title/body text placeholders, not rasterized images |
| Presets / example chips | (UX) | **in-model** — `[[example]]` chips on the page |

## Out-of-model (listed, not built)
- **AI layout selection / auto visuals / speaker notes** (SlideSpeak, MagicSlides, Manus) — needs a
  server-side LLM + image generation; gizza is browser-local pure-Rust, no account, no backend.
- **Image embedding** (`![](url)`) — would require fetching remote bytes; a pure no-network tool
  can't. Images are represented by their alt text. (An ffmpeg/media variant could embed uploaded
  images later.)
- **Mermaid diagrams / LaTeX math rendering** — need a diagram/math rendering engine; out of scope
  for a pure text-outline → OOXML converter.
- **Syntax highlighting inside code blocks** — code renders verbatim in a monospace font; per-token
  coloring is deferred (cosmetic, large).
- **Google Slides / PDF export** — different backends; this tool targets a real `.pptx`.

## UX controls we ship
- `theme` and `aspect_ratio` and `split_level` render as `<select>` dropdowns (enum params).
- `title` + `markdown` text fields with real placeholders; `markdown` is a multiline textarea.
- Three one-click example chips (a `#`-per-slide outline, a `#`-title + `##`-sections deck, a
  code/table-heavy deck).
- Binary output → a **Download .pptx** button via `page/custom.js` (mirrors `csv-to-xlsx`).

## Output verification
Tests decode the produced `.pptx` (a ZIP of OOXML parts) and assert the ZIP magic, the presence of
`[Content_Types].xml` / `ppt/presentation.xml` / one `ppt/slides/slideN.xml` per slide, the slide
count in `presentation.xml`, and the title/bullet text inside a slide part.
