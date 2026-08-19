# sentence-tokenizer — competitor analysis (2026-08-13)

Scan run BEFORE implementing, per `create-next-tool` step 4. One web search
("online sentence tokenizer tool split text into sentences and word tokens with
character offsets"), then the top three reachable references were skimmed. All
notes below are paraphrased observations of *functionality*; no competitor copy,
branding or trademark is reproduced here or in the tool.

## Competitors reviewed

1. **MeFancy "Tokenize Text"** (`mefancy.com/textchange/tokenize-text`) — the
   closest browser-based, no-signup competitor.
   - Modes: words (simple), terms (smart / multi-word), sentences, paragraphs.
   - Output formats: one per line, JSON array, comma separated, pipe separated.
   - Toggles: lowercase, "clean" (strip punctuation).
   - Stats strip: lines, characters, token count.
   - Marketing angle: periods inside abbreviations must not split a sentence.
   - No character offsets, no per-token type.

2. **syntok** (`github.com/fnl/syntok`, the segtok successor) — rule-based
   Python tokenizer + segmenter, the reference implementation for
   "tokens carry offsets".
   - Every token exposes value, preceding spacing, and its offset in the
     original document; a separate `analyze` path preserves original offsets
     while `process` normalises.
   - Handles English negation contractions (`n't`), optionally rewriting them.
   - Keeps numeric tokens intact when they contain internal symbols
     (`2018-11-11`, `1,000.00`).
   - Segmenter: paragraph = two or more consecutive line breaks; abbreviation
     and decimal aware; terminators without a following space still segment.

3. **Stanford PTBTokenizer** (`nlp.stanford.edu/software/tokenizer.html`) — the
   long-standing NLP baseline whose option list defines "table stakes".
   - Records begin/end character positions per token ("invertible" mode also
     keeps whitespace so the input can be reconstructed).
   - `splitHyphenated` (default off), newline handling (`tokenizeNLs`,
     `tokenizePerLine`), `americanize`, `normalizeParentheses` /
     `ptb3Escaping` (`-LRB-`, `-RRB-`), `normalizeCurrency`, `untokenizable`.
   - Sentence splitting is deterministic: a terminator that is not absorbed
     into a token ends the sentence.

(SoMaJo and MorphAdorner were also surfaced by the search; both are
library/CLI-only and repeat the same feature set — offsets, abbreviation-aware
segmentation — so they did not change the requirement list.)

## Table stakes → decision

| Capability (seen at) | Decision |
| --- | --- |
| Sentence segmentation that survives `Dr.`, `e.g.`, `No. 5`, initials, decimals (all three) | **In model** — built-in never-split + number-prefix abbreviation lists, dotted-acronym and initial handling |
| Word/punctuation tokens (all three) | **In model** — token stream is the primary output |
| Character offsets per token (syntok, Stanford) | **In model** — `start`/`end` (0-based, end-exclusive, Unicode code points) on every token *and* every sentence; also the `table` format's whole reason to exist |
| Per-token type / class (implicit in all three) | **In model** — `word`, `number`, `punct`, `symbol`, `url`, `email` |
| Contraction splitting `don't` → `do` + `n't` (syntok, Stanford) | **In model** — `split_contractions`, default on (PTB convention) |
| Hyphenated compounds kept or split (Stanford `splitHyphenated`) | **In model** — `split_hyphenated`, default off (matches the Stanford default) |
| Numeric tokens with internal separators kept whole (syntok) | **In model** — number scanner keeps `1,000.00`, `2018-11-11`, `12:30`, `3/4`, plus ordinals (`3rd`) |
| Line-break policy (Stanford `tokenizeNLs`/`tokenizePerLine`, syntok paragraphs) | **In model** — `newlines` = `paragraph` (default) / `never` / `always`, same vocabulary as the existing sentence-split tool |
| Lowercase toggle (MeFancy) | **In model** — `lowercase`, default off; offsets still point at the original text |
| Strip punctuation / "clean" (MeFancy) | **In model** — `drop_punctuation`, default off (drops `punct` **and** `symbol` tokens) |
| Multiple output shapes: one-per-line, JSON, delimited (MeFancy) | **In model** — `format` = `json` (default) / `table` (TSV with offsets) / `lines` / `spaces` (PTB-style re-joined) / `sentences` |
| Counts / stats strip (MeFancy) | **In model** — `counts` object in the JSON output (sentences, tokens, words, numbers, punctuation, symbols, characters) |
| Domain-specific abbreviations (not offered by any of the three, but the sibling sentence-split tool has it and it is the #1 fix for a bad split) | **In model** — `extra_abbreviations` |
| URLs and e-mail addresses kept as single tokens (Stanford) | **In model** — dedicated `url` / `email` token types, which also stop their dots from splitting the sentence |

## Out of model (listed, not built)

- **Multi-word "smart" terms** (MeFancy's terms mode, e.g. treating a city name
  as one token) — needs a gazetteer/POS model; out of scope for a deterministic
  rule-based, dependency-free block.
- **Neural / statistical segmentation** (spaCy, Stanza, SoMaJo's ML paths) —
  needs a downloaded model; gizza blocks are pure Rust in a 64 MiB sandbox.
- **POS tagging, lemmatising, stop-word removal** — separate concerns; the repo
  already has `multilingual-stemmer`, `rake-keywords` and `text-corpus-cleaner`.
- **Sub-word / BPE LLM tokenisation** — a different meaning of "token";
  already covered by the existing `token-counter` block.
- **`americanize` spelling normalisation** (Stanford) — requires a
  British→American dictionary; a normalisation concern, not a tokenisation one.
- **PTB escaping** (`-LRB-`, `` `` ``/`''` quote rewriting) and
  `normalizeCurrency` (Stanford) — implementable, but they *mutate* token text
  away from the source, which fights this tool's offset-fidelity promise. Noted
  as a possible future opt-in flag rather than shipped now.
- **Whitespace/"invertible" reconstruction tokens** (Stanford) — unnecessary
  here: offsets plus the original text already allow exact reconstruction.

## Delta vs. the existing `sentence-split` block (dup check)

`blocks/sentence-split` renders *sentences only* (lines / numbered / blank-line
/ JSON with word+character counts) and exposes **no offsets and no token
layer** (`grep -c offset` over its core = 0). `sentence-tokenizer` is the
token-level tool: every token with its type and its character span, plus the
sentence span it belongs to. Different output contract, different audience
(NLP preprocessing / span alignment vs. reading-friendly sentence lists), so it
is not a duplicate. `persian-tokenizer` is language-specific (ZWNJ handling)
and `token-counter` is LLM sub-word counting — neither overlaps.

## UX patterns adopted

- Preset example chips (`[[example]]`) for each of the interesting modes —
  competitors all lead with clickable sample text.
- Friendly `[input.labels]` on every enum so the selects read as prose.
- `multiline = true` on the text field so pasted paragraphs keep their breaks
  (line breaks are load-bearing input for the `newlines` control).
- Placeholders on every text/number field; the counts object replaces the
  competitor "stats strip" without extra chrome.
