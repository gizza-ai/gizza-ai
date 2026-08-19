# multilingual-stemmer competitor analysis — 2026-08-13

## Scope

Tool: `multilingual-stemmer` — stem words in many languages via Snowball algorithms for search indexing, keyword normalization and text analysis.

Notes are paraphrased. No competitor copy, branding or interface assets were copied.

## Competitors reviewed

| Competitor / reference class | Table-stakes observed | In-model decisions for this tool | Out-of-model / not built |
| --- | --- | --- | --- |
| Snowball project demos and language reference pages | Snowball is the expected algorithm family; users expect many languages, deterministic stems, and examples where stems may not be dictionary words. | Use the pure-Rust `rust-stemmers` Snowball port; expose all supported languages as a fixed enum; explain that stems are not lemmas; add examples across English, German and Spanish. | Authoring or editing stemming algorithms is outside a small browser tool. |
| Online stemming / Porter stemmer pages | Paste text, pick language/algorithm, click stem; default output is often stemmed text or one word per line. | Provide a multiline text area, language select, and stemmed-text output that preserves punctuation and line breaks. | Server-side batch storage and API accounts are not built; the tool stays local/offline. |
| NLP preprocessing notebooks and search-index helpers | Analysts want vocabulary reduction, form-to-stem inspection, counts, and machine-readable output for pipelines. | Add output modes: unique stems, form→stem mapping, frequency table and JSON groups/statistics. | Full tokenization pipelines with stopword removal, n-grams, lemmatization or language detection are separate tools; they would expand the schema beyond a focused stemmer. |
| Multilingual SEO/keyword tools | Common UX patterns include examples/presets, language labels, simple copy/paste workflow and short warnings about language choice. | Add preset chips, friendly labels for each enum value, worked examples, FAQ, and a visible limit section. | Keyword volume, SERP metrics, ranking data and cloud projects require external services and are out of model. |

## Parameter and UX decisions

- `input` (required multiline): pasted words, sentences, queries or small corpora.
- `language` enum: 18 Snowball languages supported by `rust-stemmers`; default `english`.
- `output` enum: `text`, `stems`, `mapping`, `table`, `json` to cover both quick use and pipeline use.
- `min_length` integer slider: lets users protect abbreviations and short product codes.
- `lowercase` checkbox default true: Snowball algorithms work best on lowercase words, but case-sensitive workflows can turn it off.

## Fit-to-model assessment

The shipped features are pure Rust and deterministic, with no I/O, model download or network call. Automatic language detection, lemmatization, stopword lists and search-volume analytics were considered but not built: they need dictionaries/models or external data and would make this more than a focused Snowball stemmer.

## Verification plan

- Unit tests for multiple languages, all output modes, lowercase/min_length toggles and clear error messages.
- Descriptor drift guard and generated manifest sync.
- CLI exact-output checks for text/mapping/json/table and enum choices.
- Browser Playwright tests for real output, deep links, checkbox state and errors.
