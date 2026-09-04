# vector-similarity — competitor analysis (2026-09-04)

Scan run BEFORE implementing, per the create-next-tool recipe. All findings are paraphrased
observations of publicly documented behaviour; no competitor copy, branding or trademarks are
reused anywhere in this block.

## Tools reviewed

1. **AI Dev Hub — "Embedding Similarity Calculator"** (aidevhub.io/embedding-similarity-calculator/)
   Two vector fields (A/B). Accepts JSON arrays plus comma- or space-separated floats. Reports
   cosine similarity, dot product, Euclidean (L2) and Manhattan (L1) **all at once**, with a short
   interpretation line and a formula/range comparison table. Has a "Load Sample" button. Claims
   support for 1 → 10,000+ dimensions and calls out the common embedding widths (384, 768, 1024,
   1536, 3072). Runs client-side.
2. **PythonAlchemist — "Vector Calculator"** (pythonalchemist.com/tools/vector-calculator)
   Per-component numeric fields with add/remove-dimension buttons (3-D by default). Computes dot
   product, cosine similarity and Euclidean distance, and shows the expanded arithmetic
   (`(1×4) + (2×5) + (3×6)`) as a worked breakdown. No precision control, no stated limits.
3. **Tilores — "Cosine Similarity Calculator"** (tilores.io/cosine-similarity-online-tool/)
   Two *string* fields; converts each to a character-bigram vector and reports one 0–1 similarity
   score. Preset category chips (person / company / address / other). Positions itself around RAG
   retrieval, deduplication and entity resolution — i.e. a text tool, not a numeric-vector tool.
4. **RedCrab — "Vector distance calculator"** (redcrab-software.com/en/Calculator/Vector/Distance)
   Classic maths-calculator surface: distance, dot/cross product, magnitude, normalisation. Pure
   two-vector geometry, no ranking.
5. **VectorWiki — "Distance Metrics" reference** (vectorwiki.com/math/distance-metrics)
   Not a calculator but the de-facto expectations document competitors are written against:
   Euclidean (L2), Manhattan (L1), cosine, dot-product; Hamming named for binary vectors. Key
   guidance: many embedding models emit normalised vectors, which makes cosine and dot product
   equivalent; cosine is the default for text embeddings, Euclidean for physical/image features.

## Table stakes → where each one landed

| Capability | Seen in | Decision |
|---|---|---|
| Cosine similarity | 1,2,3,4,5 | `metric = cosine` (default) + always shown in the all-metrics table |
| Dot product | 1,2,4,5 | `metric = dot` + column |
| Euclidean (L2) | 1,2,4,5 | `metric = euclidean` + column |
| Manhattan (L1) | 1,5 | `metric = manhattan` + column |
| Hamming | 5 (backlog row) | `metric = hamming` + column, with a `hamming_tolerance` for float vectors |
| Cosine *distance* (1 − cos) | 5, vector DBs | `metric = cosine_distance` |
| All metrics reported together | 1 | `show_all_metrics` (default true) — one row per vector, one column per metric |
| JSON-array **and** comma/space input | 1 | Parser accepts `[…]`, commas, spaces, tabs, semicolons, and newlines |
| Sample/preset loading | 1,3 | Three `[[example]]` preset chips on the page |
| Query magnitude / vector stats | 2,4 | Header line reports dimensions + query magnitude |
| Normalisation advice (unit vectors) | 5 | `normalize` toggle — L2-normalise before comparing |
| Stated dimension limits | 1 | Documented + enforced: 8192 dims, 2000 vectors |

## Gaps we close that competitors do not

- **Nearest-neighbour ranking (`top_k`).** Every calculator above compares exactly two vectors.
  The backlog row's headline use ("find the 5 nearest vectors to this query") needs a ranked list,
  so this block takes one `query` plus a newline-separated `vectors` list and returns the top *k*
  ranked by the chosen metric, with sort direction derived from the metric (similarity descending,
  distance ascending).
- **Labels.** `label: 1, 2, 3` lines survive into the ranking output, so a result is readable
  without counting rows.
- **Machine-readable output.** `output = table | json | csv` — competitors are HTML-only.
- **Precision control.** `decimals` (0–12); no competitor exposes one.
- **Chebyshev (L-inf).** Documented in the reference but implemented by none of the calculators.

## Out of model (listed, not built)

- **Text → vector embedding** (Tilores' string mode, "word embedding presets"): needs either a
  learned embedding model (no ML loader in gizza's pure-Rust/wasm runtime — same class as the
  skiplisted `embedding-export`) or a corpus-local bigram vectoriser whose scores are not
  transferable. Text similarity is already served by other blocks (`few-shot-text-classifier`,
  `fuzzy-name-matcher`).
- **Interactive 2-D vector dragging / angle visualisation** (simulations4all): needs an
  interactive canvas widget; gizza pages render declarative controls plus a text result.
- **ANN indexes (HNSW/IVF)**: this is an exact brute-force kNN over a pasted list, which is the
  right shape at the documented cap (2000 × 8192). Approximate indexes only pay off at corpus
  sizes that cannot be pasted into a page field.

## Verification notes

Worked example reused from tool 1 for cross-checkability: query `3, 2, 1` vs `1, 2, 3` gives
cosine `10/14 = 0.714286`, dot `10`, Euclidean `2.828427`, Manhattan `4` — matching that tool's
published numbers.
