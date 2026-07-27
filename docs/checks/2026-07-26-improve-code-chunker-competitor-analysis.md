# code-chunker — competitor analysis (2026-07-26)

Function: split source code into function- / class-aligned chunks suitable for
embedding or feeding an LLM context window (RAG over code). Paraphrased notes
only — no competitor copy, branding, or trademarks reproduced.

## Competitors skimmed (paraphrased)

1. **A popular LLM-framework "language-aware text splitter"** — offers a
   recursive character splitter seeded with per-language separators (e.g. it
   knows the keyword patterns that begin functions/classes for ~20 languages and
   prefers to break there). Table-stakes: pick the language; a size budget; it
   keeps a whole construct together when it fits and only falls back to a smaller
   break when a construct exceeds the budget. Size is measured in characters
   (with an optional token estimate).

2. **A retrieval-tool that uses a parser-generator's grammars** — parses each
   file into a syntax tree and emits one chunk per top-level definition
   (function, class, method), attaching the leading doc comment to the
   definition it documents, and merging tiny adjacent definitions up to a size
   cap. Reports the start/end line of every chunk and the construct's
   name/kind. Big definitions are emitted whole (or optionally sub-split).

3. **A general "code splitter" utility in an embeddings toolkit** — line-based:
   groups consecutive top-level blocks into chunks under a max-lines cap, never
   cutting inside a block; outputs chunk text plus line ranges; supports several
   output shapes (array of records / newline-delimited).

## Table-stakes → decision (in-model unless noted)

| Capability | Decision |
|---|---|
| Choose the language (or auto-detect) | **in-model** — `language` enum (auto + Python, JS, TS, Rust, Go, Java, C, C++, C#, PHP, Swift). |
| Size budget per chunk | **in-model** — `max_lines` (line-based; honest — no real tokenizer runs in wasm). |
| Keep a construct whole when it fits; group small ones | **in-model** — greedy packer up to `max_lines`. |
| Never split inside a definition | **in-model** — an oversize definition is emitted whole and flagged `oversize`. |
| Attach the leading comment/decorator to its definition | **in-model** — leading comments/decorators fold into the following unit. |
| Report line ranges + construct name/kind | **in-model** — every record carries `start_line`, `end_line`, `line_count`, `kind`, `name`. |
| Output shapes (records / ndjson / text) | **in-model** — `format` enum json / jsonl / text. |
| Real parse tree via a parser-generator's per-language grammars | **out-of-model** — those grammars are C libraries loaded at runtime; they do not build/instantiate in the pure-Rust wafer wasm sandbox. We use a bracket-depth + indentation heuristic instead (documented limit). |
| Character/token size budget | **out-of-model as a unit** — no BPE tokenizer in the browser wasm (see the `token-counter` tool); we measure in lines, which is deterministic and transparent for code. |
| Recursively sub-splitting an oversize function into smaller pieces | **out-of-model (by design)** — the tool's premise is keeping definitions intact; oversize units stay whole and are flagged so callers can decide. |

## UX / controls competitors ship → decision

- Language dropdown → `<select>` (enum param). **done**
- Size number box → number field. **done**
- Preset buttons (e.g. "Python", "small chunks") → `[[example]]` preset chips. **done** (3 chips).
- Output-format toggle → `<select>`. **done**

## Stated limits (also in the page copy)

- Heuristic, not a full parser: boundaries come from bracket balancing (for
  brace languages) and indentation (for Python), so unusual macros, here-docs,
  or code inside multi-line strings can occasionally be mis-attributed.
- Names/kinds are best-effort (e.g. a Go method reports its receiver, not the
  method name); chunk *boundaries* do not depend on the extracted name.
- Chunks do not overlap (structural chunking); use the `chunk-text` tool if you
  want sliding-window overlap.
