# epub-extract — competitor analysis (2026-07-17)

Tool: **epub-extract** — "Extracts readable text and chapter structure from an EPUB ebook so
it can be searched, summarized, or quoted." Pure Rust (zip + quick-xml + nanohtml2text),
binary-file-in → JSON-out, chat + CLI, **no page** (matches the `epub-to-markdown` /
`pdf-extract-text` no-page file-input pattern).

## Why this is not a duplicate of `epub-to-markdown`

`blocks/epub-to-markdown` converts a whole EPUB into **one concatenated Markdown (or plain-text)
document** and returns only `{title, chapters: <count>, content}`. It gives you a blob plus a
number. `epub-extract` instead returns the **chapter structure**: an ordered list of chapters,
each with its own title (from the EPUB's real TOC — NCX/nav — with heading-detection fallback),
per-chapter word count, and per-chapter readable text, plus full book metadata (author,
language, publisher, date). That lets a caller navigate, search, quote a *specific* chapter, or
pull a lightweight table of contents — none of which the concatenated-blob tool supports. Real,
non-overlapping capability, so it ships as its own tool (not skiplisted).

## Competitors surveyed (paraphrased — no copy/branding reused)

1. **Encode64 — EPUB to Text** (encode64.com) — browser-local extraction. Options: include
   title/author metadata; separate chapters with headings; optionally **limit the number of
   chapters**. Output = readable text per chapter, reading order preserved.
2. **epub-to-text (PyPI)** — CLI. `--json` exports metadata + chapters; `--chapters-text` writes
   each chapter separately; `--info` shows book information only (no full text).
3. **EPUBToText (Projet-TAMIS, Node.js)** — per-chapter objects with internal **id, title,
   excerpt, size, sequence number**; extracts chapter metadata.
4. **ebook_splitter (hirowa, GitHub)** — structured chapters → CSV. Three-tier titling: native
   **TOC** first, then **heading detection** (`<h1>/<h2>`), then a GPT fallback to *infer* titles.
5. **epub2txt2 (kevinboone)** — spine-order text extraction with a `--separator` to split output
   into per-chapter sections; formatting stripped, reading order preserved.

## Table-stakes → in-model / out-of-model decisions

| Capability | Competitors | Decision |
|---|---|---|
| Per-chapter segmentation in reading (spine) order | all | **in-model** — spine-ordered `chapters[]` |
| Real chapter titles from the EPUB TOC (NCX + EPUB3 nav) | EPUBToText, ebook_splitter | **in-model** — parse `toc.ncx` navMap + nav `<a>` |
| Heading-detection title fallback (`<h1>…<h6>`, `<title>`) | ebook_splitter | **in-model** — `first_heading` → `<title>` → `Chapter N` |
| Per-chapter word count | EPUBToText (size) | **in-model** — `words` per chapter + book `word_count` |
| Book metadata: author, language, publisher, date | Encode64, epub-to-text, ToMarkdown | **in-model** — OPF `dc:creator/language/publisher/date` |
| Include-metadata toggle | Encode64, epub-to-text | **in-model** — `include_metadata` bool (default true) |
| Structure/TOC-only mode (no full text) | epub-to-text `--info` | **in-model** — `include_text` bool (default true) |
| Limit number of chapters | Encode64 | **in-model** — `max_chapters` int (0 = all) |
| Plain readable text, formatting stripped, reading order | all | **in-model** — nanohtml2text per spine item |
| GPT/LLM title *inference* when no TOC/headings | ebook_splitter | **out-of-model** — needs an LLM; heading fallback covers it |
| Cover-image / embedded-image extraction | some editors | **out-of-model here** — separate image concern; text tool |
| Rewrite/re-flow to CSV/XLSX file download | ebook_splitter (CSV) | out-of-scope — JSON chat output; other blocks handle files |

Every table-stake is either implemented in the descriptor or explicitly listed out-of-model
above; none dropped silently.

## Resulting descriptor (in-model params)

- `include_text` (bool, default `true`) — include each chapter's readable text; set `false` for a
  structure-only table of contents (titles + word counts).
- `include_metadata` (bool, default `true`) — include book metadata (author, language, publisher,
  date) alongside the title.
- `max_chapters` (int, default `0` = all) — cap how many chapters (spine items) to extract.

Output: `{title, author, language, publisher, date, chapter_count, word_count, chapters:[{index,
title, words, text?}], truncated}`. No competitor copy, branding, or trademark reused.
