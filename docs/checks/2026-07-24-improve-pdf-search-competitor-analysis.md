# pdf-search — competitor analysis (2026-07-24)

**Tool:** `pdf-search` — "Searches a PDF for a word or phrase and lists matching pages
with surrounding context."

**Surface:** chat + CLI only, **no page**. Input is a binary PDF (`Input::Document`,
`url`⊕`ref`), which fits neither the pure-text nor the ffmpeg file→media page shapes —
the same no-page file-input pattern as the sibling `pdf-extract-text` (its module header
documents this). All PDF-*input* blocks in this repo are page-less for the same reason;
only `csv-to-pdf-table` (text→PDF) has a page.

## Competitors surveyed

1. **pdfgrep** (CLI, poppler-backed) — the closest analog. Flags: `-i` ignore case,
   `-w` whole-word, `-n` page number, `-c` count, `-C N` context, `-m N` max matches,
   `--color` highlight, regex patterns. [mankier.com/1/pdfgrep, geeksforgeeks pdfgrep]
2. **Adobe Acrobat "Advanced Search"** — search word/phrase across a document, options
   for *Whole words only* and *Case-Sensitive*; results list each hit with its page and a
   surrounding-text snippet, match highlighted. [helpx.adobe.com/acrobat/using/searching-pdfs]
3. **PDF.js viewer find bar** (Firefox/Chrome built-in PDF viewer) — Highlight all,
   *Match Case*, *Whole Words*, *Match Diacritics*; jumps between hits by page.
4. **Smallpdf / PDF2Go / SeekFast "search in PDF"** — upload a PDF, enter a term, get
   every occurrence across pages with the matching text highlighted and page numbers;
   SeekFast shows the sentence/snippet each hit appears in. [smallpdf, pdf2go, seekfast blogs]
5. **ToolsSeek / pdfFiller "PDF text search"** — online: type a word/phrase, list matches
   with page + context, highlight the term. [toolsseek.com/tool/pdf-text-search-tool]

## Table-stakes → decisions

| Capability | In model? | Decision |
|---|---|---|
| Word/phrase (literal) query | ✅ | `query` (required string). Phrase matching spans line breaks (whitespace normalized). |
| Case-insensitive by default, opt-in case match | ✅ | `case_sensitive` bool, default `false`. |
| Whole-words-only | ✅ | `whole_word` bool, default `false` (alphanumeric-boundary check). |
| Surrounding context snippet | ✅ | `context` int chars each side, default 60; the matched span is wrapped in `«…»`. |
| Page number per hit | ✅ | every match carries its 1-based `page`. |
| Total count + pages-matched | ✅ | `total_matches`, `pages_matched` in the response. |
| Max-matches cap | ✅ | `max_matches` int, default 100 (bounds LLM/CLI output); `truncated` flag when capped. |
| Highlight the term | ✅ | matched span wrapped in `«…»` inside each snippet. |
| **Regex patterns** (pdfgrep) | ⚠️ out of scope | This tool is literal word/phrase per its description. For regex, pipe `pdf-extract-text` → the existing `regex-search` block. Not built here to avoid mislabeling a literal search as regex. |
| **OCR of scanned/image-only PDFs** | ❌ out of model | Needs a trained OCR model (gizza is pure-Rust + ffmpeg, no ML). Same limit as `pdf-extract-text`: the embedded selectable text layer only. Stated in copy. |
| **Match diacritics toggle** (PDF.js) | ❌ out of scope | Unicode case-folding here takes the first lowercase char per char (ASCII-accurate); diacritic-insensitive folding not implemented. Noted as a limit. |
| **Cross-file / multi-PDF search** | ❌ | Single-source input model (one PDF per call). |

**No competitor copy, branding, or trademarks are reproduced.** Out-of-model / out-of-scope
items are listed, not built.
