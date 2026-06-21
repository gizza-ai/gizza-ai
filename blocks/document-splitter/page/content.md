## About this tool

**Document Splitter** breaks a long Markdown or HTML document into separate
files — one per top-level section. Paste a document, pick its format, and get
back a suggested filename, title, and content for every section.

- **Markdown** splits at the smallest heading level present. If your document
  uses `#` for top-level headings, it splits on every `#`; if it only uses `##`,
  it splits on every `##`.
- **HTML** splits at the smallest `<hN>` tag present (e.g. every `<h1>`).
- Any text **before the first heading** becomes an `intro` section, so nothing
  is lost.
- Filenames are numbered and slugified from each heading (e.g.
  `01-introduction.md`), so they sort in document order and never collide.

Everything runs **locally in your browser** via WebAssembly — your document is
never uploaded.

### Common uses

- Break a book chapter, spec, or README into per-section pages for a static site.
- Turn one long Markdown export into individual notes.
- Chunk documentation ahead of importing it into a wiki or CMS.
