# doc-to-text — competitor analysis (2026-07-10)

Tool: **doc-to-text** — extract readable plain text from **legacy Microsoft Word
`.doc` (Word 97–2003)** files. These are OLE2 / Compound File Binary (CFB)
containers holding the *Word Binary File Format* (a `WordDocument` stream + a
`0Table`/`1Table` stream with the piece table) — a completely different container
from the ZIP/OOXML `.docx`. All notes below are paraphrased from public product
pages and the published format documentation; no competitor copy, branding, or
trademark is reproduced.

## Scope vs. existing gizza blocks (not a duplicate)

- `docx-text-extract` and `document-text-extract` read the **ZIP-based `.docx`**
  (Office Open XML) — they reject a legacy binary `.doc` (its magic is
  `D0 CF 11 E0 …`, not `PK…`). Neither can parse the Word 97–2003 binary format.
- `pdf-extract-text` / `epub-to-markdown` cover other containers.
- So doc-to-text fills a real gap: the *old* binary `.doc` that predates the ZIP
  format. It is the file-input → text shape, so — like every other file-input
  block (pdf-extract-text, docx-text-extract, unzip, strings) — it is a
  **chat + CLI** tool with **no interactive web page** (a binary upload is not a
  page text field, and the page renderer has no file-upload control for pure
  blocks).

## Competitors surveyed (top representative online converters)

1. A general DOC→TXT online converter (upload a `.doc`, download stripped plain
   text; no account).
2. A "Word to Text" converter that internally normalises legacy `.doc` before
   extracting, advertising success on the large majority of legacy documents.
3. A multi-format cloud converter offering DOC→TXT among many pairs, with input
   from local file / URL / cloud drives.
4. An in-browser DOC/DOCX viewer that opens Word 97–2003 files and can export
   them to TXT/PDF.
5. Command-line / server batch DOC→TXT converters aimed at bulk pipelines.

## Table-stakes features and fit-to-model

| Capability | Competitors | gizza fit | Decision |
|---|---|---|---|
| Read the legacy binary `.doc` container (OLE2/CFB) | all | in-model (pure Rust `cfb`) | **built** |
| Extract the document body as clean plain text | all | in-model (piece-table walk) | **built** |
| Handle both 8-bit (Windows-1252 compressed) and 16-bit (UTF-16) text pieces | implicit | in-model | **built** |
| Map Word control marks (paragraph `0x0D`, line break `0x0B`, page break `0x0C`, cell `0x07`, field codes `0x13/0x14/0x15`) to readable text | implicit | in-model | **built** |
| Normalise whitespace / collapse blank runs (vs. keep raw structure) | some | in-model | **built (`whitespace` enum)** |
| Accept input by URL or by uploaded attachment reference | file / URL / drives | in-model (`resolve_source`, url⊕ref) | **built** |
| Report word/paragraph counts alongside the text | some viewers | in-model | **built (response fields)** |
| Preserve rich formatting / styles / fonts | some viewers | out-of-model (plain-text tool by design) | listed, not built |
| Reconstruct tables as grids / Markdown | a few | out-of-model here (legacy table geometry needs the full SPRM/PAP layout; `docx-text-extract` covers OOXML tables) | listed, not built |
| Convert `.doc` → `.docx`/PDF | several | out-of-model (no Word writer in the wasm runtime) | listed, not built |
| OCR image-only / scanned pages embedded in the doc | a few viewers | out-of-model (no ML/OCR loader in the wasmi runtime) | listed, not built |
| Google-Drive / Dropbox pickers | a few | out-of-model (no cloud-drive integration; URL fetch covers remote files) | listed, not built |

## Design decisions taken into the descriptor

- `Input::Document` — single-sources the `url` ⊕ `ref` one-of, matching the other
  file-input extractors.
- `whitespace` = `Param::enumv(["clean","raw"])`, default `clean`. `clean`
  collapses 3+ blank lines to one blank line and trims trailing spaces / outer
  whitespace (what most converters ship as the default "clean text" output);
  `raw` returns the extracted text with control marks mapped but no cosmetic
  collapsing, for callers that want to preserve the original paragraph spacing.
- Response returns `text`, `words`, `paragraphs`, `pieces`, and a `truncated`
  flag (1 MB output cap), mirroring the structured responses of the sibling
  extractors so the LLM can cite counts directly.

No competitor copy or branding was used; out-of-model rows are enumerated here,
not implemented.
