# cefr-level — competitor analysis (2026-08-29)

Scan run **before** implementing, so the descriptor could ship the table-stakes from day one.
Everything below is **paraphrased** from public tool pages — no competitor copy, branding, or
trademarks are reproduced anywhere in this repo.

## Competitors reviewed

### 1. Cathoven CEFR Checker — https://www.cathoven.com/cefr-checker/
- **Features:** overall CEFR band for a pasted text plus five sub-scores (vocabulary, tense,
  clause, sentence, and a word-by-word classification). Colour-codes every word by its band.
- **Params/options:** text box; a reading-vs-listening text-type switch. No target-level selector
  in the free widget.
- **Output:** decimal sub-levels (e.g. a high A2 renders as `A2.7`), with sentinel markers for
  "below A1" and "above C2 / unrecognised". Results are shareable/downloadable.
- **Limits:** 250 words in the free widget; 3 analyses without an account, more behind a free
  login, full-length texts behind the paid hub.
- **Free vs paid:** freemium, account-gated after a small quota.

### 2. CVLA — CEFR-based Vocabulary Level Analyzer — http://cvla.langedu.jp/
- **Features:** research-grade CEFR-J estimate from eight textual measures (abbreviated on the
  page as AVR, BPER, ARI, VperSent and others), trimmed to six by dropping the min and max before
  the regression, which stabilises the estimate.
- **Params/options:** reading-passage vs listening-monologue mode (listening scores are rescaled
  by a published linear adjustment); paste-text mode or batch `.txt` upload.
- **Limits:** 10–1000 words per text; up to 30 UTF-8 files of 10 KB each; results are sensitive to
  line-break formatting.
- **Output quality:** self-reported ~63% exact-band agreement, 100% agreement within one band.
  That honesty about accuracy is a UX pattern worth copying (the idea, not the words).

### 3. cefrlevels.com Text Analyzer — https://www.cefrlevels.com/textanalysis/index.html
- **Features:** difficulty band derived from a 10,000-word frequency list, combining average word
  rank with average word and sentence length; also emits a suggested vocabulary list with
  dictionary links.
- **Params/options:** just a textarea and a submit button.
- **Limits:** states results get more reliable above roughly 50 words; no hard cap published.
- **Output:** single band + word list. No per-band percentages, no highlighting.

### 4. Francesca La Russa CEFR Vocabulary Checker — https://www.francescalarussa.com/cefr-vocabulary-checker/
(Used in place of Text Inspector and LingoHarvest, which both returned HTTP 403 to the scan.)
- **Features:** target-level workflow — pick the level you are writing *for*, get the words that
  exceed it, with simpler synonyms and broader-term fallbacks per flagged word.
- **Params/options:** textarea (10,000-character cap) + a target-level dropdown A1–C2; buttons to
  re-run, undo a replacement, and restore the sample text.
- **Output:** total words, recognised forms, count over target, and count unclassified; per-word
  lemma + part of speech + band. Deliberately does **not** publish an overall text level.
- **Stated limits:** lexical accessibility only (no grammar/discourse), unclassified forms are
  shown rather than guessed, not a proficiency certification.

Text Inspector (`textinspector.com`) and LingoHarvest were both unreachable (403) from this
environment, so they were replaced per the scan rules rather than run with fewer competitors.

## Table stakes extracted, and what we did with each

| Table stake | Seen at | Verdict | Where it landed |
| --- | --- | --- | --- |
| Overall A1–C2 band for the whole text | all four | in-model | headline of every output format |
| Decimal sub-level (`B1.4`) | Cathoven | in-model | headline + `sublevel` in JSON |
| Per-word band classification | Cathoven, La Russa | in-model | `word_list` param + `annotated` output format |
| Per-band share of running words | Cathoven, LingoHarvest blurb | in-model | vocabulary-profile table with cumulative % |
| Target level + "words above target" | La Russa | in-model | `target` param, over-target count/%/list |
| Separate vocabulary vs grammar/sentence scores | Cathoven, CVLA | in-model | both reported, plus the sentence metrics behind them |
| Transparent level rule instead of a black box | CVLA | in-model | `coverage` param (the cumulative-coverage threshold) is user-visible and tunable |
| Honest handling of unrecognised words | CVLA, La Russa | in-model | `unknown` param: estimate / force C1 / force C2 / exclude |
| Proper nouns shouldn't inflate difficulty | implied by La Russa's "unclassified" bucket | in-model | `proper_nouns` param, excluded by default |
| Machine-readable export | Cathoven (download/share) | in-model | `output = json`, plus markdown `table` |
| Stated accuracy limits | CVLA | in-model | limits section on the page + FAQ |
| Sample/example text to try | La Russa | in-model | three `[[example]]` preset chips |
| Word-count cap messaging | Cathoven 250, CVLA 1000, La Russa 10k chars | in-model | 200,000-character / 40,000-word cap, stated on the page; no quota, no login |

## Considered, not built (out of model or rejected)

- **Synonym / hypernym replacement suggestions** (La Russa) — needs an embedded WordNet-scale
  thesaurus with sense disambiguation. Out of model for a single wasm block today; listed here so
  it isn't silently dropped.
- **Part-of-speech tagging and tense/clause sub-scores** (Cathoven) — needs a real POS tagger;
  the clause signal we *can* compute cheaply (subordinator density) is folded into the
  grammar/sentence score instead of being advertised as a tense analyser we don't have.
- **Accounts, quotas, shareable result links, file batch upload** (Cathoven, CVLA) — server-side
  by definition; gizza tools are browser-local and account-free, which is the positioning angle.
- **Listening-monologue rescaling** (CVLA) — rejected on judgement: their published adjustment is
  fitted to their own regression, so copying the *shape* of it would be a made-up number here.
- **Non-English languages** — the lexicon is English-only; stated as a limit rather than faked.

## Design decisions taken from the scan

1. **Original lexicon, no third-party word list.** Published CEFR wordlists are licensed assets, so
   the block ships its own hand-built banded lexicon (A1–C1) plus morphology (inflection stripping,
   contractions, hyphenates, derivational suffixes that bump a band) and a length/syllable/
   academic-suffix heuristic for anything outside it. Stated plainly on the page.
2. **Show the rule, don't hide it.** Competitors give a band with no visible criterion; `coverage`
   (default 90%) makes the "smallest band that covers this share of running words" rule explicit
   and adjustable, which is also what makes the result reproducible.
3. **Both halves of difficulty.** Vocabulary alone under-rates dense syntax, so a grammar/sentence
   score (sentence length, subordinator density, word length) is reported next to it and weighted
   into the headline (0.65 vocab / 0.35 grammar).
4. **Target-level workflow is a first-class param**, not a separate tool — it is the single most
   actionable thing a teacher does with this analysis.
