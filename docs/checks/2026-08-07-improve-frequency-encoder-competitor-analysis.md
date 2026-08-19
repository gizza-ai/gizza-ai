# frequency-encoder — competitor analysis (2026-08-07)

Scan run before finalizing the tool, per `create-next-tool` step 4. Notes are paraphrased from public documentation/search snippets; no competitor copy or branding is reused.

## Duplicate check

Existing adjacent gizza blocks were inspected before building:

- `target-mean-encoder` replaces categories with the mean of a separate numeric target column. That is supervised target/impact encoding and has leakage controls; it does not count category occurrence frequency.
- `frequency-distribution`, `word-frequency`, and `count-line-frequency` summarize value/text frequencies as reports. They do not transform a CSV column into a model-ready numeric feature and write the encoded CSV back out.
- `csv-stats` reports per-column statistics; it does not replace/append encoded feature columns.

`frequency-encoder` is therefore distinct: it is an unsupervised categorical feature transform for one CSV column.

## Competitors reviewed

| # | Tool/library | What it is | Reached |
|---|---|---|---|
| 1 | Feature-engine `CountFrequencyEncoder` | Python dataframe transformer for count/frequency encoding | Search/docs snippets |
| 2 | category_encoders `CountEncoder` | scikit-learn-compatible count encoder | Search result / public docs snippet |
| 3 | scikit-learn preprocessing ecosystem | ColumnTransformer pipelines with external encoders | Search result examples |
| 4 | Feature engineering tutorials/books | Worked examples of count/frequency encoding | Search result snippets |
| 5 | Generic CSV/data-prep encoders | Browser/CLI-style preprocessors | Feature summary scan |

## Table stakes → decision

| Capability | Seen in | Decision |
|---|---|---|
| Raw count encoding | Feature-engine, category_encoders | **built** — `mode=count` |
| Frequency/share encoding | Feature-engine | **built** — `mode=frequency` (0–1 share) |
| Percent output | Tutorial/UI convention | **built** — `mode=percent` |
| Log-scaled count | Common skew-handling pattern | **built** — `mode=log-count` |
| Choose encoded columns | all encoder libraries | **built** — one selected `column` by header or 1-based index |
| Replace original or append feature | data-prep UI convention; target-mean sibling | **built** — `output=replace|append` |
| Header and delimiter controls | CSV tool family table stakes | **built** — `has_header`, `delimiter=comma|tab|semicolon|pipe` |
| Missing/blank handling | encoder libraries expose missing-value choices | **built** — blank counted, NaN, or zero |
| Rare-category pooling | encoder libraries/tutorials often collapse rare values | **built** — `min_count` pools low-count levels |
| Case-insensitive grouping | CSV-cleaning UX pattern | **built** — `case_sensitive=false` groups case variants |
| Decimal rounding | frequency/percent/log outputs | **built** — `decimals` |

## Considered, not built (out of model or better elsewhere)

- Multi-column fitting/transforming with a persisted encoder object: gizza tools are stateless one-shot transforms, so the page/CLI encodes one selected column per run.
- Train/test leakage handling and out-of-fold encodings: those belong to supervised target encoding (`target-mean-encoder`), not count/frequency encoding.
- Automatic dataframe dtype detection: this tool takes CSV text and an explicit column selector; it does not infer object/category columns.
- Separate unseen-category policy: there is no separate fit/transform split, so every category in the pasted data is counted in that same run.
