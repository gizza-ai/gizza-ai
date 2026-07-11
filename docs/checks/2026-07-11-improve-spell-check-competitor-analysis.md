# spell-check — competitor analysis (2026-07-11)

Scan of the leading free online spell checkers to set table-stakes for gizza's
`spell-check` tool. **No competitor copy, branding, or trademarks are reproduced
here — this is a paraphrased feature/UX summary used only to shape our own,
original tool.**

## Competitors scanned

1. **Grammarly — free spell checker** (grammarly.com/spell-checker) — paste text,
   inline red underlines on misspellings, click a word to pick a suggestion, one
   corrected result. Upsells grammar/style/AI-rewrite behind an account.
2. **Online-Spellcheck** (online-spellcheck.com) — paste or upload a document,
   highlights misspellings with a suggestion dropdown per word, multi-language,
   shows a corrected copy. Ad-supported, server-side.
3. **Reverso spell & grammar** (reverso.net/spell-checker) — paste text, spelling +
   grammar, context suggestions, several languages.
4. **QuillBot spell checker** (quillbot.com/spell-checker) — no sign-up, paste text,
   highlights errors and offers fixes, "fix all" corrected output.
5. **Punctuator / online spelling corrector** (punctuator.org/spelling) — paste
   text, per-word suggestions, corrected text out.

## Table-stakes → where each lands

| Capability | In our model? | Where |
|---|---|---|
| Detect misspelled words in pasted text | ✅ in-model | `core::check` tokenizer + dictionary membership |
| Per-word correction suggestions (ranked, several) | ✅ in-model | `suggest()` Damerau-Levenshtein + frequency rank; `max_suggestions` (1–20) |
| Fully corrected copy of the text ("fix all") | ✅ in-model | `Report.corrected` (top suggestion, casing preserved) |
| Count of errors / words checked | ✅ in-model | `misspelled_count`, `words_checked` |
| Ignore ACRONYMS (NASA, HTML) | ✅ in-model | `ignore_uppercase` (default on) |
| Ignore proper nouns / names | ✅ in-model | `ignore_capitalized` |
| Custom / personal dictionary | ✅ in-model | `custom_words` |
| Character offset of each error (for highlighting) | ✅ in-model | `Misspelling.offset` |
| Preserve `don't`-style apostrophes, skip digits/1-char | ✅ in-model | tokenizer rules |
| Runs privately, no sign-up, no upload to a server | ✅ in-model (advantage) | wasm, browser-local |

## Out-of-model (listed, not built)

gizza is browser-local pure Rust with no ML model, so these competitor features
are intentionally **not** built:

- **Grammar / punctuation / style checking** — needs a grammar model; we check
  spelling only and say so on the page.
- **Real-word errors** (e.g. *their* vs *there*, *affect* vs *effect*) — a
  correctly spelled word in the wrong place needs a language model / context.
- **Multi-language** — we ship an English (ASCII a–z) dictionary only.
- **AI rewrite / expand / simplify / tone** — out of scope for a spell checker.
- **Document upload (.docx/.pdf) parsing** — the page takes pasted text; other
  gizza tools handle file extraction.

## Decisions

- No new fixed-choice params, so no `Param::enumv` is warranted — all params are
  free text / numeric / boolean.
- Kept `max_suggestions` as the only numeric knob; rendered as a bounded
  **slider** (1–20) on the page since competitors let users widen the suggestion
  list.
- The corrected-text "fix all" output is our headline differentiator vs. the
  per-word-only free tiers, plus full privacy (nothing leaves the browser).
