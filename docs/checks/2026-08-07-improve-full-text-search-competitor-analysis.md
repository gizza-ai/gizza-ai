# full-text-search — competitor scan + gap analysis (2026-08-07)

Scan run **before** implementation, per `create-next-tool` step 4. All findings are
paraphrased from public documentation; no competitor copy, branding, or trademarks are
reproduced or reused.

## Backlog row

```
full-text-search	full-text-search	Searches across pasted documents with BM25/TF-IDF
ranking, stemming, and snippet highlighting, returning ranked results.	pure
```

## Duplicate check (done first — this row has three close neighbours)

| Existing block | What it actually does (verified in `core/src/lib.rs`) | Overlap verdict |
| --- | --- | --- |
| `blocks/fuzzy-doc-search` | Scores **snippets inside ONE pasted blob**. Ranking = mean bounded-Levenshtein word quality × query coverage × 100. `grep -Ei 'stem\|porter\|idf\|corpus\|avgdl\|k1'` over its core returns **nothing** — no stemming, no IDF, no corpus, no length normalisation. Ranks lines/sentences/paragraphs. | **Not a duplicate.** Different ranking model *and* different unit of retrieval (snippet within one text vs document within a corpus). |
| `blocks/search-index-builder` | **Builds** an inverted-index JSON for an *external* client to rank with. Its own doc comment says the output is "what a client needs to rank results with TF-IDF" — it never queries or ranks. | **Complementary, not duplicate.** It is the index half; this row is the query half. |
| `blocks/search-in-documents`, `blocks/pdf-search`, `blocks/regex-search` | Literal/regex grep over a binary document, a PDF, or lines of text. No ranking at all. | Not duplicates. |

The decisive point: **BM25 is inherently corpus-level.** Its IDF term
(`ln(1 + (N − df + 0.5)/(df + 0.5))`) and its length normalisation against `avgdl` cannot
be computed from a single undivided text blob, which is the only input shape
`fuzzy-doc-search` accepts. Built.

## Competitors reviewed

1. **MiniSearch** (in-browser JS full-text engine) — the closest shape to ours: an in-memory
   engine over a pasted/loaded document set.
2. **Lunr.js** (client-side search for static sites) — the reference for the
   stemming + stop-word pipeline.
3. **BM25 as shipped by Lucene-class engines** (Elasticsearch / `pg_textsearch` /
   Azure DocumentDB full-text) — the reference for the scoring function and its tunables.

## Table stakes → decision

Every item below lands in the descriptor or in the out-of-model list. Nothing dropped silently.

| # | Table stake | Seen in | Decision |
| --- | --- | --- | --- |
| 1 | BM25 relevance ranking | Lucene-class, MiniSearch | **In** — `algorithm = "bm25"` (default). Canonical smoothed IDF + TF saturation + length normalisation. |
| 2 | Classic TF-IDF as an alternative scorer | MiniSearch (supports both) | **In** — `algorithm = "tfidf"`. |
| 3 | `k1` tunable (TF saturation) | Lucene-class, MiniSearch `bm25.k1` | **In** — `k1`, default **1.2** (documented standard range 1.2–2.0), slider. |
| 4 | `b` tunable (length normalisation) | Lucene-class, MiniSearch `bm25.b` | **In** — `b`, default **0.75** (documented standard), slider. |
| 5 | Porter stemming (`running` → `run`) | Lunr (pipeline default), lunr-languages | **In** — `stemming`, default on. Implemented as a dependency-free Porter stemmer in the core. This is the capability `fuzzy-doc-search` structurally cannot reach: `running`→`run` is edit distance 4, past its fuzziness cap of 3. |
| 6 | English stop-word filtering | Lunr (`stopWordFilter` in the default pipeline) | **In** — `stopwords`, default on. Skipped when it would empty the query. |
| 7 | AND / OR term combination | MiniSearch `combineWith` (default `OR`) | **In** — `match` enum `any`/`all`, default `any`, matching the documented default. |
| 8 | Prefix search (`moto` → `motorcycle`) | MiniSearch (opt-in), Lunr (via `*`) | **In** — `prefix`, default **off**, matching both competitors' opt-in default. |
| 9 | Per-field boosting (e.g. title weighted above body) | MiniSearch `boost`, Lunr field boosts | **In, adapted.** Our corpus is pasted text, not structured records, so the first line of each document is treated as its title and `title_boost` (default 2.0) weights matches there. Same user-visible effect without inventing a schema. |
| 10 | Phrase search (`"exact phrase"`) | Lucene-class query syntax | **In** — quoted runs in `query` must match as a contiguous token sequence (positions are tracked at tokenise time; stemming applies to phrase tokens too). |
| 11 | Negation / term exclusion (`-term`) | Lucene-class query syntax | **In** — a `-term` prefix drops any document containing that term. |
| 12 | Top-N result cap | All three | **In** — `max_results`, default 10, capped at 50. |
| 13 | Snippet / keyword-in-context highlighting | Named in the backlog row; a search-UI table stake (Lunr leaves it to the caller) | **In** — `snippet_words` window (default 30, `0` disables), hits wrapped in `«…»` to match the house convention already used by `fuzzy-doc-search` and `search-in-documents`. |
| 14 | Document separator for a pasted corpus | n/a — our input shape | **In** — `separator` enum (`dashes` / `blank-line` / `form-feed`), default `dashes`. |

