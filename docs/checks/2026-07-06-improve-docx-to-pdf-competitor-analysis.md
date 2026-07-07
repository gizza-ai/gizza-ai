# docx-to-pdf — competitor analysis (2026-07-06)

Tool: convert an uploaded/linked Microsoft Word **`.docx`** document into a
clean, paginated **PDF** (US Letter default, or A4). Chat + CLI block, **no
page** — a binary file input with binary (PDF) output is the "no-page
file-input" pattern, like `pdf-to-epub` / `markdown-to-pdf` / `images-to-pdf`.

Pure-Rust: `zip` opens the `.docx` container, `quick-xml` streams the
`word/document.xml` WordprocessingML body, and `lopdf` writes the PDF with the
built-in base-14 Helvetica fonts (no font embedding). Runs on every backend
including the chat Service Worker.

## Scan

One `WebSearch` ("convert docx to PDF online converter Word document features")
plus a review of the top reachable competitor tool pages. All paraphrased — no
copy, branding, or trademarks reproduced.

1. **Adobe Acrobat — Word to PDF** (`adobe.com/acrobat/online/word-to-pdf`).
   Input: `.doc`/`.docx`. Output: a single PDF. Headline: faithful preservation
   of fonts, images, tables, and alignment. UX is zero-config: drag-drop / file
   picker, sign-in unlocks more. No page-size/margin knobs exposed.
2. **iLovePDF — Word to PDF** (`ilovepdf.com/word_to_pdf`). Input: one or many
   Word files (batch), plus Google Drive / Dropbox import. Output: a PDF per file
   (or a zip). Preserves layout/formatting. No typographic knobs; the value-add
   is batch + cloud import.
3. **Smallpdf — Word to PDF** (`smallpdf.com/word-to-pdf`). Input: DOCX/DOC via
   drag-drop. Output: a PDF. Preserves formatting; part of a larger tool suite
   (compress/merge afterward). Privacy pitch: TLS in transit, files auto-deleted
   after ~1h. No layout options.
4. (Cross-checked: CloudConvert `docx-to-pdf` is the most configurable — it
   exposes an engine choice and options like page range — but is aimed at
   developers/API users, not a zero-config consumer flow.)

Common denominator: a **zero-config, upload-and-download** experience whose whole
promise is *fidelity of formatting*. None expose typographic controls to a normal
user; the differentiators are batch, cloud import, and privacy.

## Table-stakes → in-model / out-of-model (every one accounted for)

| Competitor table-stake | Decision | Where it lands |
| --- | --- | --- |
| Upload a `.docx`, download a PDF | **in-model** | `Input::Document` (url⊕ref) → `application/pdf` envelope |
| Preserve headings / title styles | **in-model** | `w:pStyle` → scaled + bold heading sizes |
| Preserve bold / italic | **in-model** | `w:b` / `w:i` → Helvetica-Bold/Oblique/BoldOblique |
| Preserve explicit font sizes | **in-model** | `w:sz` (half-points) honored per run |
| Preserve paragraph alignment | **in-model** | `w:jc` → left / center / right (justify → left-aligned) |
| Preserve lists (bulleted/numbered) | **in-model (simplified)** | `w:numPr`/`w:ilvl` → bullet marker, indented by level |
| Preserve hard + page breaks | **in-model** | `<w:br/>` line break, `<w:br w:type="page"/>` new page |
| Preserve tables | **in-model (simplified)** | flattened to readable pipe-separated rows + header rule |
| Choose page size (Letter/A4) | **in-model** | `page_size` enum (we expose it; most competitors don't) |
| Set margins | **in-model** | `margin` param in points (bonus over competitors) |
| Page numbers footer | **in-model** | `page_numbers` boolean (bonus over competitors) |
| Local / private processing | **in-model (stronger)** | pure-Rust, runs on-device — the document never leaves the browser |
| Embed images / drawings | **out (listed limitation)** | feasible via lopdf XObjects but heavy (positioning/anchoring); deferred for a lightweight converter — stated in the tool description |
| Font embedding (exact fonts) | **out (listed limitation)** | remaps to base-14 Helvetica family; keeps output tiny/deterministic, no arbitrary font files |
| Batch / multi-file conversion | **out** | one file per call (chat/CLI shape) — a workflow concern, not a converter feature |
| Cloud import (Drive/Dropbox) | **out** | url⊕ref only; not part of gizza's I/O model |
| Legacy `.doc` (binary Word 97-2003) | **out (listed limitation)** | only OOXML `.docx` (a ZIP of XML); legacy `.doc` is a different binary format |
| RTF / ODT / TXT input | **out** | separate formats; `text-to-pdf` / `markdown-to-pdf` cover neighboring cases |

No table-stake was dropped silently: every one above is either in the descriptor
or explicitly listed as a limitation (and surfaced in the tool description). None
of the competitors ship sliders / color pickers / preset chips — the flow is
zero-config drag-drop — so there is no UX-control gap to match (and, being a
no-page block, there is no page form to render anyway).

## Verification (end-to-end)

- 12 core unit tests (in-memory `.docx` fixtures): simple paragraph, bold/italic
  + heading styles, explicit `<w:br w:type="page"/>` → 2 pages, 400-paragraph
  pagination, table parse + render, center alignment + explicit `w:sz`, list
  levels, XML entity decoding, A4 vs Letter MediaBox, page-number footer, empty
  body, and the error matrix (empty / non-zip / zip-without-`word/document.xml` /
  font-size and margin bounds / unknown page size). 4 block tests incl. the
  drift-guard schema.
- CLI, real Microsoft Word file (calibre `demo.docx`, a real-world typographic
  demo): default → **7-page** PDF at **612×792 pts (Letter)**; `page_size=a4
  page_numbers=true` → **6-page** PDF at **595×842 pts (A4)**; extracted text
  (`pdftotext`) reads correctly ("Demonstration of DOCX support…"). `font_size=48`
  (cap) succeeds; `font_size=49` errors ("must be between 6 and 48 points").
  Non-`.docx` URLs fail gracefully (`application/*` content-type guard).
