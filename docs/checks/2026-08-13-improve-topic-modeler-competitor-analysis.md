# topic-modeler — competitor analysis (2026-08-13)

Scan run **before** implementing `blocks/topic-modeler`, per
`.claude/skills/create-next-tool/SKILL.md` step 4. Everything below is **paraphrased** — no
competitor copy, branding, or trademarks are reproduced, and no competitor asset is used.

## Viability check (done first)

- **Duplicate scan.** `ls blocks/ | grep -iE 'topic|lda|keyword|cluster|text'` → nearest
  neighbours are `rake-keywords` (single-document keyphrase extraction, RAKE),
  `textrank-summarize` (extractive sentence summary, PageRank), `data-clusterer`
  (KMeans/DBSCAN/hierarchical over *numeric CSV columns*), `cluster-similar-values` (string
  fuzzy grouping) and `naive-bayes-text-classifier` (*supervised*, needs labels). None of
  them discovers latent topics across a **collection** of documents, and none is
  generative/unsupervised over a document–word matrix. `docs/tool-skiplist.txt` has no
  `topic`/`lda` entry. **Not a duplicate.**
- **Model fit.** LDA is a *statistical* model fitted at run time from the pasted corpus —
  it needs no pretrained weights, no tokenizer download and no ML runtime. Collapsed Gibbs
  sampling is integer counts + one multiply/add per topic per token, so it compiles to pure
  Rust with zero dependencies and runs identically on wasm32-wasip1 (chat/CLI) and
  wasm32-unknown-unknown (page). Determinism comes from an explicit seeded PRNG.
  **In model** — matches the backlog row's own note ("plain Rust→WASM: a Gibbs-sampled LDA
  implementation runs offline in the browser").

## Competitors reviewed

Four references were skimmed; the pressbooks walkthrough (403) and the Voyant docs page (502)
were unreachable, so they were replaced by the MALLET `train-topics` reference and the gensim
`LdaModel` API — the two engines nearly every hosted topic-modelling UI wraps.

| # | Tool | Shape | Notes |
|---|------|-------|-------|
| 1 | Lemmata (lemmata.app) | Browser-local topic modelling for humanities corpora | Closest competitor by *model* (in-browser, no upload). Uploads TXT/PDF/DOCX/ODT/EPUB or pasted text; slider-driven config; per-language stopword + POS filters; min_df/max_df vocabulary pruning; chunk size; reports topic word lists with frequencies, C_v coherence, perplexity/log-likelihood, topic maps/heatmaps/word clouds; exports CSV matrices, PNG/SVG, PDF, ZIP. |
| 2 | lettier/lda-topic-modeling | PureScript browser demo | Takes two or more documents and soft-clusters them into up to four topics. Minimal parameter surface; the point is the interactive document↔topic view. Confirms the "paste a few docs, see the mixture" interaction is the core job. |
| 3 | Topic Modeling Tool (MALLET GUI) | Desktop point-and-click wrapper | Exposes topic count, iteration count, stopword removal + **custom stopword file**, n-grams, HTML/punctuation stripping, regex tokenisation, alpha/beta optimisation, metadata columns, automatic segmentation. Outputs topic-keys and doc-topics tables. |
| 4 | MALLET `train-topics` / gensim `LdaModel` | The reference CLIs/APIs | MALLET: `--num-topics` (default 10), `--num-iterations`, `--num-top-words`, `--alpha`, `--beta`, `--optimize-interval`, `--random-seed`; outputs a topic-keys file (top *k* words per topic) and a doc-topics file (per-document composition). gensim: `num_topics` (default 100), `passes` (1), `alpha` (`symmetric` = 1/K), `eta`. |

## Table-stakes → decision

Every table-stake below ends in the descriptor **or** the out-of-model list; none is dropped
silently.

| Table-stake (seen in) | Fit | Where it landed |
|---|---|---|
| Multi-document corpus input, pasted | 1,2 | `documents` (multiline) + `separator` enum (`blank-line` / `line` / `dashes`) — covers "one doc per paragraph", "one doc per line", and explicit `---` fences |
| Number of topics | all | `topics`, default 5, 2–20, **slider** |
| Top words per topic | 3,4 | `words_per_topic`, default 8, 1–25, **slider** |
| Sampling iterations / passes | 3,4 | `iterations`, default 200, 50–1000, **slider** |
| Dirichlet priors α / β | 3,4 | `alpha` (0 = auto `50/K`, the MALLET convention) and `beta` (0.01) |
| Stopword removal | 1,3 | `remove_stopwords` boolean, default on (built-in English list) |
| Custom/extra stopwords | 1,3 | `stopwords` — comma-separated, merged with the built-in list |
| Minimum word length / vocabulary pruning | 1 | `min_word_length`, default 3, 1–12, **slider** (the in-model half of min_df/max_df) |
| Reproducible runs / random seed | 4 | `seed`, default 42 — every gizza surface must be deterministic anyway, so this is exposed rather than hidden |
| Topic-keys **and** doc-topics output | 3,4 | Report shows both: ranked topics with weights **and** each document's topic mixture |
| Machine-readable export (CSV matrices) | 1,3,4 | `output` enum: `report` / `json` / `csv` (csv = the doc-topic matrix, the shape MALLET's doc-topics file has) |
| Slider-driven config + presets | 1 | `kind = "slider"` on the four bounded numeric fields; four `[[example]]` preset chips |
| Stated limits | — | Caps (300 docs / 25,000 kept tokens / 20,000 vocabulary terms) are on the page, not just in code |

## Considered, not built (out of model or rejected)

- **File upload of PDF/DOCX/ODT/EPUB corpora** (Lemmata) — out of scope for a pure text
  block; gizza already ships `pdf-extract-text`, `docx-text-extract`, `epub-extract` and
  `doc-to-text`, so the composition is "extract → paste here". Noted on the page.
- **Coherence (C_v), perplexity, log-likelihood diagnostics** (Lemmata) — C_v needs a sliding
  reference corpus and PMI statistics that a single pasted corpus can't supply honestly;
  perplexity on the training corpus is a misleading number to surface to a non-specialist.
  *Considered, rejected* rather than shipped as a decorative metric.
- **Interactive topic maps / heatmaps / word clouds, PNG-SVG-PDF-ZIP export** (Lemmata) — the
  page driver renders one text or one media output, not a multi-artifact dashboard
  (`references/page-patterns.md`). The `csv` output covers the "get the matrix out" job.
- **Non-English stopword lists + POS filtering / lemmatisation** (Lemmata) — a per-language
  stopword table plus a POS tagger is a data/model dependency, not an algorithm. The
  `stopwords` field lets a user paste their own list for any language, which is the in-model
  half; the limitation is stated in the FAQ.
- **n-grams, regex tokenisation, HTML stripping, metadata columns, auto-segmentation**
  (MALLET GUI) — schema bloat for a browser tool; gizza has `html-to-text`,
  `regex-extract`, `chunk-text` and `split-text` for the same jobs upstream. *Considered,
  rejected.*
- **Hyperparameter optimisation (`--optimize-interval`, gensim `alpha='auto'`)** — asymmetric
  prior learning changes the result silently between runs of the same input and would make
  the page's recompute-on-input model confusing. Fixed priors, exposed, stay honest.
- **Cloud/batch corpora, accounts, saved projects** — outside gizza's browser-local,
  no-account model by definition.