### Out of model (listed, deliberately not built)

- **Typo/fuzzy tolerance (edit distance).** Feasible here, but it is exactly what
  `blocks/fuzzy-doc-search` already ships. Building it again would make these two blocks
  genuine duplicates. Cross-linked from the page instead.
- **Persisted / incremental index.** MiniSearch and Lunr serialise an index and re-query it
  without re-indexing. gizza blocks are single-shot and stateless, so each run indexes the
  paste from scratch. The serialise half is already served by
  `blocks/search-index-builder`; cross-linked from the page.
- **Non-English stemmers / language packs** (lunr-languages ships ~15). The Porter algorithm
  is English-specific; a multi-language snowball port is a much larger surface than one tool.
  English-only is stated on the page.
- **Semantic / vector / hybrid retrieval** (Redis, Milvus, Azure DocumentDB hybrid modes).
  Needs an embedding model; gizza's runtime is pure-Rust + ffmpeg with no ML model loader —
  the same blocker recorded for the skiplisted `embedding-export`.
- **Searching binary documents directly** (PDF/DOCX/EPUB). Already served by
  `document-text-extract` → this tool; cross-linked from the page rather than duplicated.

## UX control patterns adopted

- `k1`, `b`, `title_boost` and `snippet_words` render as **sliders** (`kind = "slider"` with
  fractional `step`) — competitors expose these as numeric tuning knobs, and dragging beats
  typing for bounded ranges.
- `[input.labels]` gives `separator`, `algorithm` and `match` readable `<select>` labels
  (`Any word (OR)` / `All words (AND)`, `BM25 (recommended)` / `Classic TF-IDF`).
- **`[[example]]` preset chips** for the patterns competitors demo: a plain BM25 ranking run,
  a stemming demo (`running` finds `run`), a quoted-phrase search, and a TF-IDF comparison.

## Sources

- [MiniSearch](https://github.com/lucaong/minisearch) and its
  [SearchOptions reference](https://lucaong.github.io/minisearch/types/MiniSearch.SearchOptions.html)
- [Lunr core concepts](https://lunrjs.com/guides/core_concepts.html) and
  [lunr-languages](https://github.com/MihaiValentin/lunr-languages)
- [Understanding the BM25 full-text search algorithm](https://emschwartz.me/understanding-the-bm25-full-text-search-algorithm/)
- [pg_textsearch (BM25 for PostgreSQL)](https://github.com/timescale/pg_textsearch),
  [Azure DocumentDB BM25 keyword search](https://learn.microsoft.com/en-us/azure/documentdb/full-text-search-keyword)
