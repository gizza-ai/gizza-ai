# text-normalizer — competitor analysis (2026-07-17)

Tool function: clean and normalize text — Unicode normalization (NFC/NFD/NFKC/NFKD),
case folding, accent/diacritic stripping, whitespace collapsing, and punctuation
normalization — as one configurable pass. Runs entirely client-side (wasm), like the
rest of the gizza toolkit.

## Competitors scanned (top real tools)

1. **EasyProTools — Text Normalizer** (easyprotools.com/text/text-normalizer) — ships an
   "NLP Prepare" preset that chains: lowercase, remove punctuation, remove numbers,
   remove accents, trim whitespace, normalize Unicode. Also collapses multiple spaces to
   one. Preset buttons for common workflows.
2. **Kovertiz — Unicode Text Normalizer** (kovertiz.com/tools/unicode-normalization-tool)
   — the four canonical forms NFC/NFD/NFKC/NFKD, 100% in-browser.
3. **ToolsGod — Normalize Unicode** (toolsgod.com/normalize-unicode) — forms plus
   "Strip Accents" (accented letter → base form) and "Flatten Fancy Fonts" (NFKC mapping
   of styled-letter code points).
4. **Online Text Tools — Remove Diacritics** (onlinetexttools.com/remove-text-diacritics)
   — dedicated diacritic stripping; sibling tools for whitespace clearing and lowercasing.
5. **CodeShack / TextCleaner** — remove accents (é,à,ö → e,a,o) with a case option
   (lowercase/uppercase), collapse multiple spaces, all in-browser.

## Table-stakes → decision

| Capability | Competitors | Our decision |
| --- | --- | --- |
| Unicode forms NFC/NFD/NFKC/NFKD | all Unicode tools | **in** — `form` enum (adds `none` passthrough) |
| Lowercase / UPPERCASE | CodeShack, EasyProTools, TextCleaner | **in** — `case` enum `lower`/`upper` |
| Case folding (caseless matching, ß→ss) | NLP tools | **in** — `case=fold` |
| Strip accents / diacritics | ToolsGod, OnlineTextTools, CodeShack | **in** — `strip_accents` boolean |
| Collapse multiple spaces → one | EasyProTools, TextCleaner | **in** — `whitespace=collapse` (default) |
| Trim leading/trailing whitespace | most | **in** — `whitespace=trim` / included in `collapse` |
| Keep line breaks but tidy each line | — (gap) | **in** — `whitespace=collapse-lines` (our extra) |
| Normalize fancy punctuation (curly quotes, dashes, ellipsis → ASCII) | implied by "normalize Unicode" | **in** — `punctuation` boolean |
| Preset workflows (NLP prep, search key) | EasyProTools | **in** — `[[example]]` preset chips |
| Character/code-point diff & analysis panel | Donesnap, FreeTools Pro | **out** — visualization UI, not a text→text transform |
| Remove punctuation *entirely* / remove numbers (NLP prep) | EasyProTools | **out of scope** — destructive NLP-prep steps belong to a dedicated text-cleaner; we *normalize* punctuation to ASCII, not delete it |
| Zero-width / BOM removal | Ritetext, smart-quotes-clean | **out** — already covered by the existing `zero-width-cleaner` and `smart-quotes-clean` blocks |

Every table-stake is either in the descriptor or explicitly listed out-of-scope above —
nothing dropped silently. No competitor copy, branding, or trademarks are reproduced.

## Order of operations (documented on the page)

punctuation → Unicode form → strip accents → case → whitespace. Whitespace runs last so it
tidies anything the earlier steps introduce.
