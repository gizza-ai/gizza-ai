# pdf-notes-outliner — competitor analysis (2026-07-06)

Tool function: extract the text layer from a lecture/course PDF and build a
structured heading outline (table of contents) with a short extractive summary
under each section. Pure-Rust, runs in the browser / chat / CLI, nothing uploaded,
no ML model.

## Competitor scan (paraphrased — no copy/branding reproduced)

Searched for "PDF outline generator / extract headings / table of contents /
section summary" and "lecture notes PDF outliner". Top real tools reviewed:

1. **pdf.tocgen (Krasjet, open-source CLI toolset).** Extracts a PDF table of
   contents automatically from embedded font attributes (name, size, bold/italic,
   color) plus text position. Supports a multi-level heading hierarchy (level 1,
   2, 3…) driven by font-size buckets, per-page filtering, and a human-readable
   ToC output. Recipe files let a power user hand-tune which font maps to which
   level. Output is a ToC (titles + levels + page positions), no summaries.

2. **PDFOutliner AutoTOC (macOS app).** "AutoTOC" analyses the range of fonts in
   the document and teases out a nested H1–H4 heading hierarchy automatically;
   users can also manually assign a selected font to a heading level and save
   style sets. Documented limitation: works poorly on scanned/OCR PDFs (no font
   info) and on documents with minimal font differentiation.

3. **AI PDF/lecture outliner+summarizer tools (Study Cue note outliner, Knowt,
   NoteGPT, Gemini "PDF to notes" prompt packs).** Turn a dense academic PDF into
   a structured outline plus summaries, glossaries, key terms, essential
   questions and "exam-ready points". These are LLM-backed: they generate
   abstractive summaries and net-new study material, not just re-structure the
   source text.

(All three real tools reachable; none copied.)

## Table-stakes → in-model / out-of-model

| Capability | Source | Decision |
|---|---|---|
| Automatic font-size heading detection → hierarchy | pdf.tocgen, PDFOutliner | **in-model** — reuse `pdf-to-markdown-core` font-size-bucket ranking (H1..H6) |
| Configurable heading depth (H1–H4/limit) | pdf.tocgen levels, PDFOutliner H1–H4 | **in-model** — `max_depth` param (1–6, default 3); deeper headings fold into parent section |
| Page number per heading | pdf.tocgen positions | **in-model** — page tracked via page-separator boundaries |
| Per-section summary | AI outliners | **in-model** — extractive **TextRank** per section (`summary_sentences`, reuse `textrank-summarize-core`) |
| Headings-only / pure ToC mode | pdf.tocgen | **in-model** — `summary_sentences = 0` |
| Graceful handling of scanned/no-text PDFs with a warning | PDFOutliner limitation | **in-model** — `note` field, no OCR |
| Manual font→level assignment / recipe editing | pdf.tocgen recipes, PDFOutliner manual assign | **out-of-scope** — interactive desktop feature; we auto-detect (their *automatic* mode). Listed, not built. |
| Import/write the ToC into PDF bookmarks (pdftocio) | pdf.tocgen | **out-of-scope** — a distinct "add PDF bookmarks/outline" writer tool, different scope. Listed, not built. |
| Glossary, key terms, Q&A, abstractive summary, translation, "study pack" | AI outliners | **out-of-model** — needs an LLM. Listed, not built. |

## UX / control patterns

Competitors are CLI (pdf.tocgen), a native desktop app (PDFOutliner) and LLM web
apps. Our surface is **chat + CLI, no standalone page** — same as the other
PDF→text tools (`pdf-to-markdown`, `pdf-extract-text`, `pdf-to-epub`,
`epub-to-markdown`): a PDF is a binary file input whose text/JSON output fits
neither the pure-text nor the ffmpeg file→media page shapes (all file-input pages
in the repo are `runtime="ffmpeg"`). So slider/color/chip page controls are N/A;
the in-model knobs live in the descriptor (`max_depth`, `summary_sentences`),
each with a `.describe()` an LLM/CLI user can act on.

## Design outcome (descriptor)

- `Input::Document` (url ⊕ ref).
- `max_depth`: integer 1–6, default 3 — deepest heading level kept; deeper
  headings fold into the parent section's body so their text still feeds the
  summary.
- `summary_sentences`: integer 0–10, default 2 — TextRank sentences per section;
  0 = pure outline / table of contents.
- Output: `outline` (indented text with page numbers + summaries), `sections`
  array `{level,title,page,summary}`, `section_count`, optional `note`.
- Limits stated: embedded text layer only (no OCR of scanned PDFs); summaries are
  **extractive** (verbatim source sentences ranked by TextRank), not abstractive.
