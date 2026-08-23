# Competitor analysis — few-shot-text-classifier (2026-08-23)

## Scope

Tool: `few-shot-text-classifier` — classify text by similarity to a user-supplied support set of labeled examples. The gizza model is pure Rust/WebAssembly with no network calls, model downloads, persistent training, or external inference.

## Competitors scanned

- scikit-learn documentation/examples for text classification and nearest-neighbor/TF-IDF workflows.
- Hugging Face zero-shot and few-shot classification examples and hosted demos.
- MonkeyLearn / Levity-style no-code text classification products.
- Spreadsheet/tutorial workflows that classify text with small labeled samples using TF-IDF or nearest neighbors.

The scan was used to identify common controls and expectations; implementation and copy are original and constrained to the local deterministic gizza model.

## Table-stakes capabilities

| Capability / UX expectation | In model? | Decision |
| --- | --- | --- |
| Paste labeled examples and classify arbitrary text | Yes | Required `examples` and `text` params. |
| Support multiple label/text separators | Yes | `separator` enum: auto, tab, comma, pipe, colon. |
| Batch classify many rows | Yes | `input_mode=lines`, CSV/JSON/report outputs. |
| Confidence or score per label | Yes | Report confidence and similarity per label; `top_k` controls score table. |
| k-nearest-neighbor style scoring | Yes | `method=knn` with `k` boundary 1-50. |
| Prototype/centroid scoring | Yes | Default `method=centroid`. |
| Single best training example explanation | Yes | `method=best-match`; all methods report nearest example when explain is on. |
| Text preprocessing controls | Yes | Lowercase, accent stripping, stop words, sublinear TF, min document frequency. |
| Word and character n-grams | Yes | `analyzer` + `ngram_max` options. |
| Multiple similarity metrics | Yes | Cosine, dot, inverted Euclidean, Jaccard. |
| Machine-readable output | Yes | JSON and CSV outputs. |
| UI presets/examples | Yes | Page example chips for support tickets, feedback batch, and typo-tolerant short text. |
| Uncertainty threshold | Yes | `min_confidence` reports `uncertain` below threshold. |
| Semantic embeddings or transformer inference | No | Out-of-model: would require a model download or remote API. Documented limitation. |
| Saved trained classifier / model export | No | Out-of-model: this tool is stateless per run. |
| Active learning, labeling queue, team workflow | No | Product workflow, not a pure block. |
| Evaluation on held-out datasets | Partial | Users can run batch mode externally, but no separate train/test UI is built. |

## Defaults chosen

- `method=centroid` because it is stable for a handful of examples per label.
- `similarity=cosine` because it compares documents of uneven length better than raw dot product.
- `weighting=tfidf` because it suppresses terms common to every example.
- `analyzer=word`, `ngram_max=1`, `lowercase=true` as the least surprising defaults for English-like short text.
- `explain=true` because competitor tools usually provide either highlighted words, nearest examples, or score evidence.
- `min_confidence=0.0` to preserve a direct top-label prediction unless the user explicitly asks for an uncertainty floor.

## Fit gaps left out intentionally

- No external language model or embedding model. This keeps the tool deterministic, offline, and small enough for WebAssembly.
- No persistent training set storage. The support set is user input, not account state.
- No automatic data cleaning beyond lexical normalization options.
- No vendor-specific labels, benchmarks, or copy were imported.
