# text-statistics — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/text-statistics` — count words, characters, sentences,
paragraphs and lines, with reading/speaking time and averages. Pure-Rust. Chat +
CLI + a page.

## Relationship to `word-count`

`word-count` is a basic counter (words, characters, lines). `text-statistics` is
the **richer readability/stats** tool: it adds **sentences**, **paragraphs**,
**reading time**, **speaking time**, **average word length**, and **average words
per sentence**. Different job (estimate effort / readability), not just counting —
so it complements rather than duplicates `word-count`.

## What competitors do

- **Online word counters** (wordcounter.net, charactercountonline, etc.) — fast and
  popular; many add reading time. **Weakness: text is sent to / processed by an
  ad-heavy third-party page**, and features vary.
- **MS Word / Google Docs "Word count"** — built in, accurate, but inside a
  document editor; not scriptable or available for arbitrary pasted text in a CLI.
- **`wc`** — local and instant for words/chars/lines, but no sentences,
  paragraphs, or reading-time, and no Unicode-aware char counting.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm: chat Service
   Worker, CLI, and in-browser page. The text never leaves the device.
2. **More than a counter.** Sentences (`.?!` runs, collapsing `?!`/`...`),
   paragraphs (blank-line blocks), reading time (200 wpm) and speaking time
   (130 wpm), plus average word length and words/sentence — quick readability
   signals in one shot.
3. **Unicode-correct.** Characters are counted as Unicode scalar values (so emoji
   and accented letters count as one), and average word length ignores attached
   punctuation, unlike naive byte/`wc` counts.
4. **Structured + same everywhere.** Chat/CLI return a JSON object (each metric a
   field) an LLM or script can use directly; the page shows a readable summary.
   Identical via chat, CLI, and `?text=…`.

## Honest scope

- **Heuristic sentence/paragraph detection** — `.?!` terminators and blank-line
  paragraphs; abbreviations ("Dr.") or unusual layouts can shift the sentence
  count slightly. It's an estimate, like every reading-time tool.
- **English-pace reading/speaking estimates** (200 / 130 wpm); other languages or
  content types read at different rates.

## Tests

7 core unit tests: basic word/sentence/character (with + without spaces) counts;
paragraphs vs lines on a multi-line/multi-paragraph string; **repeated terminators
collapse** (`?!`, `...` count once); reading time (400 words → 2.0 min); averages
(word length ignores punctuation; words/sentence); empty input → all zero; and the
human-readable `summary`. Plus the block drift-guard schema test. **CLI verified**
end-to-end. **Page** verified with Playwright (paste text → stats). `wafer build`
instantiates the chat block.
