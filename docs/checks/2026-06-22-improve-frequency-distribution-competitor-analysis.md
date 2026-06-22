# frequency-distribution — competitor analysis (2026-06-22)

Tool: build a character, byte, or n-gram frequency table from text or hex, with
counts and percentages, ranked most→least frequent. Three surfaces verified:
chat block (`wafer build` OK), CLI (`gizza tool frequency-distribution …`), and
the in-browser page (`/tools/frequency-distribution/`, 3 Playwright tests green).

## Top competitors surveyed

1. **dCode — Frequency Analysis** (dcode.fr/frequency-analysis) — the richest.
   Unigram/bigram/trigram + custom-N; case/diacritic standardization; character
   class filters (letters / letters+digits / digits / custom / all±spaces);
   block vs sliding-window n-grams; count and percentage; histogram vs reference
   language models; cipher-cracking aids (substitution suggestion).
2. **onlinetoolz.net — Letter frequency** — letters/bigrams/trigrams; sort by
   frequency or alphabetical; count + percentage; optional special-char inclusion;
   "show top N" dropdown.
3. **Browserling — Character Frequency Counter** — paste→count letters, minimal.
4. **charactercounter.com — Letter Frequency Counter** — real-time 26-letter table,
   sort alpha / by percentage, bar charts + A–Z heat-map, CSV download.
5. **Boxentriq — Frequency Analysis** — unigram…n-gram for code-breaking, percentages.

## Capability diff vs gizza frequency-distribution

| Capability | Competitors | gizza (this tool) |
|---|---|---|
| Character frequency + percentage | all | ✅ |
| Ranked most→least frequent | all | ✅ (ties keep first-seen order) |
| Bigrams / trigrams / custom-N (sliding window) | dCode, onlinetoolz, Boxentriq | ✅ (`mode=ngram`, `n`) |
| **Raw byte frequency (`0xNN`)** | none seen | ✅ (differentiator) |
| **Hex-string input → byte/char analysis** | none seen | ✅ (differentiator) |
| Readable whitespace/control-char labels | partial | ✅ (`␠ (space)`, `\t`, `\n`, `\xNN`) |
| Case-insensitive grouping | dCode, charactercounter | ✅ (`case_sensitive=false`) |
| Distinct-symbol + grand-total summary | partial | ✅ |
| Runs fully client-side, no upload | some | ✅ (wasm; also CLI + chat JSON) |
| Structured JSON output (count + percent per symbol) | rare | ✅ (chat/CLI) |

## Gaps (ranked, fit-to-model)

Closed this pass:
- **Case-insensitive grouping** (`case_sensitive`, default true) — folds `A`/`a`
  before tallying in char/ngram modes; byte mode stays raw. Now matches dCode and
  charactercounter on this axis.

In-model, not yet built (candidate follow-ups, deliberately scoped out to keep this
a focused single-purpose tool — none block shipping):
- **Alphabetical / by-symbol sort toggle** — current order is frequency-desc with
  stable ties, which is the primary use case; an alpha sort is cosmetic.
- **Character-class filter** (letters-only / alnum / ignore-spaces) — in-model;
  users can pre-clean input today.

Out-of-model (NOT built, by design — gizza is pure-Rust + ffmpeg, browser-local):
- **Reference-language histograms / expected-frequency comparison** — needs bundled
  per-language frequency datasets; data-bundle scope, not a compute primitive.
- **Cipher-cracking aids** (substitution suggestion, transposition detection) — a
  separate cryptanalysis tool, out of scope for a frequency table.
- **Bar charts / heat-map visualization** — the page surface renders text output;
  charting is a different rendering mode.

No competitor copy, branding, or trademarks were used. The differentiators (byte
mode + hex input) extend the tool beyond the letter-only competitors.
