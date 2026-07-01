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

## FAQ

<details>
<summary>How does the tool decide which heading level to split on?</summary>

It scans the whole document and splits at the **smallest heading level
actually present**. A document with `#` headings splits on every `#`; a
document that only ever uses `###` splits on every `###`. HTML works the same
way with `<h1>`–`<h6>` tags. You can't force a deeper split level directly —
but you can run a single extracted section back through the tool.

</details>

<details>
<summary>What happens to content before the first heading?</summary>

It isn't dropped: any preamble becomes its own section titled `intro`, with a
filename like `01-intro.md`, so the split output always reassembles into the
full original document.

</details>

<details>
<summary>Can two sections with the same title overwrite each other?</summary>

No. Filenames are built as `NN-slug.ext` (numbered in document order), and if
two sections would still slugify to the same name the later one gets a numeric
suffix — so every section is guaranteed a unique filename.

</details>

<details>
<summary>Does it understand headings inside code blocks?</summary>

Not specially — a line starting with `# ` inside a fenced code block (say a
shell comment) is treated like any other Markdown heading and will start a new
section. If your document contains such code, split it as HTML after
rendering, or adjust the comment lines first.

</details>
