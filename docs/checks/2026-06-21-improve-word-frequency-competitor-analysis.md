# word-frequency — competitor analysis (2026-06-21)

Tool: **gizza-ai/word-frequency** — counts how often each word occurs in a block
of text and ranks them most→least frequent, with count + percentage (keyword
density) per word. Pure-Rust, runs locally on all surfaces (chat/LLM, CLI, page);
nothing is uploaded.

## Surfaces verified

- **Chat block** — `wafer build` validates `target/block.wasm` (OK, 334 KiB);
  drift-guard schema test passes (descriptor == authored chat schema).
- **CLI** — `gizza tool word-frequency text=… [case_sensitive=…] [min_length=…]
  [ignore_stopwords=…] [top=…]` returns ranked JSON with count + percent.
- **Page** — `/tools/word-frequency/`, 2 Playwright tests pass (ranking +
  stopword/top-N options). Booleans render as checkboxes, integers as fields,
  text as a multiline textarea.

## Top competitors surveyed

1. **convertcase.net** — Word Frequency / Keyword Density report. Shows count +
   density %. Single + multi-word phrase modes.
2. **charactercounter.com/word-frequency-counter** — ranked list, totals for
   words/characters/sentences, case + stopword + min-length filters.
3. **browserling.com/tools/word-frequency** — client-side word occurrence count,
   case-insensitive option.
4. **textfixer.com word-frequency-counter** — ranked common words, excludes a
   built-in stop-word list.
5. **textground.com / codeshack.io / wordfrequency.org** — ranked lists; some add
   bigram/trigram phrase frequency, lemmas/POS tags, CSV/PDF export, word clouds.

## Gap analysis (fit to gizza's pure text-in / text-out model)

| Capability | Competitors | gizza word-frequency | Action |
|---|---|---|---|
| Ranked word frequency | all | yes | — |
| Count per word | all | yes | — |
| Case-sensitivity toggle | most | yes (`case_sensitive`, default off) | — |
| Stop-word removal | most | yes (`ignore_stopwords`, built-in EN list) | — |
| Minimum word length | some | yes (`min_length`) | — |
| Top-N limit | some | yes (`top`) | — |
| **Percentage / keyword density** | convertcase, charactercounter | **added this pass** (per-entry `percent`, shown in table + JSON) | **closed** |
| Distinct + total counts | some | yes (`distinct`, `total`) | — |
| Deterministic tie order | n/a | yes (first-seen order) | — |
| Unicode / contraction handling | varies | yes (alphanumerics + interior apostrophes; café, don't) | — |

### Out-of-model (intentionally not built)

- **Bigram/trigram phrase frequency** — a distinct feature; would belong in a
  separate `phrase-frequency` tool, not this single-word counter.
- **Lemmatization / part-of-speech tags** — needs an NLP model (out of gizza's
  pure-Rust + ffmpeg model).
- **Word cloud / bar-chart visualization** — the page output is text; image
  rendering is a different output shape.
- **File upload (.docx/.pdf) + CSV/PDF export** — gizza already has dedicated
  extract/convert tools; this tool stays text-in / text-out.

## Improvements applied this pass

- Added a **per-word percentage** (keyword density) to every entry — surfaced in
  the JSON (`percent`) and the page table (`count<TAB>pct%<TAB>word`). This was
  the one in-model capability gap vs. the keyword-density competitors. Updated
  the chat/manifest descriptions, page output label, and SEO copy to match.

No competitor copy, branding, or trademarks were used.
