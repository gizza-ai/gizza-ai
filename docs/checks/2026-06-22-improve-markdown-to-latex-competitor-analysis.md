# markdown-to-latex — competitor analysis (2026-06-22)

Snapshot taken while building and improving the `markdown-to-latex` tool
(new tool from the backlog). All competitor notes are paraphrased — no copy,
branding, or assets were reproduced.

## What the tool does

Converts a Markdown document into LaTeX source, entirely in the browser
(WebAssembly, no upload, no account). Output is a body fragment by default, or a
full compilable `article` document when `full_document=true`. A `heading_offset`
(0–5) demotes headings for pasting into an existing document.

## Competitors surveyed

1. **Pandoc** (`pandoc -f markdown -t latex`, and the in-browser `try pandoc`) —
   the de-facto reference universal converter. Huge feature surface: tables,
   definition lists, metadata blocks, footnotes, citations/bibliographies (CSL),
   math, raw LaTeX passthrough, custom templates and filters. CLI / library /
   small web playground.
2. **markdownlivepreview.dev "Markdown to PDF (with LaTeX math)"** — browser tool
   that renders Markdown to PDF via a LaTeX-math pipeline. Emphasis on math
   rendering and a styled blockquote/PDF output; not a raw `.tex` exporter.
3. **innateblogger "Markdown to PDF"** — browser converter; GFM features
   (strikethrough, task lists), blockquotes with a left border, horizontal rules.
4. **Overleaf "Writing Markdown in LaTeX Documents"** (the `markdown` package) —
   embeds Markdown inside a `.tex` file at compile time rather than emitting
   `.tex`; supports a configurable subset incl. footnotes and definition lists.
5. **MyST / mystmd typography** — Markdown-superset for technical writing that
   targets LaTeX/PDF; rich directive/role syntax (admonitions, cross-refs),
   footnotes, definition lists, strikethrough.

(Five real comparators found; Pandoc dominates the category and sets the
feature bar.)

## Gap diff vs our tool (at first build) and disposition

| Capability | Competitors | Ours (initial) | Action |
|---|---|---|---|
| Headings (ATX) → sectioning | all | yes | — |
| Setext headings (`===`/`---`) | Pandoc, MyST | **no** | **Added** (in-model) |
| Bold / italic / inline code | all | yes | — |
| Strikethrough `~~…~~` | Pandoc(+gfm), innateblogger, MyST | **no** (escaped tildes) | **Added** → `\sout` (ulem) |
| Footnotes `[^id]` | Pandoc, Overleaf, MyST | **no** | **Added** → `\footnote{…}` |
| Ordered/unordered/nested lists | all | yes | — |
| Task lists `- [x]` | gfm tools | yes | — |
| Pipe tables + alignment | Pandoc, gfm | yes (booktabs) | — |
| Fenced + indented code | all | yes (listings/verbatim) | — |
| Blockquotes | all | yes | — |
| Links / images / autolinks | all | yes (hyperref/graphicx) | — |
| Inline/display math passthrough | Pandoc, mdlivepreview, MyST | yes | — |
| Special-char escaping | all | yes (10 TeX specials) | — |
| Full standalone document toggle | Pandoc (templates) | yes (`full_document`) | — |
| Heading offset / demote | Pandoc (`--shift-heading-level-by`) | yes (`heading_offset`) | — |

### In-model gaps closed in this build

- **Strikethrough** `~~text~~` → `\sout{...}`, with the `ulem` package added to
  the `full_document` preamble (`\usepackage[normalem]{ulem}`).
- **Footnotes** — reference-style `[^id]` in the body + `[^id]: definition`
  lines become inline `\footnote{...}` (definition text is itself inline-parsed;
  unmatched references degrade to literal text; definition lines are stripped
  from the body).
- **Setext headings** — a text line underlined by `===` (level 1) or `---`
  (level 2) maps to `\section` / `\subsection`, without breaking the existing
  `---` thematic-break handling (a `---` after a blank line is still a rule).

Each shipped with its own unit test (`strikethrough`,
`footnote_reference_and_definition`, `footnote_with_inline_formatting_in_definition`,
`unresolved_footnote_ref_is_literal`, `setext_headings`,
`setext_does_not_eat_thematic_break`, `full_document_includes_ulem_for_strikethrough`)
plus a combined Playwright page test.

## Considered, NOT built (out-of-model or out-of-scope)

- **Citations / bibliographies (CSL, `\cite`)** — needs a bibliography database
  and CSL style data; a server/data-backed feature, out of the browser-local,
  no-account model.
- **Definition lists** — Pandoc-style `Term\n: definition`; deferred (lower
  demand than footnotes/strikethrough; can be a follow-up). Listed here so it
  isn't lost.
- **PDF rendering** — competitors that output PDF run a TeX engine; gizza already
  has separate `markdown-to-pdf` / `text-to-pdf` tools, and a full TeX engine is
  not browser-local-feasible here. This tool intentionally emits `.tex` source.
- **Custom document templates / metadata YAML front-matter** — Pandoc templates
  are a large configuration surface; out of scope for a focused converter.
- **Raw inline HTML passthrough** — Markdown allows embedded HTML; LaTeX has no
  general HTML equivalent, so it's escaped as prose rather than translated.

## Surfaces verified (2026-06-22)

- **Chat / LLM API:** `cargo test --workspace` (29 core tests + drift-guard schema
  test compiled under wasm); `wafer build` validates+instantiates the block;
  `wafer test` fixtures (`convert.json`, `full-document.json`) pass.
- **CLI:** `gizza tool markdown-to-latex …` — headings, inline formatting, lists,
  tables, `full_document`, `heading_offset`, strikethrough, footnotes, and setext
  all confirmed.
- **Page (incl. query-param deep-link):** 5 Playwright tests pass against
  `/tools/markdown-to-latex/`.

No copied copy, branding, or assets. All output copy and code are original.
