# sentence-split — competitor analysis (2026-08-08)

Scan run BEFORE implementing, per the create-next-tool recipe. One web search
("online sentence splitter tool split text into sentences tokenizer") plus direct
reads of the top results. All notes are paraphrased observations of behaviour and
option sets — no competitor copy, branding, or trademarks are reproduced or reused.

## Competitors reviewed

| # | Tool | Shape | What it does |
|---|------|-------|--------------|
| 1 | Txtory "Sentence Extractor" (txtory.com/text-utilities/extract-sentences) | Browser text utility | Sentence segmentation with an explicitly advertised abbreviation/decimal/quote-aware boundary detector |
| 2 | Stanford CoreNLP `ssplit` annotator (stanfordnlp.github.io/CoreNLP/ssplit.html) | Library/reference implementation | The de-facto rule-based sentence splitter; documents the knob set the whole category copies |
| 3 | Online Text Tools "Split Text" (onlinetexttools.com/split-text) | Browser text utility | Generic delimiter/regex/length splitter with preset example chips and prefix/suffix/separator output controls |
| 4 | Crimsoni "Katana" English sentence segmenter (crimsoni.ai/katana) | Hosted ML prototype | Hybrid rules + CRF model trained on scientific text (checked as a 4th because webutility.io's splitter returned HTTP 403 and sentencesplitter.com returned HTTP 404 — replaced rather than running with fewer) |

## Table-stakes features observed → decision

| Capability | Seen in | In model? | Where it landed |
|---|---|---|---|
| Abbreviation-aware boundaries (`Mr.`, `Dr.`, `etc.`, `Inc.`, `e.g.`, `No.`) | 1, 2, 4 | yes | Core, always on: a built-in title/abbreviation table split into never-boundary and context-checked classes |
| Decimals / money / version numbers not split (`3.14`, `$99.99`, `1.2.3`) | 1 | yes | Core: a period between digits is never a boundary |
| Quoted/bracketed material kept intact (`"Stop!" he said.`) | 1 | yes | Core: terminator runs absorb trailing closing quotes/brackets; a following lowercase word suppresses the break |
| Lowercase-follower heuristic (period + lowercase word = mid-sentence) | 1 | yes | Core: uniform rule across `.`/`!`/`?`/`…` |
| Number the sentences | 1 | yes | `format = "numbered"` |
| Trim whitespace per sentence | 1 | yes | `trim` (default on) |
| Structured/JSON export with counts | 1 | yes | `format = "json"` — per-sentence index/text/word count/char count plus a total |
| Sentence count + per-sentence word count readout | 1 | yes | Included in the JSON format; the numbered/lines formats stay copy-paste clean |
| Newline policy (never / always / blank-line only) | 2 (`ssplit.newlineIsSentenceBreak` = never/always/two) | yes | `newlines = "paragraph" \| "never" \| "always"` |
| Minimum sentence length filter | 1 (short-fragment cleanup), common in the category | yes | `min_chars` (default 0 = keep everything) |
| Extensible abbreviation list for domain text | 2 (custom boundary regexes), 4 (scientific-text tuning) | yes | `extra_abbreviations` — comma/space separated, merged into the never-boundary set |
| Output separator styles (one per line / blank line between) | 1, 3 | yes | `format = "lines"` and `format = "blank-line"` |
| Preset example chips | 3 | yes | Four `[[example]]` chips on the page (abbreviations + decimals, dialogue, paragraph-per-line, JSON with a length filter) |
| Copy / download the result | 1, 3 | yes | Generic page affordance — `format = "text"` pages already get copy + download |
| Custom boundary regex / arbitrary delimiter splitting | 2, 3 | out of scope | Already covered by the existing `split-text` block (literal/whitespace/chars delimiters); duplicating it here would blur the tool |
| Keyword filter over the extracted sentences | 1 | out of scope | Line filtering is a separate concern; keeping this tool one job (segment) keeps output pipeable into a filter tool |
| CRF / ML segmentation model, scientific-text tuning | 4 | out of model | gizza blocks are pure Rust + ffmpeg, no ML weights. The rules cover common English prose; the doc states this limit plainly |
| Sentence *simplification* / rewriting long sentences | Sapling's utility (search result) | out of model | Requires a language model; not built |
| XML/HTML element boundaries, token-discard patterns | 2 | out of scope | Markup-aware segmentation belongs upstream of this tool (strip markup first) |
| Non-Latin terminators (`。`, `！`, `？`) | 2 (multilingual pipelines) | yes, cheap | Core recognises them as terminators alongside `.!?…` |
| Daily usage caps / paid tiers | 3, 4 | n/a | Not applicable — this runs locally in the browser with no cap |

Every observed table-stake is either implemented or listed above as out-of-scope /
out-of-model; nothing was silently dropped.

## Where this tool lands

- **Ahead on control:** three newline policies, an extensible abbreviation list, a
  minimum-length filter and four output shapes, all deep-linkable as `?param=` on the
  page and identically available from the CLI and the chat schema. The competitors that
  segment well expose a fixed rule set; the one that exposes a rich knob set is a library,
  not a tool page.
- **Ahead on privacy/limits:** runs entirely in the browser with no request cap, versus
  the hosted prototype's per-day segmentation limit and word-count bounds.
- **Behind on:** ML-grade disambiguation of genuinely ambiguous cases (an unknown
  abbreviation followed by a capitalised word). The page states this limit and offers
  `extra_abbreviations` as the escape hatch.

## Notes for future improvement

- If a demand for domain abbreviation packs appears (legal, medical, bibliographic),
  ship them as named presets rather than growing the built-in table.
- A `keep_terminator = false` option (strip the final punctuation) was considered and
  left out: no reviewed tool offers it and it makes round-tripping lossy.
