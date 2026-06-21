# markdown-render — competitor analysis (2026-06-21)

Tool: `blocks/markdown-render` — renders Markdown (CommonMark + GitHub-flavoured
extensions) into HTML via the pure-Rust `pulldown-cmark` crate, then **sanitizes**
the result with `ammonia` (html5ever-based) so the output is safe to embed. Single
text input (`markdown`) → single HTML-text output. Runs fully client-side (WASM on
the page, wafer sandbox in chat); zero server, zero upload.

## Surfaces verified

- **Chat / LLM API** — `wafer build` validates `target/block.wasm` instantiates;
  drift-guard test (`schema_json_matches_authored_chat_schema`) passes.
- **CLI** — `gizza tool markdown-render markdown='…'` renders headings, bold,
  links, GFM table, task-list checkboxes, and strikethrough correctly.
- **Page** — Playwright `tool-page-markdown-render.spec.ts` (2 tests) passes:
  field render + query-param deep-link, including a GFM table.

## Top 5 competitors

| Tool | URL | Notable features |
|---|---|---|
| StackEdit | stackedit.io | Full editor; GFM + Markdown Extra; footnotes; highlight.js; live scroll-sync preview; KaTeX/UML; OAuth cloud sync |
| Dillinger | dillinger.io | Monaco editor; GFM; syntax highlighting; live preview; export MD/HTML/PDF; OAuth sync |
| markdowntohtml.com | markdowntohtml.com | marked + highlight.js; GFM; tabbed Preview vs raw HTML source; copy + download; `sanitize:false` (raw HTML passes through) |
| Markdown Live Preview | markdownlivepreview.com | marked + DOMPurify; GFM; split editor live preview; Mermaid; light/dark theme; sanitized |
| Marked.js demo | marked.js.org/demo | GFM; output selector preview / HTML source / token tree; editable options panel; permalink; `sanitize` removed (raw HTML passes through) |

(Also surveyed: CodeBeautify markdown-to-html — paste/upload/load-by-URL, copy,
download; Browserling — one-button converter with `?input=` param.)

## Where markdown-render already leads / matches

- **Footnotes** — among the five, only StackEdit supports them; the three
  lightweight marked-based converters lack them. We enable footnotes by default.
- **Safe by default** — output is sanitized with `ammonia` (scripts, event
  handlers, and dangerous URL schemes such as `javascript:` are stripped), so the
  HTML is safe to embed. This is structurally safer than the `sanitize:false`
  tools (markdowntohtml.com, Marked demo) which pass raw HTML straight through.
- **GFM parity** — tables, task lists (disabled checkboxes preserved),
  strikethrough, and smart punctuation are all enabled, matching the marked-based
  converters.
- **Zero-server / no-upload** — true client-side WASM with no server path at all,
  unlike CodeBeautify's load-by-URL and the OAuth-sync editors.

## Gaps and disposition

### In-model and present

- GFM tables, task lists, strikethrough, footnotes, fenced code blocks
  (`<code class="language-…">`), smart punctuation, and ammonia sanitization are
  all enabled in the core.

### Deferred — page-driver / UX features, not single-tool scope

These are real competitor features but live in the shared site page driver
(`site/tool.js`), not in a single tool's core, so they are out of scope for this
tool's build (they would change every tool's page, not just this one):

- Copy-to-clipboard button, download-HTML button, load-`.md`-file, sample loader.
- Rendered-preview-vs-HTML-source tabbed view; live split preview pane.
- Light/dark theme toggle; permalink (the page already supports `?markdown=` deep
  links, which covers the share-by-URL case).
- Per-extension/parser option toggles (an options panel) — a page-driver UI change.

### Out of model (no server / no account / non-HTML output)

- Cloud sync / OAuth storage (GitHub, Drive, Dropbox, OneDrive, Bitbucket) — needs
  a server + accounts.
- PDF / Word export — needs a rendering/print backend; beyond single HTML-text output.
- Load-from-URL fetch — needs a CORS-proxy server.
- WYSIWYG rich-text editing toolbar — a different interaction model.
- Syntax-highlighted code blocks (highlight.js colours) — pulldown-cmark already
  emits `language-…` classes, but actually colouring them needs CSS/JS theming in
  the page driver; deferred as a page-driver concern.
- LaTeX/KaTeX math, Mermaid/UML diagrams — need extra client libs and a richer
  output than the single sanitized-HTML contract.

No competitor copy, branding, or trademarks were used.
