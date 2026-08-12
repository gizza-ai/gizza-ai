# persian-tokenizer competitor analysis (2026-08-12)

Backlog row: `persian-tokenizer` — splits Persian text into sentences and words, correctly handling ZWNJ-joined compounds and Persian punctuation.

## Competitor scan

Search query: "Persian tokenizer online Farsi text tokenizer ZWNJ sentence segmentation tool".

| Competitor / reference | Observed table-stakes | In gizza model? | Decision for this tool |
| --- | --- | --- | --- |
| Hazm Persian NLP Toolkit | Persian normalization, sentence tokenization, word tokenization, stemming, lemmatization, POS tagging and parsing. | Tokenization and light normalization fit. Stemming, lemmatization and tagging need a larger NLP library/model and are out of scope for a small deterministic wasm block. | Ship words/sentences/both modes, Arabic-to-Persian normalization, punctuation and ZWNJ handling. Defer linguistic analyzers. |
| Parsivar | Persian half-space correction, normalizer, word/sentence tokenizer, stemming, POS/chunk/dependency tools and spell checking. | Tokenization fits; automatic half-space correction, spell checking and parsers are outside the current pure rule-based gizza model. | Keep existing ZWNJ compounds intact by default and provide `split_zwnj`; do not infer missing half-spaces. |
| DadmaTools | Persian normalizer, tokenizer, lemmatizer, spellchecker and sentiment resources. | Lightweight tokenization fits. Sentiment and lemmatization depend on trained resources. | Provide deterministic in-browser preprocessing, with explicit out-of-model notes for ML/resource-heavy tasks. |
| Persian BPE tokenizer on Hugging Face | Subword/BPE vocabulary tokenizer for model inputs. | Not a fit: requires model/vocabulary assets and emits subwords, not human word/sentence tokens. | Do not implement BPE; keep this tool a word/sentence tokenizer. |
| parsitext Rust library | Rust Persian processing with normalization, ZWNJ-aware tokenization and entity recognition. | Conceptually fits, but adding a new engine crate requires wasm instantiation proof. The built implementation can cover the table-stakes directly with small rule-based code. | Implement a local rule-based tokenizer to avoid dependency risk and preserve predictable wasm behavior. |

## In-model feature set shipped

- Modes: `words`, `sentences`, `both`.
- Formats: `lines`, `numbered`, `space-separated`, `json`.
- Punctuation policies: `separate`, `attach`, `remove`.
- ZWNJ handling: keep half-space compounds by default; optional `split_zwnj=true`.
- Normalization: fold Arabic `ي/ك/ى/ة` to Persian forms, Arabic-Indic digits to Persian digits, strip harakat and tatweel.
- Entity preservation: URLs, emails, mentions, hashtags, dates and separator-bearing numbers stay whole by default.
- Newline modes: `paragraph`, `never`, `always`.
- 200,000-character cap for predictable wasm/browser memory use.

## Out-of-model or deferred

- Automatic half-space correction for text that lacks ZWNJ.
- Stemming, lemmatization, POS tagging, parsing and sentiment analysis.
- BPE/subword tokenization for LLM model vocabularies.
- Batch files and corpus-scale processing.

## Verification snapshot

- Core unit tests cover ZWNJ compounds, Persian punctuation, Persian/Arabic digits, URLs/emails/entities, normalization, newline modes, JSON output, exact cap boundary and invalid-option errors.
- The descriptor is schema-guarded in the block test.
