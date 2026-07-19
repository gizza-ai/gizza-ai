# search-index-builder — competitor analysis (2026-07-17)

Tool function: turn a JSON array of document objects into a serialized **inverted-index
JSON** for offline, serverless full-text search. Each chosen field is tokenized and the
index maps `token → field → { df, postings: { ref → tf } }`, alongside a document store
of display fields and optional per-field ranking boosts. Pure, deterministic, and fully
client-side (wasm), like the rest of the gizza toolkit — no crawling, no hosting, no
query runtime.

## Competitors scanned (top real tools)

1. **Lunr.js** (lunrjs.com) — in-browser inverted-index library. You declare fields and a
   ref, add documents, and it builds an immutable in-memory index that serializes to/from
   JSON (`toJSON` / `load`), so the index can be pre-built and shipped as a static file. It
   ranks with TF-IDF and supports per-field query-time boosts; the index is fixed once
   built.
2. **Elasticlunr.js** (elasticlunr.com) — a Lunr fork. Same register-fields / set-ref /
   add-JSON-docs / serialize-to-JSON model, with combined Boolean + TF-IDF scoring, a
   choice of which fields to index vs. search, per-field boosts, and an option to omit the
   stored document to shrink the index. Acts as both a build-time index builder and a
   query-time library.
3. **MiniSearch** (github.com/lucaongaro/minisearch) — lightweight client-side engine with
   configurable field selection, stored fields, tokenization/term processing, TF-IDF/BM25
   ranking, and prefix + fuzzy matching. It also supports mutable indexes (add/remove after
   creation) and JSON serialization for build-then-load workflows.
4. **FlexSearch** (github.com/nextapps-de/flexsearch) — performance-focused client-side
   engine for large datasets, offering several indexing strategies (classic inverted index
   plus more memory-efficient/contextual modes), real-time/incremental indexing,
   tokenization presets, and index export/import. More a query-time engine than a plain
   build artifact.
5. **Pagefind** (pagefind.app) — build-time static-search tool that runs after a site
   generator, reads the rendered HTML, and emits a chunked static index bundle plus a
   JS/WASM query API and prebuilt UI, downloading only the fragments a query needs. It is a
   crawler + index generator + query runtime + UI — much broader than a pure JSON→JSON
   transform. (Stork — stork-search.net — is a close Rust/WASM analogue: a CLI reads a
   config of documents and produces a single index file consumed by a WASM query library.)

**Framing:** Lunr / Elasticlunr / MiniSearch / FlexSearch are primarily *query-time
libraries* that happen to serialize a pre-built index; Pagefind / Stork are *build-time
generators* bundled with their own crawler and query runtime. Our tool occupies the pure
middle slice: a deterministic documents-JSON → inverted-index-JSON transform, with no
crawling, hosting, or query runtime.

## Table-stakes → decision

| Capability | Competitors | Our decision |
| --- | --- | --- |
| Field selection (which fields to index) | Lunr, Elasticlunr, MiniSearch, FlexSearch | **in** — `fields` (blank = every string field) |
| Id / ref field | Lunr/Elasticlunr `setRef` | **in** — `id_field` (falls back to array position) |
| Lowercase / case-folding | default tokenizer everywhere | **in** — `lowercase` (default on) |
| Minimum token length filter | tokenizer/trimmer stages | **in** — `min_length` 1–20 |
| Stop-word removal | Lunr/Elasticlunr stop filter | **in** — `remove_stopwords` (short English list) |
| Stored display fields (title/url store) | Elasticlunr doc store, MiniSearch `storeFields` | **in** — `store_fields` → `documents` store |
| Per-field ranking boosts | Lunr/Elasticlunr field boosts | **in** — `boosts`, recorded as index metadata |
| df / tf postings for TF-IDF | all TF-IDF/BM25 engines | **in** — the core output of this tool |
| Compact vs. pretty JSON | serialize + minify | **in** — `pretty` (default compact for shipping) |
| Stemming (Porter, etc.) | Lunr/Elasticlunr | **out of scope** — language-specific; heavy and locale-bound. Consumers can pre-stem tokens if desired |
| Crawling / reading files / HTML parsing | Pagefind, Stork | **out of scope** — input is documents-JSON; crawling/hosting is a different product and breaks purity/determinism |
| Query-time search + ranking | all libraries | **out of scope** — we emit an index, not a runtime; ranking consumes df/tf + boosts client-side |
| Fuzzy / typo matching | MiniSearch, FlexSearch, Fuse.js | **out of scope** — a query-time concern, not an index-build concern |
| N-gram / prefix / autocomplete indexing | MiniSearch prefix, FlexSearch | **out of scope** — multiplies index size and bakes in a query strategy; keep the artifact a plain token inverted index |
| Incremental / mutable indexing | MiniSearch, FlexSearch | **out of scope** — conflicts with a pure full-rebuild transform; consumers re-run the build |

Every table-stake is either implemented in the descriptor or explicitly listed
out-of-scope above — nothing dropped silently. No competitor copy, branding, or
trademarks are reproduced beyond factual tool names/URLs.

## Positioning

Own the deterministic, language-neutral "documents JSON in → inverted-index JSON out"
slot (the top block of the table) and deliberately exclude language-specific processing
(stemming), I/O (crawling), and query-time behaviors (ranking, fuzzy, prefix,
incremental) — which are exactly where the broader tools diverge and where scope and
index size balloon.
