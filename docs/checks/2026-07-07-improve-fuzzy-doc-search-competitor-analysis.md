# fuzzy-doc-search — competitor analysis (2026-07-07)

Tool built this run: **fuzzy-doc-search** — full-text fuzzy (typo-tolerant) search over
pasted text, returning ranked matching snippets with surrounding context. Pure, browser-local,
no upload, no sign-up.

All notes below are **paraphrased** — no competitor copy, branding, or trademarks reproduced.

## Competitors scanned

1. **GroupDocs fuzzy search web app** (products.groupdocs.app/search/fuzzy) — the closest
   direct end-user tool. Upload files → fuzzy search across them.
2. **Fuse.js** (fusejs.io) — the de-facto fuzzy-search library; its options define the
   parameter table-stakes for the category.
3. **MiniSearch** (github.com/lucaong/minisearch) — in-browser full-text engine; defines the
   ranked-relevance + matched-terms table-stakes.

(All three reachable. Fuse.js `/api/options.html` 404'd; captured the options from the home page.)

## Table-stakes matrix (param / feature → in-model? → where it landed)

| Capability | Competitor(s) | In gizza's model? | Landed as |
|---|---|---|---|
| Search query (one or more words) | all | yes | `query` (required) |
| Document text to search | all | yes | `text` (required, multiline paste) |
| Fuzziness / edit-distance tolerance | GroupDocs (1–9 chars), Fuse (`threshold`), MiniSearch (`fuzzy`) | yes | `fuzziness` int 0–3 (max typos per term; 0 = exact) |
| Case sensitivity | Fuse (`isCaseSensitive`), GroupDocs | yes | `case_sensitive` bool (default off) |
| Match ANY vs ALL words (phrase/all/any) | GroupDocs (search-type selector) | yes | `match` enum `any`/`all` (OR vs AND) |
| Whole-word vs substring/prefix | MiniSearch (prefix), Fuse | yes | `whole_word` bool (off = substring/prefix matches too) |
| Relevance score per result (`includeScore`) | Fuse, MiniSearch | yes | each result carries a 0–100 `score` |
| Ranked results (best first) | all | yes | results sorted by score desc, location asc |
| Snippet / context around match | GroupDocs, Fuse (`includeMatches`) | yes | snippet = the whole matching line/sentence/paragraph, matched words wrapped in «…» |
| Match granularity (where the snippet comes from) | (implicit) | yes | `unit` enum `line`/`sentence`/`paragraph` |
| Cap number of results (best-only toggle) | GroupDocs (best-matches-only) | yes | `max_results` int 1–50 (default 10) |
| Highlight matched characters/words | GroupDocs, Fuse | yes | matched document words wrapped in «…» in every snippet |
| Search across MULTIPLE documents | GroupDocs (multi-file), all libs (corpus) | partial | paste one or more documents as one corpus; ranking is corpus-wide. Separate per-file upload is out-of-model (below) |
| **File upload — PDF / DOCX / XLSX / EPUB / 80+ formats** | GroupDocs | **out-of-model** | listed, not built (see below) |
| Drag-and-drop / multi-file picker | GroupDocs | **out-of-model** | listed, not built |
| Server-side indexing of huge corpora | GroupDocs, cloud engines | **out-of-model** | listed, not built (browser-local, in-memory) |

## Out-of-model (considered, not built) — with reasons

- **Binary file upload (PDF / DOCX / EPUB / XLSX):** gizza's pure-tool page is text-field only —
  the shared page runtime wires file inputs **only** for the ffmpeg media runtime, and a pure page's
  `gatherArgs()` reads text fields, not file bytes. Pulling a PDF parser into the browser wasm +
  a bespoke file-upload `custom.js` is a large, separate surface. gizza already ships
  **document-text-extract** (PDF/DOCX/EPUB → text) and **pdf-extract-text** — the honest workflow is
  extract-then-search: run those, paste the text here. Stated on the page + FAQ. `txt` and `markdown`
  ARE supported directly (both are plain text — paste them).
- **Separate multi-file upload / drag-and-drop:** same root cause (no file input on a pure page).
  Multiple documents are supported by pasting them as one corpus; ranking spans the whole paste.
- **Server-side indexing / very large corpora:** gizza runs entirely in the browser tab, in memory.
  Practical for documents up to a few MB of text, not for indexing gigabytes. Stated as a limit.

## UX patterns adopted (original implementation, no copied assets)

- Fuzziness exposed as a small **slider** (0–3) — the "mistake threshold" control the GroupDocs app
  and Fuse both surface, but bounded to what edit-distance matching can honestly deliver.
- **`match` and `unit` render as `<select>`s**, `case_sensitive`/`whole_word` as checkboxes
  (right control for the data), so the page reads as a purpose-built search box, not a generic form.
- **Preset example chips** (`[[example]]`) demonstrate a typo-tolerant search and a phrase (all-words)
  search so a first-time visitor sees ranked output before typing.
- Every result shows **rank · location · score** and highlights the matched words — the "ranked
  snippets with context" the category is defined by.
