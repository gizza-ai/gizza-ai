# tiff-to-pdf — competitor analysis (2026-09-05)

Scope: one web search for "online TIFF to PDF converter multi page TIFF PDF options free browser", then five reachable results were skimmed. This note paraphrases observed behaviour only; no competitor copy, branding, logos or trademarks are reused.

## Competitor profiles (paraphrased)

### 1. tiff-viewer.vercel.app — browser TIFF viewer/converter
- **Function:** drag-and-drop TIFF viewer with page navigation and a one-click PDF export.
- **Inputs:** local TIFF files; explicitly mentions multi-page viewing.
- **UX controls:** drag/drop file picker, page controls, export button.
- **Output:** downloadable PDF.
- **Not surfaced:** page-size, margins, colour conversion, rotation or page-range controls.

### 2. tiff2pdf.com — TIFF/PDF combiner
- **Function:** upload TIFF images and optionally existing PDFs, then output either separate PDFs or one combined document.
- **Inputs:** up to 20 files by upload/drop; TIFF and existing PDF are both accepted.
- **UX controls:** upload/clear, combine mode, individual-vs-merged output.
- **Output:** PDF downloads; page images may be automatically rotated, scaled and optimized.
- **Not surfaced:** deterministic per-page selection, explicit DPI override or precise PDF page geometry.

### 3. convertico.com/tiff-to-pdf — batch converter
- **Function:** converts single-page or multi-page TIFF files into PDFs.
- **Inputs:** local files; page says batch conversion is supported.
- **UX controls:** upload area plus page-size/quality-style conversion settings.
- **Output:** downloadable PDFs.
- **Not surfaced:** retaining 1-bit fax scans as 1-bit image XObjects or matrix-only rotation details.

### 4. pngtopdf.co/tiff-to-pdf — browser-local TIFF to PDF
- **Function:** turns TIFF scans, including multi-page files, into PDF inside the browser.
- **Inputs:** local TIFF files.
- **UX controls:** page options, combine-all-files toggle, downloadable single PDF or per-file bundle.
- **Output:** PDF or ZIP of PDFs.
- **Privacy note:** emphasizes browser-side processing.
- **Not surfaced:** selecting arbitrary pages inside one TIFF, explicit DPI correction or grayscale conversion for size.

### 5. convertio.co/tiff-pdf — hosted format converter
- **Function:** hosted TIFF-to-PDF conversion workflow.
- **Inputs:** uploaded files and cloud sources depending on the site surface.
- **UX controls:** upload queue and conversion/download flow.
- **Output:** downloadable PDF.
- **Out of model for this repo:** server-backed queueing/cloud-provider imports/accounts.

## Table stakes distilled

| Table stake | Seen in | Our fit |
| --- | --- | --- |
| Accept single- and multi-page TIFF | 1, 2, 3, 4, 5 | **built** — walks the TIFF IFD chain and writes one PDF page per selected TIFF page |
| Preserve page order | 1, 2, 4 | **built** — default converts every page in file order |
| PDF download/media result | all | **built** — returns an `application/pdf` media envelope |
| Page-size control | 3, 4 | **built** — `page_size=fit|a4|letter|legal|a3|tabloid` |
| Margins | 4-adjacent page options | **built** — `margin_pt` 0-144 points |
| Rotation / auto-orientation | 2 | **built** — `rotate=0|90|180|270`; fixed sheets also have `orientation=auto|portrait|landscape` |
| Page-range selection inside a multi-page TIFF | not consistently exposed | **built, ahead** — `pages` accepts `1-3`, `2,5`, `4-` |
| DPI correction for broken scan metadata | not consistently exposed | **built, ahead** — `dpi=0` reads tags, explicit DPI overrides bad/missing tags |
| Keep bilevel fax scans small | implied by TIFF/PDF use case | **built** — auto mode embeds bilevel as 1-bit DeviceGray with Flate compression |
| Browser page upload UI | 1, 4 | **out of model here** — this repo's standalone tool pages do not currently support source-file uploads for this file→PDF envelope class; existing comparable PDF/file blocks are chat+CLI only |
| Merge several independent TIFF/PDF files | 2, 4 | **out of model** — current block takes one source file; multi-source binary inputs and ZIP outputs are a separate surface |
| Hosted/cloud import queue | 5 | **out of model** — no server/accounts/cloud storage in this toolkit repo |

## Fit decisions

**Built as a chat+CLI file-input block, not a standalone page.** The faithful tool needs binary TIFF input and returns binary PDF output. Existing gizza file/PDF transforms in this shape are no-page blocks because the generic page runtime is built for scalar text/number controls or specialised ffmpeg media pages, not arbitrary source-file upload plus an `application/pdf` envelope. The CLI and chat surfaces can resolve `url`/`ref`, run the pure Rust converter, and return the media envelope.

**Pure Rust conversion is in model.** The implementation uses the `tiff` crate to read multi-IFD TIFFs, normalises each page to bilevel/gray/RGB samples, Flate-compresses each image XObject and writes a PDF with `lopdf`. No external renderer, browser engine or ffmpeg path is needed.

**Out-of-model competitor features are listed, not simulated.** Batch merging several independent uploads, cloud-provider import, account-backed queues, ZIP bundles and stateful browser upload UI are omitted because they require multi-file/server/page capabilities outside this block's current gizza model.
