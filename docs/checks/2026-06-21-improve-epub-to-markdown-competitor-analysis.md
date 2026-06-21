# epub-to-markdown — competitor analysis & differentiation

**Tool:** `gizza-ai/epub-to-markdown` — extract an EPUB's chapters into clean
Markdown (or plain text), in reading order.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| Calibre (`ebook-convert in.epub out.txt/.md`) | Desktop app / CLI | The reference, but a heavyweight GUI install; Markdown output is not a first-class target (goes via its own intermediate formats); overkill for "just give me the text". |
| Pandoc (`pandoc in.epub -t markdown`) | CLI | Good, but needs a Haskell install and produces very verbose Markdown with media/attribute clutter. |
| Online "EPUB to TXT/Markdown" converters | Web | Most **upload the book to a server**; many ignore the spine and dump files alphabetically (wrong chapter order) or include nav/toc cruft. |
| `epub2txt`, ad-hoc unzip + grep | CLI/scripts | Unzipping gives you raw XHTML in arbitrary order; you still have to find the OPF, read the spine, and strip tags yourself. |

## How gizza's tool is better / different

1. **Correct reading order.** It parses the EPUB's `META-INF/container.xml` →
   OPF **spine**, so chapters come out in the author's intended order — not the
   ZIP's alphabetical order (the most common bug in naive converters).
2. **Clean Markdown *or* plain text.** Markdown (via `htmd`) keeps headings,
   lists, and links; text mode (`nanohtml2text`) gives a plain reading copy.
   Chapters are separated by a Markdown horizontal rule.
3. **Runs locally.** Chat service worker or CLI, all WASM — the book is never
   uploaded. Most web converters can't say that.
4. **Returns structure, not just a blob.** Title, chapter count, character count,
   and a truncation flag — easy to act on programmatically or in chat.
5. **No install.** `gizza tool epub-to-markdown --json '{"url":"…"}'` or ask in
   chat; no Calibre/Pandoc/Haskell toolchain.

## Verification

Run against Project Gutenberg's *Pride and Prejudice* EPUB
(`cache/epub/1342/pg1342.epub`): correctly reported the title "Pride and
Prejudice", extracted 16 spine documents in order, and produced ~747 K
characters of Markdown with chapter rules.

## Surfaces & honest scope

- **Chat + CLI only — no web page.** An EPUB is a binary file input, which the
  page framework only supports under the ffmpeg media runtime (and the output
  here is text, not media). Same no-page file-input pattern as
  `pdf-extract-text` / `detect-file-type`.
- Front-matter/nav and Gutenberg boilerplate appear as their own "chapters"
  because they are separate spine documents — faithful to the book's structure.

## Possible future enhancements

- Optional per-chapter output (array of {title, markdown}) instead of one blob.
- Strip nav/cover documents heuristically, or expose a "skip front-matter" flag.
- Pull richer metadata (author, language, publisher) from the OPF.
