# document-splitter — competitor analysis & differentiation

**Tool:** `gizza-ai/document-splitter` — split a long Markdown or HTML document
into separate files, one per top-level section.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `csplit` / `awk` one-liners | CLI scripts | Powerful but require crafting a regex per document; no slugged filenames, no preamble handling, steep for non-devs. |
| Pandoc `--split-level` / `--chunk-template` | CLI | Heavy install (Haskell), aimed at format conversion; splitting is a side feature and config-heavy. |
| `remark`/`unified` + custom JS plugin | Library | Must write code; only Markdown; no HTML mode out of the box. |
| Obsidian "Note Splitter" / similar plugins | App plugin | Locked to one app; not scriptable; Markdown only. |
| Various "split text file online" sites | Web | Split by line count or size, **not by heading** — they don't understand document structure, and most upload your file to a server. |

## How gizza's tool is better / different

1. **Structure-aware, not size-aware.** Splits on the *smallest heading level
   actually present* (`#`, or `##` if there's no `#`; `<h1>`…`<h6>` for HTML),
   so it matches how the document is really organized — unlike line/byte
   splitters.
2. **Two formats, one tool.** Markdown *and* HTML in the same UI, selected by a
   dropdown.
3. **No content lost.** Text before the first heading becomes an explicit
   `intro` section rather than being dropped.
4. **Ready-to-save filenames.** Each section gets a numbered, slugified filename
   (`01-introduction.md`) that sorts in document order and never collides, even
   for duplicate headings.
5. **Three surfaces, zero upload.** Chat tool, CLI (`gizza tool
   document-splitter`), and a browser page — all running the same Rust/WASM core
   locally. Nothing is sent to a server.

## Possible future enhancements

- Optional explicit heading-level selector (force split at H2 even if H1 exists).
- ZIP-of-files output for the page surface (would require the file-output
  envelope rather than text preview).
- Front-matter / first-paragraph-based title override.
