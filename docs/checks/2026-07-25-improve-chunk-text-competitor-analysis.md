# chunk-text — competitor analysis (2026-07-25)

Tool goal: split a long document into overlapping chunks by token or character
count for RAG / embedding pipelines.

## Competitor scan (3 real tools skimmed)

1. **RAG Chunker (developers.do/tools/rag-chunker)** — sizing: token-based
   (default) or character-based; modes: fixed-size, sentence-aware,
   paragraph-aware, hybrid. Defaults: token sizing, 350 tokens/chunk, 50-token
   overlap, gpt-tokenizer. Tokenizer picker (9 options, incl. character / word /
   sentence). Max recommended input ~200,000 characters. Sample-text load/clear.
   No output-format switch shown.
2. **Andergrove AI Text Chunker (andergrove.com/tools/text-chunker)** — unit:
   tokens (~4 chars/token estimate), characters, words, or sentences. Controls:
   unit, chunk size, overlap **percentage**. Output: JSON array, JSONL (one
   object per line), or downloadable `.jsonl`. Each record has `id`, chunk text,
   estimated token count. Browser-only, no upload. Recommends "a few hundred
   tokens" with 10–20% overlap.
3. **ChunkViz / LangChain RecursiveCharacterTextSplitter** — the de-facto RAG
   standard: `chunk_size` + `chunk_overlap`, recursively split on a separator
   hierarchy `["\n\n", "\n", " ", ""]` (paragraph → line → word → char) so a cut
   lands on the coarsest boundary that fits. Visualizes chunk boundaries and
   overlap.

## Table-stakes → where each lands

| Table-stake | Decision |
|---|---|
| Chunk size (numeric) | in-model — `chunk_size` param |
| Overlap between chunks | in-model — `overlap` param (absolute, same unit) |
| Size unit: characters / tokens / words | in-model — `unit` enum |
| Approx chars-per-token (no real browser tokenizer) | in-model — `chars_per_token` param |
| Boundary mode: hard / word / sentence / paragraph (≈ recursive splitting) | in-model — `boundary` enum with recursive fallback |
| Output format: JSON array / JSONL / plain | in-model — `format` enum |
| Per-chunk metadata (id, char + token count, offsets) | in-model — emitted in JSON/JSONL records |
| Trim chunk whitespace | in-model — `trim` boolean |
| Preset chips (e.g. RAG 512/64, small-chunk) | in-model — meta.toml `[[example]]` chips |
| Sample-text load / clear | out-of-model — page has its own field + examples; no "load sample" button in the generic generator |
| Downloadable `.jsonl` file | out-of-model — the generic page already exposes a Download link for `format = "text"` output |
| Real per-model BPE tokenizer (tiktoken, 9 tokenizers) | out-of-model — no real tokenizer runs in the browser wasm; we approximate via `chars_per_token` and say so (the separate `token-counter` tool does real BPE) |
| Semantic / embedding-based chunking | out-of-model — needs an ML model; gizza is pure-Rust |
| Live boundary visualization overlay | out-of-model — page renders text output, not an interactive highlight canvas |

## Design decisions

- **Overlap is absolute** (in the chosen unit), matching RAG Chunker's "50-token
  overlap". A percentage is a thin wrapper users can compute; absolute is the
  RAG-standard `chunk_overlap`.
- **`boundary`** maps the competitors' "modes": `character` (hard fixed cut),
  `word` (default — never split a word), `sentence`, `paragraph`. Coarser
  boundaries fall back to finer ones when none fits inside the budget — the
  RecursiveCharacterTextSplitter behavior.
- **`unit = tokens` default**, `chunk_size = 500`, `overlap = 50`
  (10% overlap) — squarely inside every competitor's recommended range, honest
  about the estimate.
- Records carry `id`, `text`, `chars`, `tokens` (estimate), `start`, `end`
  (character offsets) so downstream embedding code can cite spans.

No competitor copy, branding, or trademarks were reproduced; out-of-model items
are listed, not built.
