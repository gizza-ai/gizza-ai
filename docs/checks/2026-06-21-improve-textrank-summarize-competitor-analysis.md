# textrank-summarize — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/textrank-summarize` — extractive summary by selecting the
highest-ranked sentences with TextRank. Pure-Rust, no ML model. Pure-text input →
text output: chat + CLI + a page.

## What competitors do

- **LLM/abstractive summarizers** (ChatGPT, "AI summarizer" sites) — fluent
  paraphrased summaries, but they **send your text to a model/server**, can
  **hallucinate** facts not in the source, and are non-deterministic.
- **`sumy`, `gensim` (old), `summa`** — Python TextRank/LexRank libraries: solid
  and local, but require a Python environment and setup.
- **Browser "TextRank" demos** — exist, but quality and privacy vary.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm: chat Service
   Worker, CLI, and in-browser page. The text never leaves the device.
2. **Extractive + faithful.** The summary is composed of **verbatim sentences from
   the source**, in their original order — so it can't invent or distort facts the
   way an abstractive model can. Ideal when accuracy matters (legal, medical,
   notes).
3. **Deterministic, instant, model-free.** A graph PageRank over sentence
   similarity — same input → same summary, every time, with **no model download**
   and no inference cost.
4. **Tunable length.** Pick exactly how many sentences you want.
5. **Same everywhere.** Identical via chat, CLI (`gizza tool textrank-summarize
   text=… sentences=…`), and a `?text=…&sentences=…` page.

## Honest scope

- **Extractive, not abstractive** — it selects sentences, it does not rewrite or
  compress within a sentence; for a fluent paraphrase, an LLM is the better fit.
- **English-tuned** — a built-in English stopword list and `.?!` sentence
  splitting; other languages work but with reduced quality.
- Quality depends on the text having several reasonably-related sentences (it's a
  no-op passthrough when the text already has ≤ the requested number of sentences).

## Tests

5 core unit tests: sentence splitting (`.?!`); short text (≤ N sentences) returned
whole; selecting a **subset** where every returned sentence is verbatim from the
source and the count is ≤ N; **original order preserved** among the chosen
sentences; and empty input → empty. Plus the block drift-guard schema test. **CLI
verified** end-to-end (multi-sentence input → top-N summary). **Page** verified
with Playwright. `wafer build` instantiates the chat block.
