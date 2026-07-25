# pptx-to-text — competitor analysis (2026-07-25)

Tool: `pptx-to-text` — extract text and a per-slide outline from a modern
ZIP-based PowerPoint `.pptx` (Office Open XML PresentationML) file.

Distinct from the existing `ppt-to-text` block, which reads the LEGACY binary
`.ppt` (OLE2/Compound File Binary, PowerPoint 97–2003). A `.pptx` is a ZIP of
XML parts (`ppt/slides/slideN.xml`, `ppt/notesSlides/notesSlideN.xml`), an
entirely different container/format — no existing gizza block parses it
(`document-text-extract` = PDF/DOCX/EPUB, `docx-text-extract` = `.docx` only,
`pdf-extract-text` = PDF). Not a duplicate.

## Scan method

One WebSearch ("pptx to text extract slide outline notes online tool"); skimmed
the product/feature pages of the tools below. Paraphrased feature lists only — no
competitor copy, branding, or trademarks reproduced.

## Competitors skimmed

1. **pptxEXT (Musashino Software)** — extracts slide text + speaker notes;
   output as plain text / Markdown / HTML / DOCX; "Extraction Target" selector
   (slides only / notes only / both); two layouts when both ("per page" keeps
   each slide's notes adjacent vs "all slides then all notes"); "don't output
   empty slides/notes" toggle; copy / save-as-file; local (no upload).
2. **Slide2Text (extractppttext.com)** — extracts text + speaker notes; output
   plain text (.txt) / Markdown (.md) / clipboard; "exclude hidden slides"
   checkbox; slide-view vs raw-text display; 150 MB upload cap; first-5-slides
   free preview (paid beyond).
3. **Sharayeh PowerPoint-to-Outline** — structured outline: slide titles → H1/H2,
   bullets → nested list items, each entry tagged with its source slide number;
   speaker notes as toggleable italic annotations per slide; export Markdown /
   DOCX / OPML / plain text / JSON; tables flattened to nested lists; charts →
   `[Chart: title]`, images → `[Image: alt]`; RTL support; bulk (Pro).
4. **SlidesPilot / AutoSlide / SlideSpeak (notes extractor)** (skimmed from search
   summaries) — same core shape: upload `.ppt`/`.pptx`, extract slide text and/or
   speaker notes, download as TXT or Markdown, free/no-account.

## Table-stakes → decision (fit to the gizza chat+CLI JSON model)

| Feature | Decision | Where |
|---|---|---|
| Extract slide text | in-model | core: all `<a:t>` runs, `<a:p>` = paragraph, `<a:br>` = line break |
| Extract speaker notes | in-model | `notes` enum (include/exclude/only), resolved via each slide's rels → `notesSlideN.xml` |
| Notes-target selector (slides only / notes only / both) | in-model | `notes` enum mirrors pptxEXT's "Extraction Target" |
| Per-slide outline (number, title, body) | in-model | `slides[]` array: `{number, title, text, notes, hidden}` in presentation reading order |
| Slide titles as headings | in-model | title = text of the shape whose `<p:ph>` type is `title`/`ctrTitle` |
| Source slide numbers | in-model | `slides[].number` (1-based, presentation order via `presentation.xml` `sldIdLst` + rels) |
| Reading order (not ZIP/alpha order) | in-model | `ppt/presentation.xml` `<p:sldIdLst>` + `ppt/_rels/presentation.xml.rels` (fallback: numeric `slideN.xml` sort) |
| Exclude hidden slides | in-model | `include_hidden` boolean (default true); reads `<p:sld show="0">` |
| Skip empty slides/notes | in-model (implicit) | `whitespace=clean` trims empties; empty slide `text` is `""`, empty notes `null` |
| Whitespace clean vs raw | in-model | `whitespace` enum (clean/raw), mirrors `ppt-to-text` |
| Table cell text | in-model | table cells are `<a:tbl>…<a:t>` — captured as ordinary paragraphs |
| Markdown / HTML / DOCX / OPML export | OUT of model | this is a chat+CLI JSON tool; the structured `slides[]` array + flat `text` give the consumer everything to render any of these formats itself |
| Charts → `[Chart: …]`, images → `[Image: alt]` placeholders | OUT of model | text-extraction scope; drawing/chart semantics not parsed |
| Client-side "no upload" / copy / save-as-file / preview | N/A | UX of a browser page; gizza `.pptx` extractors are no-page (chat + CLI), like `ppt-to-text`/`doc-to-text` |
| 150 MB uploads, bulk / multi-file | OUT of model | single-source input; capped at 16 MiB (sandbox memory budget) |
| RTL text | in-model (free) | Unicode text is extracted in document order; direction preserved |

## Descriptor (final)

- `url` ⊕ `ref` — the presentation source (`Input::Document`).
- `notes` — enum `include` (default) | `exclude` | `only`.
- `whitespace` — enum `clean` (default) | `raw`.
- `include_hidden` — boolean, default `true`.

Response (flat JSON the LLM reads directly): `text` (full plain text), `slides`
(the outline array), `slide_count`, `words`, `paragraphs`, `truncated`.

Every table-stake above lands in the descriptor or is explicitly listed
out-of-model — none dropped silently.
