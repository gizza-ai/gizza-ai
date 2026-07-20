# pdf-diff — competitor analysis (2026-07-20, build-time scan)

Scanned before implementing (create-next-tool step: competitor scan BEFORE implementing).
Top real competitor tools skimmed (paraphrased — no copy/branding taken):

1. **diff.tools PDF Compare** (diff.tools/pdf-compare) — local in-browser compare; two
   uploads; page-selection strip (compare/exclude specific pages); word-level text edits
   across page boundaries; formatting-only change cards (font/size/color); replaced-image
   detection shown old-next-to-new; change list to walk through; export extracted text to a
   text-diff view; OCR option for scanned PDFs.
2. **way2pdf Compare** (way2pdf.com/compare) — original v1 + revised v2; page-by-page
   text-layer diff; additions green / deletions red-strikethrough; summary counts (total
   pages, changed pages, unchanged pages); sequential page alignment (extra pages = new);
   50 MB cap; states its limits: text layers only, no scanned PDFs (OCR first), no
   password-protected files, formatting ignored.
3. **PDF24 Compare** (tools.pdf24.org/en/compare-pdf) — two files; textual mode with
   same/new/removed color coding; accepts non-PDF inputs by converting to PDF first;
   server-side processing; adjustable settings before comparing.

## Table stakes → in-model / out-of-model

| Capability (table stake) | Where seen | Tag | Landed as |
| --- | --- | --- | --- |
| Two PDF inputs, original-then-revised order | all 3 | in-model | required `files` source_list (exactly 2, original first) |
| Per-page text diff, word-level add/remove/replace | way2pdf, diff.tools | in-model | word-level LCS hunks per aligned page pair, with context snippets |
| Line-level diff option | PDF24 textual view | in-model | `mode` enum `words\|lines` (default `words`) |
| Summary counts: total/changed/unchanged/added/removed pages | way2pdf | in-model | `pages` summary object + one-line `summary` |
| Page selection (compare only some pages) | diff.tools | in-model | `pages` range param (`"1-5,8"`, `odd`/`even`/`all`), same grammar as pdf-split/pdf-delete-pages |
| Cross-page alignment (inserted/removed pages don't cascade) | diff.tools (word diff across page boundaries) | in-model (adapted) | `align` enum `auto\|sequential` (default auto): similarity-based page alignment marks inserted/removed pages, then pairs the rest |
| Replaced/added/removed image detection | diff.tools | in-model (object level) | per-page image-XObject content hashes → added/removed/replaced counts in `visual_changes` |
| Formatting change surfacing (font changes) | diff.tools change cards | partial in-model | per-page font-set (BaseFont) added/removed in `visual_changes`; word-level font/size/color attribution is out-of-model (below) |
| Page geometry changes | (implicit in visual compare tools) | in-model | MediaBox size + /Rotate changes per aligned pair in `visual_changes` |
| Case sensitivity control | generic diff tools | in-model | `ignore_case` boolean (default false) |
| Stated limits (text layer only, no OCR, no passwords) | way2pdf | in-model (copy) | descriptor + error messages say so explicitly; encrypted PDFs get a clear error |
| Document metadata changes (Title/Author/dates) | Acrobat-class compare | in-model | `metadata_changes` from the /Info dict |

## Out-of-model (listed, not built)

- **Rendered pixel-level visual diff / side-by-side page overlays** (diffguru "light
  table", diff.tools rendered pages): needs a full PDF rasterizer (pdfium/mupdf class);
  no wasm-safe pure-Rust PDF renderer is available here. We do object-level visual
  comparison instead (page size, rotation, image hashes, font sets) and say so.
- **OCR for scanned PDFs** (diff.tools local OCR): needs an ML model — out for gizza
  (pure Rust + ffmpeg). Tool reports "no selectable text" explicitly instead.
- **Word-level formatting attribution** (this word turned bold/red): needs positioned
  layout extraction; lopdf text chunks carry no font/coords per word. Font-set changes
  per page are the in-model approximation.
- **Marked-up output PDF / redline export** (Draftable-style): highlight rectangles need
  glyph coordinates lopdf's extractor does not expose.
- **Non-PDF inputs auto-converted** (PDF24 accepts Word/Excel): separate tools already
  cover conversion (docx-to-pdf etc.); this tool takes PDFs only.
- **Side-by-side synced scroll UI**: the standalone page surface takes a single upload;
  a two-file tool ships chat+CLI only (platform constraint, same as
  video-audio-sync-offset-finder / loudness-matched-ab-prep / merge-pdf).
- **Annotation (comment/link) diffing** (Acrobat-class): deferred; not in the top-3 web
  tools' table stakes.

## Design decisions

- Auto page alignment uses Jaccard similarity over per-page word-hash sets +
  monotone max-weight matching (threshold 0.3), then pairs leftover gap pages
  index-wise as "rewritten" so a fully rewritten page reads as changed, not
  removed+added. Docs > 200 pages fall back to sequential alignment (warned).
- Word-diff DP is prefix/suffix-trimmed with a middle-region cell cap; beyond the cap a
  page is summarized (added/removed word counts + first difference) with `truncated: true`
  rather than OOMing the 64 MiB sandbox.
- Hunk caps: 40 hunks per page, 250 total, context 6 words each side; truncation flagged.
- Same-URL-twice → `identical: true` gives the CLI an exact-output determinism case.
- 8 MiB per file (merge-pdf precedent); pages cap 2000/doc.
