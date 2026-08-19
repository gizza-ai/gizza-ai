# Competitor analysis — prose-linter (2026-08-14)

Tool: `prose-linter` — flags clichés, weasel words, passive voice, redundancies and related prose style issues using a local embedded rule set.

## Competitors skimmed

| Competitor / tool shape | Table-stakes capabilities observed | UX/control patterns | Fit decision |
| --- | --- | --- | --- |
| write-good / CLI-style prose linting | Detects weasel words, passive voice, adverbs, repeated words, lexical illusions, "so" starts, "there is" starts, and E-prime-style "to be" checks. Produces line/column findings and rule names. | Text input, named checks, readable output suitable for terminal/editor workflows. | In model. Implemented core rule set with line/column report, annotated output, JSON output, configurable checks, and opt-in E-prime. |
| ProseLint / style-guide linters | Flags clichés, corporate jargon, hedges, redundancies, uncomparable words, archaisms, and wordy phrases. Usually treats findings as suggestions rather than automatic rewrites. | Rule-category selection, suggestions/replacements, deterministic output. | In model. Added phrase lists with suggestions for jargon, wordy phrases and redundancies; documented heuristic limits. |
| Hemingway-style readability editors | Highlights hard-to-read sentences, adverbs, passive voice, and simpler alternatives, with visual severity and readability grades. | Large paste area, highlighted/annotated text, simple toggles, examples/presets. | Partly in model. Added long-sentence threshold and annotated output. Readability grade, color-coded rich editor, and automatic rewriting are out of model for this plain deterministic block/page. |
| LanguageTool/Grammarly-style grammar checkers | Spelling, grammar, punctuation, tone, fluency, and rewrite suggestions backed by large rule sets or models. | Rich editor, browser extensions, account/cloud features. | Out of model. This gizza block is pure Rust/local and avoids ML/cloud services; grammar/spelling/factual correctness are documented as limits. |

## Built requirements

- Paste-area text input with a 1,000,000 byte cap.
- `checks` control supports `all`, named rule subsets, `-rule` removals, `none`, and opt-in `eprime`.
- `output` enum supports `report`, `annotated`, and `json`.
- `ignore` allow-list suppresses approved words/phrases.
- `max_issues` caps listed findings without hiding the total.
- `long_sentence_words` controls the sentence-length heuristic; `0` disables it.
- Page examples cover a business draft, passive/weasel annotated output, and JSON with E-prime.

## Out-of-model / deliberately not built

- Spell checking, punctuation correction, grammar parsing, tone scoring, readability grade formulas, browser extension UX, accounts, cloud document storage, and automatic rewrites.
- Rich color overlays are reduced to deterministic `annotated` text with carets so the generic text page can verify exact output.

## Worked example used for checks

Input:

```text
So the cat was stolen at the end of the day.
```

Expected report includes `so-start`, `passive`, and `cliche` findings with one-based line/column positions.
