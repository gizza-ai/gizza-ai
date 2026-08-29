# markdown-deck-to-pdf — competitor analysis (2026-08-30)

Scan run **before** implementing, per `create-next-tool` step 4. All findings are **paraphrased**;
no competitor copy, branding, theme names, or trademarks are reproduced or shipped.

## Scope check (why this is not a duplicate)

`ls blocks/ | grep -iE 'markdown|slide|deck|pdf'` surfaced three near neighbours; all were read
before deciding to build:

| Existing block | What it actually does | Overlap |
| --- | --- | --- |
| `markdown-to-pdf` | Flows a Markdown **document** onto portrait Letter/A4 pages; content spills across pages continuously. No slide concept, no `---` splitting, no aspect ratio, no theme. | Different output shape |
| `markdown-to-slides` | Emits a self-contained **HTML** deck (browser-rendered, light/dark). No PDF. | Different format |
| `markdown-to-pptx` | Emits a binary **OOXML `.pptx`** (editable in PowerPoint/Keynote). No PDF. | Different format |

The gap is the one thing the family is missing: a **fixed, paginated PDF of a slide deck — one
slide per page** — which is what people actually attach, print, and hand out. Same relationship
`markdown-to-pdf` / `markdown-to-docx` / `markdown-to-latex` already have to each other in this
repo (same input, different target format). Built.

## Competitors reviewed

1. **Marp / Marpit / Marp CLI** (marp.app, marpit docs, marp-cli README) — the reference
   implementation of "Markdown in, slide PDF out". Reachable, documented.
2. **Slidev** (sli.dev export guide) — developer-focused deck framework with a PDF export path.
   Reachable, documented.
3. **md-converter.com** — browser-local Markdown→PDF/slides converter (Marp-backed slide mode).
   Reachable; marketing page only, option list not published, so only its advertised control
   *categories* are usable.

(A fourth candidate, markdowntoolsonline.com, returned HTTP 403 and was replaced by Slidev.
SlideSpeak appeared in search but is an account-gated AI service — out of model by construction.)

## Table stakes → decisions

| Capability | Seen in | Verdict | Where it landed |
| --- | --- | --- | --- |
| `---` thematic break splits slides | Marp, Slidev, md-converter | **in-model** | Always splits, at any `split_level` |
| Auto-split at a heading level (Marp's heading-divider idea) | Marp, md-converter ("heading-based splitting") | **in-model** | `split_level` = `h1` / `h2` / `both` / `none` |
| Slide aspect ratio / paper choice | Marp (deck size), md-converter (paper control) | **in-model** | `slide_size` = `16:9` / `4:3` / `a4-landscape` / `letter-landscape` |
| Light + dark presentation themes | Marp (3 bundled themes), Slidev (`--dark`) | **in-model** | `theme` = `light` / `dark` (own colors, own names) |
| Slide numbering toggle | Marp (pagination directive, off by default) | **in-model** | `page_numbers`, default **on** — a handout PDF is the whole point here |
| Repeated header / footer text | Marp (header/footer directives) | **in-model** | `header`, `footer` (free text, every slide) |
| Title / cover slide | Marp, Slidev, md-converter ("covers") | **in-model** | `title` → centered cover page |
| PDF outline / bookmarks per slide | Marp CLI (`--pdf-outlines`) | **in-model** | `outline`, default on — one bookmark per slide |
| Body text scale | Marp (auto-scaling), typical converters | **in-model** | `font_size` (8–48 pt) **plus** automatic shrink-to-fit per slide |
| Slide-range export (`--range 1,6-8`) | Slidev | **considered, rejected** | Schema bloat for a tool whose output is already a PDF you can page-extract; `pdf-split` covers it |
| Speaker notes as PDF annotations | Marp CLI (`--pdf-notes`) | **considered, rejected** | Notes have no agreed Markdown syntax; would need a private convention |
| Custom CSS / user themes | Marp, Slidev | **out-of-model** | Needs a CSS engine + font embedding; base-14 PDF fonts only |
| Background images, embedded images | Marp, Slidev | **out-of-model** | No file/network access from a text-in tool; alt text is rendered instead |
| Math typesetting (KaTeX/MathJax) | Marp, Slidev | **out-of-model** | Needs a formula layout engine |
| Per-slide inline directives / classes | Marp | **out-of-model** | Would be a private DSL with no portable meaning |
| Live editor with drag-and-drop page breaks | md-converter | **out-of-model** | Editor product, not a single-shot converter |
| Click-animation steps as extra pages | Slidev | **out-of-model** | No animation model in Markdown input |
| AI-inferred slide boundaries | SlideSpeak | **out-of-model** | Account + server model |

## UX control patterns adopted

- Long Markdown field is a **textarea** (`multiline = true`) so pasted decks keep newlines.
- Every fixed-choice param is `Param::enumv` → renders as a `<select>`; `[input.labels]` gives the
  options human labels while the deep-link values stay canonical.
- `font_size` uses `kind = "slider"` (bounded 8–48 pt range) with the canonical number box mirrored.
- Competitors ship presets/covers, so the page ships **`[[example]]` preset chips**: a widescreen
  team update, a dark 4:3 deck split on `##`, and a `---`-separated deck with header/footer.
- Binary output → `page/custom.js` renders a real **Download PDF** button (same pattern as
  `markdown-to-pptx`), not a base64 blob in the output box.

## Stated limits (on the page, not just in code)

- Base-14 PDF fonts, WinAnsi/Latin-1 text; characters outside Latin-1 render as `?`.
- A `---`/`***`/`___` line on its own is *always* a slide break, so it cannot be used as a setext
  heading underline.
- A slide whose content still overflows after the automatic shrink-to-fit continues onto extra
  pages rather than silently dropping content.
- 8 MB output cap; images are rendered as their alt text.
