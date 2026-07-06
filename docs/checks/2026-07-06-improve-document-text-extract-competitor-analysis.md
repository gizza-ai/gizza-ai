# document-text-extract — competitor analysis (2026-07-06)

Tool: extract searchable plain text from an uploaded/linked **PDF, DOCX, or EPUB**
document, auto-detecting the format from the file's magic bytes. Chat + CLI block
(no page — a binary file input with text output is the "no-page file-input"
pattern, like `pdf-extract-text` / `epub-to-markdown` / `web-fetch`).

## Scan

One `WebSearch` ("document text extractor online tool PDF Word EPUB features")
plus fetches of the top 3 reachable competitor tool pages. All paraphrased — no
copy, branding, or trademarks reproduced.

1. **PDF2Go — PDF to Text** (`pdf2go.com/pdf-to-text`). Input: PDF only. Output:
   plain .txt. Headline feature is OCR with several quality tiers (standard /
   AI-OCR / photo-OCR) and a language picker (20+ languages) that only matters for
   OCR. UX: choose-file button, drag-drop, paste-from-clipboard, URL input, a
   sample file, start/stop. Free with paid tiers for larger/faster jobs.
2. **PDF Candy — Extract Text** (`pdfcandy.com/extract-text.html`). Input: PDF
   (plus Google Drive/Dropbox picker). Output: .txt (+ share link / QR / cloud
   export). Auto-OCR when no text layer is found. Multi-file batch (desktop app),
   thumbnail previews, auto-start on upload, files deleted after ~2h. No
   language/encoding/page-range knobs exposed. Paid tiers raise the file-size cap
   (up to 500 MB).
3. **PDFforge — Extract Text** (`pdfforge.org/online/en/extract-text`). Input: PDF
   only, max 250 MB. Output: a downloadable text file. Deliberately minimal — no
   OCR/language/page-range/encoding options. UX: file dialog, drag-drop, URL
   input, 3-step flow. Emphasis on EU-hosted, privacy-respecting processing.

## Table-stakes → in-model / out-of-model (every one accounted for)

| Table-stake | Decision |
|---|---|
| Extract the embedded text layer to plain text | **In-model** — the core function. ✅ shipped (lopdf for PDF). |
| Input via URL | **In-model** — `url` param. ✅ |
| Input via file upload | **In-model (chat/CLI)** — chat accepts an attachment `ref`; CLI/chat accept `url`. No drag-drop *page* exists (no-page pattern). |
| Plain-text output | **In-model** — flat `{text, chars, format, truncated}` JSON. ✅ |
| **OCR of scanned / image-only PDFs** | **Out-of-model** — needs an ML/OCR model (Tesseract/transformers); gizza is pure-Rust + ffmpeg, no model. Stated explicitly ("embedded text layer only — does NOT OCR"). |
| OCR **language selection** | **Out-of-model** — only meaningful with OCR (above). |
| **DOCX (Word) input** | **In-model + our differentiator** — none of the top-3 do Word/EPUB in one tool. ✅ shipped (zip + quick-xml WordprocessingML flattening). |
| **EPUB (e-book) input** | **In-model + differentiator** — ✅ shipped (reuses `epub-to-markdown` core, plain-text mode). |
| Auto-detect the format | **In-model** — sniff `%PDF-` vs ZIP (`word/document.xml` → DOCX, `META-INF/container.xml` → EPUB). ✅ |
| Batch / multiple files at once | **Out-of-model** — the descriptor models one input; single-input by design. |
| Cloud-storage pickers (Drive/Dropbox) | **Out-of-model** — no accounts, no backend. |
| Convert to .docx / searchable-PDF output | **Out of scope** — this tool extracts *text*, it doesn't re-encode (that's a conversion tool). |
| Page-range selection | **Considered, deferred** — a PDF-only concept that doesn't map to DOCX/EPUB; per-page PDF extraction already lives in `pdf-extract-text` (`page` param). Kept the unified tool's semantics clean (all text). |
| Large file caps (250–500 MB) | **Considered, not matched** — browser-local wasm memory: input capped at 16 MiB, output at 1M chars (both stated). |

## UX-control patterns

Competitor UX (drag-drop, paste-to-upload, presets, language dropdowns) are all
**page** affordances. This tool has no page (binary-in/text-out doesn't fit the
pure-text or ffmpeg-media page shapes), so there are no sliders/pickers/chips to
match. The chat surface handles uploads via attachment refs; the CLI takes a URL.

## Result

Ours is the only one of the four that extracts **PDF + DOCX + EPUB** from a single
auto-detecting tool. It matches the shared table-stake (embedded-text → plain text)
and cleanly states the one big competitor feature it can't do in-model (OCR of
scanned pages). No copy or assets were reproduced.
