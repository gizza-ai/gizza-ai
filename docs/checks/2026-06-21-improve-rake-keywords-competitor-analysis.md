# rake-keywords — competitor analysis (2026-06-21)

## Tool
`rake-keywords` — extracts the top keywords/keyphrases from a document using the
RAKE (Rapid Automatic Keyword Extraction) algorithm. Pure-Rust, no model, no
network. Three surfaces verified: chat block (`wafer build` validates instantiation),
CLI (`gizza tool rake-keywords`), and the standalone page (Playwright).

## Surface verification
- **Chat / LLM API:** schema single-sourced from `descriptor()`; drift-guard unit test
  asserts the authored schema (`text` required, `top_n` default 10, `max_words` default 0).
  `wafer build` validates the block instantiates (331.8 KiB).
- **CLI:** `gizza tool rake-keywords text="…" top_n=5` returns the classic RAKE paper's
  expected ranking — `linear diophantine equations` (8.5) > `linear constraints` (4.5) >
  `natural numbers` / `strict inequations` / `nonstrict inequations` (4.0).
- **Page:** `/tools/rake-keywords/` with `text`, `top_n`, `max_words` fields; Playwright
  confirms the ranked keyphrase list renders.

## Competitors surveyed
1. **rake-nltk** (PyPI, Python) — reference implementation. Splits on a stopword list +
   punctuation, word score = degree/frequency, phrase score = sum of word scores, ranks
   desc. Configurable min/max phrase length and the metric (degree/frequency/ratio).
2. **RAKE-Keyword / u-prashant** (Python) — same core algorithm, returns scored phrases.
3. **rake-php-plus** (PHP) — string-in, scored-phrases-out; ships multiple language
   stopword lists.
4. **simplecodingtools.com keyword tool** — the main *online* RAKE tool: paste text →
   ranked keyword list in the browser.
5. **MATLAB Text Analytics `rakeKeywords`** — same scoring; lets you set the n-gram cap.

## Gap diff (fit-to-model)
| Capability | Competitors | gizza rake-keywords | Status |
|---|---|---|---|
| Core RAKE (degree/freq word score, summed phrase score, desc rank) | yes | yes | matched |
| Stopword + punctuation phrase splitting | yes | yes (NLTK-style embedded list) | matched |
| Limit number of results (top-N) | rake-nltk / php-plus | `top_n` (0 = all) | matched |
| Cap phrase length (max words) | rake-nltk min/max length, MATLAB n-gram | `max_words` (0 = no cap) | matched |
| Dedupe repeated phrases | implementations vary | yes (keep first/best score) | matched |
| Per-phrase score exposed | yes | yes (rounded to 3 dp) | matched |
| Privacy / no upload | online tool uploads to a server | runs locally in-browser / in CLI | **better** |
| Alternate scoring metric (frequency-only / ratio) | rake-nltk option | only degree/freq (the canonical default) | out-of-scope (kept the published default for determinism) |
| Multiple-language stopword lists | rake-php-plus | English only | out-of-scope (English doc focus; no extra dep) |

## Decisions
- Implemented the canonical RAKE scoring exactly (degree ÷ frequency word score, summed
  phrase score) — matches rake-nltk's default metric so results are comparable.
- Added `top_n` and `max_words` to cover the two most common competitor knobs (result
  count + n-gram cap) without pulling any dependency.
- Kept English-only stopwords and the single default metric to stay pure-Rust, fast, and
  deterministic; alternate metrics / multilingual lists are noted as out-of-scope, not built.
- No competitor copy, branding, or trademarks were used; the stopword list is the public
  NLTK-style English set.

## Sources
- https://pypi.org/project/rake-nltk/
- https://github.com/u-prashant/RAKE
- https://github.com/Donatello-za/rake-php-plus
- https://www.simplecodingtools.com/tool/keyword
- https://www.mathworks.com/help/textanalytics/ug/extract-keywords-from-documents-using-rake.html
