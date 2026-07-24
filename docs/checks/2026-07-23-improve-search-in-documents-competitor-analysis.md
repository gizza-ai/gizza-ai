# search-in-documents — competitor analysis (2026-07-23)

Tool function: search a regex or keyword *inside* binary document files (PDF, DOCX,
EPUB) and inside ZIP archives of documents, returning each match with the source
document and the page/section/line it came from. Paraphrased notes only — no
competitor copy, branding, or trademarks reproduced.

## Competitors skimmed (top real tools for this function)

1. **ripgrep-all / `rga`** (phiresky) — a ripgrep wrapper that recursively searches
   regex patterns and transparently descends into PDF, DOCX, ODT, EPUB, zip, tar.gz,
   sqlite, and more via per-format adapters. Reports `file:line`/`file:page`. Caches
   extracted text between runs for speed. All ripgrep flags carry over
   (`-i` ignore-case, `-w` word, `-F` fixed-string, `-c` count, `-A/-B/-C` context,
   smart-case default).
2. **pdfgrep** — grep for PDFs. Flags: `-n`/`--page-number` (report the page each
   match is on), `-i` ignore case, `-w` whole word, `-c` count, `-r`/`-R` recursive
   over a directory, `-C`/`-A`/`-B` context lines, `--color`, `-P` Perl regex,
   `-F` fixed string, `--cache`. Match location is the 1-based page number.
3. **PowerGREP** (GUI, Just Great Software) — type a keyword/phrase or regex, pick a
   folder, get a per-file hit list across PDF/Office/etc. Emphasises literal-phrase
   vs regex modes, whole-word, case options, and showing the surrounding context of
   each hit.
4. **findstring** (PyPI) — `grep -rI`-style search across a directory that also reads
   inside PDF (pdfminer) and DOCX (python-docx). Keyword/substring focus.
5. **pdf-regex-search** (GitHub, patogeno) — CLI that runs a regex over every PDF in a
   folder tree and lists matches with their file + page.

## Table-stakes → in-model / out-of-model decisions

| Capability | Competitors | Decision |
|---|---|---|
| Literal keyword **and** regex mode | all | **in** — `regex` boolean (default off = literal substring, like `-F`) |
| Case-insensitive search | all (`-i`, smart-case) | **in** — `case_sensitive` boolean (default off → case-insensitive) |
| Whole-word match | rga/pdfgrep `-w` | **in** — `whole_word` boolean |
| Report the **page** of each PDF match | pdfgrep `-n`, rga, pdf-regex-search | **in** — PDF units are per-page; `location = "page N"` |
| Report **which document** a match came from (archive/folder) | rga, pdfgrep `-r`, PowerGREP | **in** — for a ZIP archive each match carries its `document` = entry path |
| Match count / result cap | `-c`, ripgrep limits | **in** — `max_matches` cap + `truncated` flag |
| Return the matching line with the hit marked | grep default, `--color` | **in** — matching line returned with the hit wrapped in guillemets `«…»` (output is plain text; no colour spans) |
| Search inside DOCX (Word) | rga, findstring, PowerGREP | **in** — DOCX flattened (WordprocessingML runs); location is the line number (DOCX stores no page breaks) |
| Search inside EPUB (e-book) | rga | **in** — EPUB reading-order text; location is the line number |
| Search inside a ZIP archive of documents/text | rga (zip/tar) | **in** — each PDF / text entry in the archive is searched, matches tagged with the entry path |
| Recursive search over a **local folder tree** | rga `-r`, pdfgrep `-r`, findstring | **out** — gizza takes a single URL/attachment input, not a dropped folder. A ZIP archive is the in-model multi-document container. |
| OCR of scanned / image-only PDFs | (rga has an optional OCR adapter) | **out** — no OCR engine in the pure-Rust/wasm model; the embedded text layer only (scanned pages return no text) |
| Extraction caching between runs | rga, pdfgrep `--cache` | **out** — each call is stateless; N/A to a single request |
| ODT / tar.gz / sqlite / other formats | rga | **out (scope)** — this tool targets PDF, DOCX, EPUB, and ZIP per its spec |
| Context lines (`-A/-B/-C`) | rga, pdfgrep | **out (for now)** — each match already returns its full source line for context; whole surrounding-line context is delivered, adjacent-line context deferred |
| Nested Office/EPUB **inside** an archive | rga recurses | **partial** — ZIP entries that are PDFs or plain-text files are searched; nested DOCX/EPUB inside an archive are not recursed (provide them directly). Noted in output. |

## UX / control patterns

Competitors are CLI/GUI. This is a chat + CLI tool with no page (a document is a
binary file input whose output is structured text — the "no-page file-input" pattern,
like `pdf-extract-text` / `document-text-extract`). So there are no page controls
(sliders/chips) to match; the parity target is the flag set above, expressed as
descriptor params: `pattern` (required), `regex`, `case_sensitive`, `whole_word`,
`max_matches`. Every table-stake above lands in the descriptor or the out-of-model
list — none dropped silently.
