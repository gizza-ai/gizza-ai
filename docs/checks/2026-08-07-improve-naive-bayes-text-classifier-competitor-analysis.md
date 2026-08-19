# naive-bayes-text-classifier — competitor analysis (2026-08-07)

Scan run before implementation. Notes are paraphrased observations; no competitor copy, branding, wording, or trademarks are reused.

Searches: `online naive bayes text classifier train examples classify text tool`, `scikit-learn naive bayes MultinomialNB BernoulliNB ComplementNB alpha fit predict_proba documentation`, and `online text classification naive bayes demo train text examples classify`.

## Competitors / references skimmed

### C1 — MetricGate multinomial Naive Bayes calculator/tutorial
- **Input pattern:** labelled examples and a text to classify; demonstrates the classical count-vector text-classification workflow.
- **Model:** multinomial Naive Bayes, bag-of-words counts, class probabilities via Bayes rule.
- **Options / defaults observed:** emphasis on smoothing and term counts; examples are small and educational rather than a production batch tool.
- **Output:** predicted class with probability-like scores and an explanation of the arithmetic.
- **UX pattern:** a compact form plus a worked example is the expected baseline.

### C2 — scikit-learn Naive Bayes documentation
- **Models:** multinomial, Bernoulli, complement, categorical, Gaussian; for text counts, multinomial and complement are the key fit.
- **Table-stakes parameters:** `alpha` smoothing, `fit_prior`/class priors, explicit class prior support, predicted probabilities, and complement Naive Bayes for imbalanced text data.
- **Output / API:** fit from labeled samples, then predict labels and probability estimates; exposes class counts / feature counts indirectly through the estimator.
- **Scope note:** incremental training and sparse matrices are in a Python runtime, not in this pure WASM block.

### C3 — educational text-classification tutorials (Raschka / Stanford / IBM-style examples)
- **Model fit:** tokenization, lowercasing, optional stop-word handling, word-count features, and n-grams are recurring expectations.
- **Core caveat:** smoothing is needed so unseen features do not create zero probabilities.
- **UX pattern:** simple spam/ham or topic examples are used to teach the model and make results checkable.
- **Output:** a predicted label plus the tokens that drove the decision helps users trust a tiny training set.

## Table stakes → decisions

| Table stake | Decision | Where it lands |
| --- | --- | --- |
| Paste labelled training examples | **in-model** | required `training_data` textarea, one `label<separator>text` per line |
| Paste text to classify | **in-model** | required `text` textarea |
| Auto-detect common label separators | **in-model** | `separator=auto`, with explicit tab/comma/pipe/colon choices |
| Multinomial naive Bayes | **in-model** | default `model=multinomial` |
| Bernoulli naive Bayes | **in-model** | `model=bernoulli`, presence/absence scoring |
| Complement naive Bayes | **in-model** | `model=complement` for imbalanced labels |
| Additive smoothing | **in-model** | `alpha`, default 1.0, min 0, max 10 |
| Case folding | **in-model** | `lowercase`, default true |
| Stop-word removal | **in-model** | `remove_stopwords`, default false |
| N-gram features | **in-model** | `ngram_max` 1..3 |
| Rare-token pruning | **in-model** | `min_count`, default 1 |
| Empirical vs uniform class priors | **in-model** | `priors=empirical|uniform` |
| Class probability list | **in-model** | `top_k`, with 0 meaning all labels |
| Explain influential tokens | **in-model** | `explain`, report and JSON |
| Machine-readable output | **in-model** | `output=json` |
| Batch-classify one line per row | **in-model** | `input_mode=lines` |
| Query-param deep links / presets | **in-model** | page examples and Playwright deep-link test |
| Sparse matrices / huge corpora | **out-of-model** | pure WASM tool caps training bytes/examples/vocab and trains from scratch each run |
| Pre-trained sentiment or topic models | **out-of-model** | would require model download / ML runtime; user supplies examples instead |
| Incremental `partial_fit` | **out-of-model** | no persisted model state in stateless gizza calls |
| Evaluation split, cross-validation, confusion matrix | **out-of-model** | useful but belongs to a broader ML evaluation tool; this classifier trains and predicts |

## Scope honesty

This tool is a small local classifier for exploratory labelling and explainable baselines. It is not a hosted ML platform, does not persist models, and does not download pre-trained transformers. It intentionally reports limits and small-sample warnings instead of implying production accuracy from a handful of examples.
