# text-corpus-cleaner — competitor analysis (2026-07-24)

Tool function: clean a raw, line-oriented text corpus (one record/sentence/word per line) in one
deterministic browser-local pass — Unicode normalization, whitespace handling, lowercase,
per-line length/symbol/language filters, deduplication, and blank-line collapsing. All notes
below are **paraphrased** — no competitor copy, branding, or trademarks reproduced.

## Landscape

There is no single dominant "clean my text corpus online" page; the de-facto references are the
Python preprocessing libraries practitioners reach for, plus the deduplication step every LLM
data-prep pipeline runs. Those libraries define the parameter/feature surface any corpus cleaner
is judged against.

### 1. `clean-text` (jfilter/clean-text — paraphrased)
- **Features:** wraps `ftfy` + `unidecode` + hand-written regex; fixes Unicode, lowercases,
  strips/normalizes whitespace, and can remove URLs, emails, phone numbers, currency symbols,
  numbers, and punctuation, optionally transliterating to ASCII.
- **Params/defaults (paraphrased):** `fix_unicode=True`, `to_ascii=True`, `lower=True`,
  `no_line_breaks=False`, `normalize_whitespace=True`, plus per-entity `no_urls/no_emails/...`
  toggles with replacement tokens. Exposes a scikit-learn-compatible API for pipelines.

### 2. `ftfy` — "fixes text for you" (paraphrased)
- **Features:** repairs mojibake and encoding damage, normalizes Unicode (NFC/NFKC), fixes smart
  quotes/ligatures, removes invisible control characters, and standardizes multiple
  representations of the same character.
- **Reference behavior:** Unicode normalization form is the key knob — NFC vs NFKC changes whether
  ligatures/full-width/styled forms are folded. This is the direct analog of our `unicode_form`.

### 3. Dedup + whitespace tutorials (GeeksforGeeks, Towards Data Science, Pybites — paraphrased)
- **Features:** lowercase, `strip()`/`" ".join(split())` whitespace collapsing, dropping empty
  and near-empty lines, and removing exact duplicate lines/records (commonly `set` or
  `dict.fromkeys` to keep first occurrence). LLM-scale pipelines add fuzzy/normalized dedup and
  language filtering (e.g. fastText/whatlang-style detection) before training.
- **UX patterns worth matching:** keep-first duplicate removal, a length/quality threshold to drop
  junk lines, and single-language filtering for multilingual scrapes.

## Table-stakes → decision (every one lands in the descriptor OR the out-of-model list)

| Table-stake | Decision | Where |
| --- | --- | --- |
| Unicode normalization NFC/NFKC (and off) | in-model | `unicode_form` enum |
| Trim / collapse interior whitespace | in-model | `whitespace` enum |
| Lowercase | in-model | `lowercase` bool |
| Exact duplicate-line removal (keep first) | in-model | `dedupe=exact` |
| Fuzzy/normalized dedup (case + spacing folded) | in-model | `dedupe=normalized` |
| Drop too-short lines (chars / words) | in-model | `min_chars`, `min_words` |
| Drop mostly-symbol / junk lines | in-model | `max_symbol_ratio` |
| Single-language filter (no model files) | in-model | `language` enum (whatlang) |
| Collapse blank-line runs, trim edges | in-model | `collapse_blank` bool |
| Split on `\n` / `\r\n` / `\r` | in-model (documented behavior) | core split step |

### Out-of-model / considered, not built
- **Entity stripping (URLs / emails / phone numbers / currency) with replacement tokens** —
  clean-text's regex entity removal is a large, locale-sensitive surface and shifts the tool from
  "line cleaner" to "PII/entity scrubber". Listed, not built; a dedicated tool fits better.
- **ASCII transliteration (`unidecode`)** — lossy and language-specific; NFKC covers the common
  compatibility-folding case without romanizing scripts. Listed, not built.
- **Mojibake / encoding repair (ftfy's headline feature)** — needs heuristic re-decoding of
  mis-decoded bytes; disproportionate scope and risk for a deterministic per-line pass. Listed,
  not built.
- **Stopword removal / stemming / tokenization** — that's downstream NLP, not corpus hygiene, and
  is strongly language-dependent. Out of model.

## Design outcome
Descriptor params: `input`, `unicode_form` (enum), `whitespace` (enum), `lowercase` (bool),
`dedupe` (enum), `language` (enum), `min_chars` (int), `min_words` (int), `max_symbol_ratio`
(number), `collapse_blank` (bool). The tool matches the whitespace/lowercase/Unicode-form/dedup/
length/language table-stakes of clean-text + ftfy + the dedup tutorials, while deliberately
leaving entity scrubbing, transliteration, and mojibake repair to separate tools so the output
stays a clean, same-shape, line-oriented corpus.

Sources (paraphrased only): [clean-text](https://github.com/jfilter/clean-text),
[ftfy](https://usefuldatatips.com/tools/data-cleaning/ftfy),
[Efficient Text Data Cleaning — GeeksforGeeks](https://www.geeksforgeeks.org/python-efficient-text-data-cleaning/).
