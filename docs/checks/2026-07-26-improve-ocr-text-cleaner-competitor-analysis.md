# ocr-text-cleaner — competitor analysis (2026-07-26)

Scan done BEFORE implementing. Goal: a **deterministic, browser-local** cleaner for
the mechanical errors OCR engines produce (character confusables, ligatures, broken
spacing, hyphenated line breaks). All competitor copy paraphrased — nothing copied.

## Competitors skimmed

1. **ZentrixLabs OCR Correction** (zentrixlabs.net/ocrcorrection) — a rule-based
   engine (~600+ substitution patterns) with categories: pipe→I, capital-I/lowercase-l
   confusion (`HeIIo`→`Hello`), spacing (`th e`→`the`, `Thanks.Next`→`Thanks. Next`),
   apostrophes, letter↔number (`l00`→`100`, context-aware `0`↔`O`). Config toggles:
   detailed log, excluded categories, context-aware capitalization (experimental, off),
   processing passes (Quick/Standard/Thorough).
2. **OCR-Software.com Smart Text Cleanup** — line-break handling (remove mid-sentence
   breaks, keep paragraph breaks, dehyphenate across line boundaries), spacing
   normalization (collapse multiple spaces, trailing-space removal), structure detection
   (lists/headings/code). Four modes: Smart (AI), Clean-for-Email (strip all breaks),
   Clean-for-Word (join within paragraphs), Preserve-All (minimal). Before/after preview,
   copy/download.
3. **ImageToTexts "Understanding OCR errors"** (guide) — documents the canonical
   substitution classes: `O`↔`0`, `l`↔`1`, `rn`↔`m`, merged/broken words, missing
   punctuation, ligatures, deskew/preprocessing. Fixes: preprocessing, regex correction,
   spell-check, AI re-rank.

(A fourth candidate, yeschat "OCR Fixer", is a wrapper around a chat LLM — no concrete
deterministic feature list, so replaced by the three above for the table-stakes matrix.)

## Table-stakes matrix (each tagged in-model / out-of-model)

| Feature | Decision |
| --- | --- |
| Char confusables `0↔O`, `1↔l/I`, `\|→I` by word/number context | **in-model** → `fix_confusables` param |
| Ligature expansion `ﬁ ﬂ ﬀ ﬃ ﬄ ﬆ` → `fi fl ff ffi ffl st` | **in-model** → `fix_ligatures` param |
| Dehyphenation of words split at line breaks (`hyphen-\nation`→`hyphenation`) | **in-model** → `join_hyphenated` param |
| Line-break handling: keep / join paragraphs / join all | **in-model** → `line_breaks` enum |
| Spacing: collapse runs, strip space-before-punct, space-after-punct, trailing | **in-model** → `fix_spacing` param |
| `rn→m` OCR merge error | **in-model but AGGRESSIVE** → opt-in `fix_rn` (default off); context-safe version is out-of-model |
| Before/after preview, copy/download | **in-model** — the generic page gives live output + Copy button (platform) |
| Preset chips (modes) | **in-model** — `[[example]]` preset chips for common OCR profiles |

## Out-of-model (listed, NOT built — need a dictionary / language model)

- Missing-space insertion (`thejob`→`the job`) and compound splitting (`prettylucky`→`pretty lucky`) — needs a dictionary.
- Missing-apostrophe insertion (`dont`→`don't`) — needs a lexicon.
- Context-aware, safe `rn→m` (apply only where a real word results) — needs a dictionary; our `fix_rn` is the blind opt-in version.
- Spell-correction / AI re-ranking / context-aware capitalization.
- Confidence scoring, batch/cloud processing, accounts.
- Image preprocessing / deskew (that's the OCR step, upstream of text cleanup).

## UX controls to match

- Toggles per fix category (all competitors expose category on/off) → boolean params → checkboxes.
- Mode/preset selector → `line_breaks` `<select>` + `[[example]]` preset chips.
- Live before→after → generic page renders output live; Copy button is platform-provided.

Smart-quotes / curly-quote straightening is intentionally **left out** — already shipped as
`blocks/smart-quotes-clean` (avoids a semantic dup). This tool focuses on OCR-specific
mechanical errors.
