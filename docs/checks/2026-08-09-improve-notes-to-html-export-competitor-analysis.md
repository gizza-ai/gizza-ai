# notes-to-html-export — competitor analysis (2026-08-09)

Scan run **before** implementation, per the create-next-tool recipe. All findings are
**paraphrased observations** of what each tool does — no competitor copy, branding, or
trademarked text is reproduced or reused. Out-of-model items are *listed*, not built.

Tool under construction: bundle a set of Markdown notes into ONE styled, self-contained
HTML page with a table of contents.

## Competitors reviewed

| # | Tool | Shape | What it does for this job |
|---|------|-------|---------------------------|
| 1 | Pandoc (`pandoc -s --toc`) | CLI | Stitches several Markdown files into one standalone HTML document; `-s` adds the full HTML boilerplate + embedded default stylesheet, `--toc` inserts a generated, linked table of contents, `--toc-depth=N` bounds it (docs default 3), `--number-sections` numbers headings, `--metadata title=…` sets the document title, `--template` swaps the whole wrapper. Headings get auto-generated `id` anchors the TOC links to. |
| 2 | markdown-to-standalone-html (EdJoPaTo, Rust CLI) | CLI | One Markdown file → one self-contained HTML with a built-in simple CSS template, a table of contents, and code-block syntax highlighting; inlines external assets (images/CSS) by default via an external helper binary, with a flag to turn inlining off, and a custom-template option. Explicitly frames "self-contained" as PDF-like portability. |
| 3 | Markdown Monster — "Packaged HTML File" export | Desktop app | Export modes: raw HTML fragment, *packaged* single HTML with images/CSS/fonts inlined, HTML plus loose asset files, or a zip bundle. The packaged file is the share-by-email/one-file story; output styling follows the app's current preview theme. TOC is not part of the export step. |
| 4 | Obsidian "Notes to HTML Pages" plugin | Note-app plugin | Exports one note *or a whole folder* of notes as self-contained HTML with no network dependency; clickable table of contents, responsive navigation, styled tables/callouts/code blocks, a choice of two reading styles (themes), plus custom reading font and text size. Closest analogue to this row's "set of notes → one shareable page". |
| 5 | Joplin "Copy as HTML" plugin | Note-app plugin | Converts a note to HTML for pasting elsewhere and can embed images as base64 so the HTML survives outside the app. Single note, no TOC (Joplin's TOC is a separate in-app side panel, not an export feature). |

Also seen but not counted as direct competitors: browser Markdown→HTML converters
(single file, fragment output, no TOC, no multi-note bundling) and BitDownTOC-style
generators that emit a Markdown TOC only — that job is already covered here by
`blocks/toc-generator`.

## Duplicate check (why this is a distinct block)

- `blocks/markdown-render` — one Markdown string → sanitized HTML **fragment**. No document
  wrapper, no CSS, no TOC, no multi-note bundling.
- `blocks/markdown-to-slides` — one Markdown doc → a self-contained **slide deck** (paged,
  keyboard/swipe navigation). Different output shape; no TOC, no per-note sectioning.
- `blocks/toc-generator` — emits a TOC **by itself** (Markdown or HTML list), does not render
  or wrap the document.
- `blocks/html-preview-bundler` — bundles hand-written HTML+CSS+JS; takes no Markdown.

This tool is the multi-note *document* export: split a pasted pile of notes into sections,
render + sanitize each, generate a linked TOC over their headings, and wrap the whole thing in
one themed, dependency-free HTML file. None of the four covers that.

## Table-stakes → decisions

| Table stake (seen at) | Decision |
|---|---|
| Many notes → one document (1, 4) | **In-model, built.** `notes` is one pasted body; `split` chooses the boundary convention — `heading` (every level-1 `#` starts a new note, i.e. plain concatenation of note files) or `hr` (a `---`/`***`/`___` thematic break between notes). |
| Linked table of contents (1, 2, 4) | **In-model, built.** GitHub-style slug anchors on every heading, deduped (`intro`, `intro-1`, …), nested list. |
| TOC depth limit (1, `--toc-depth`) | **In-model, built.** `toc_depth` 1–6, default 3 (matches Pandoc's documented default), rendered as a slider. |
| Section numbering (1, `--number-sections`) | **In-model, built.** `number_sections` checkbox; numbers both the headings and the TOC entries. |
| TOC placement / responsive nav (1 inline, 4 sidebar) | **In-model, built.** `toc` = `sidebar` (sticky column on wide screens, collapses above the content on narrow ones), `top` (inline block before the notes), or `none`. |
| Document title metadata (1, `--metadata title`) | **In-model, built.** `title` param; also used as the visible page heading, defaults to "Notes". |
| Theme / reading style (2, 3, 4) | **In-model, built.** `theme` = `light`, `dark`, or `auto` (follows the reader's OS setting via a media query). |
| Truly self-contained output (1, 2, 3, 4) | **In-model, built.** Single `<!doctype html>` string with all CSS embedded, zero external requests, no JS required to read it. |
| Sanitized output (implicit) | **In-model, built.** Rendered Markdown is sanitized (scripts/handlers stripped) before embedding — the export is meant to be shared. |
| Rich Markdown: tables, task lists, strikethrough, footnotes (1, 4) | **In-model, built.** CommonMark + GitHub-flavored extensions. |
| Preset "one-click" configurations (common UX pattern) | **In-model, built.** `[[example]]` chips on the page for a sidebar/dark/numbered handbook and a plain top-TOC export. |

## Considered, NOT built (out of model)

- **Reading multiple real files from disk / a folder picker** (1, 4). Gizza blocks take typed
  params, and the page is a single form — notes arrive as one pasted body with an explicit
  split convention instead. A multi-file upload surface does not exist in this model.
- **Inlining remote images as base64** (2, 3, 5). Requires fetching each `![](https://…)`;
  a pure block has no network. Images already written as `data:` URIs pass through intact,
  and that limit is stated on the page.
- **Language-aware code syntax highlighting** (1, 2). Needs a bundled highlighter (a JS
  library or a syntect-class grammar set), which would dwarf the block and its page wasm.
  Code blocks are styled monospace with the language preserved as a plain class-free label
  in the source; no colorized tokens.
- **Custom HTML template / arbitrary CSS override** (1, 2). Accepting raw user CSS/templates
  into a shared, sanitized export widens the injection surface for a file whose whole point
  is being safe to hand to someone else; the three built-in themes cover the need.
- **PDF output** (common adjacent ask) — already `blocks/markdown-to-pdf`.
- **Custom reading font + text size** (4). App-level reader preferences; in a one-shot export
  they would be baked in permanently for every reader. Rejected on judgment (schema bloat for
  a preference the reader's own browser zoom already handles), not on feasibility.

## Copy / SEO notes (original wording only)

Search intent clusters around "export markdown notes to html", "combine markdown files into
one html", "markdown to standalone html", "self-contained html with table of contents". The
page copy targets those phrasings in our own words, shows one worked input→output example,
and states the limits above (no remote image inlining, no syntax colorizing, notes split by
heading or thematic break).
