# sentence-length-stats — competitor analysis (2026-08-23)

Scan run before implementing `blocks/sentence-length-stats`. All observations are
**paraphrased**; no competitor copy, wording, branding or assets were reused.

## Scope

The tool reports sentence count, average and maximum sentence length, and the
distribution of lengths for a block of pasted text. The competitive set is
"sentence length / sentence rhythm analyzers" — a distinct category from plain
sentence *counters* (which stop at a count) and from readability scorers (which
fold length into a grade formula).

## Competitors profiled

### 1. Content Powered — Sentence Length Checker
`https://www.contentpowered.com/tools/sentence-length-checker/`

- **Metrics:** average words/sentence, sentence count, word count, shortest and
  longest sentence, standard deviation, count of sentences over 35 words.
- **Distribution:** five fixed buckets — very short (<10), short (10–15),
  medium (15–25), long (25–35), very long (35+).
- **Extras:** a rhythm/variance score derived from how much adjacent lengths
  differ; per-paragraph average sentence length; a per-sentence list with each
  sentence's word count; flagging of 35+ word sentences and of short-sentence
  clustering.
- **UX:** paste box, explicit Analyze / Clear buttons, a bar chart of sentence
  lengths in document order, benchmark commentary against a 15–20 word target
  and against Flesch-Kincaid / Gunning Fog / Coleman-Liau.
- **FAQ:** ideal length, readability formulas, mobile reading, how to fix long
  and short sentences, SEO angle.

### 2. What Are Syllables — Sentence Length Analyzer
`https://whataresyllables.com/tool/sentence-length-analyzer`

- **Metrics:** sentence count, average sentence length, a "variability" index,
  longest and shortest sentence length, complex-sentence count, average word
  length in characters, a complexity index.
- **Options:** none — a single paste box and an Analyze button.
- **Output:** a flat report card of eight numbers; no distribution, no
  per-sentence list, no export.
- **Gap it leaves:** no histogram at all, and nothing configurable.

### 3. ProWritingAid — Sentence Length report
`https://prowritingaid.com/art/346/How-to-use...-The-Sentence-Length-Report.aspx`

- **Metrics:** a sentence-variety score computed from the standard deviation of
  lengths, average sentence length, and every individual sentence's length.
- **Visual:** a graph of sentence length across the document so runs of similar
  lengths are visible at a glance.
- **Benchmarks:** cites 11–18 words as the typical published-writing average;
  above that reads verbose, below it reads choppy.
- **Model mismatch:** account-gated desktop/web editor, not a paste-and-go page.

### 4. Entangled Text — Sentence Rhythm Analyzer (secondary reference)
`https://www.entangledtext.com/tools/sentence-rhythm`

- **Metrics:** average words/sentence, a length histogram, and a "monotony"
  percentage measuring how often adjacent sentences have near-identical lengths.
- **Guidance:** 12–18 words for commercial fiction; 5–10 for action passages.
- **UX:** local-only processing, sample text button, copy result, Ctrl+Enter to
  run.

## Table stakes → where each landed

| Capability | Competitors with it | Our decision |
|---|---|---|
| Sentence count | all | shipped |
| Average words/sentence | all | shipped |
| Longest / shortest sentence | 1, 2, 3 | shipped, with the sentence number |
| Median length | none | shipped (mean alone hides a skewed tail) |
| Standard deviation | 1, 3 | shipped |
| Variety / variability score | 1, 2, 3 | shipped — 0–100 from the coefficient of variation, with a plain-language band |
| Length distribution / histogram | 1, 3, 4 | shipped — five fixed buckets + ASCII bars |
| Long-sentence count | 1 | shipped, with a **configurable** threshold (competitors hard-code 35) |
| Monotony / adjacent-similarity | 1, 4 | shipped — share of adjacent pairs within 2 words |
| Per-sentence list | 1, 3 | shipped as a top-N **longest** list (full per-sentence dump is `sentence-split`'s job, which already emits per-sentence word counts) |
| Average word length (chars) | 2 | shipped — average characters per sentence is reported alongside words |
| Abbreviation-safe splitting | not documented by any | shipped — reuses the `sentence-split` rule-based detector (Dr., e.g., initials, decimals, list enumerators) plus a user-supplied extra-abbreviation list |
| Line-break handling control | none | shipped — `newlines` enum, so subtitle/list-shaped text measures correctly |
| Benchmark commentary | 1, 3, 4 | page copy states the 11–18 / 15–20 word bands; the tool itself stays numeric, not prescriptive |

## Considered, not built

- **Per-paragraph averages** (Content Powered) — real, but it makes the output a
  second table for a minority use; `text-statistics` already reports paragraph
  counts. Rejected on output-bloat grounds, not feasibility.
- **In-document bar chart / colour-highlighted source text** (1, 3) — the page
  renders a text result surface; an ASCII histogram carries the same
  information without a bespoke renderer.
- **Complexity index / clause counting** (2) — needs a parser we do not have,
  and `readability-score` already covers grade-level scoring.
- **Accounts, saved history, cloud sync** (3, 4) — out of model: gizza tools are
  browser-local with no account and no server.
- **AI-prose detection** (4) — a classifier claim we will not make from length
  statistics alone.
