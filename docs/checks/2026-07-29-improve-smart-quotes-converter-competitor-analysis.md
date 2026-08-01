# smart-quotes-converter — competitor analysis (2026-07-29)

Function: convert quotation marks between **straight ASCII** (`"` `'`) and **curly
typographic** (`“ ” ‘ ’`) forms, in either direction, choosing opening vs closing
marks from surrounding context (SmartyPants-style "educate quotes"). Pure-compute,
runs entirely in the browser / CLI / chat sandbox.

## Competitors surveyed (top real tools)

| # | Tool | Directions | Double/single toggle | Opening/closing detection | Feet/inches primes | Also cleans dashes/ellipsis |
|---|------|-----------|----------------------|---------------------------|--------------------|-----------------------------|
| 1 | WebUtility.io — Straight → Curly | straight→curly | no | yes | no | no |
| 2 | Infyways — Smart Quotes Converter | both | no | yes | no | no |
| 3 | FreeToolkit.ai — Smart Quotes Converter | both | no | yes | no | yes (ellipsis + em dash) |
| 4 | TextGround — Smart Quotes | both | no | yes | no | no |
| 5 | Curly Quote Converter (curlmyquotes.com) | straight→curly | no | yes | no | no |

Paraphrased from public tool pages; no copy/branding/trademarks reproduced.

## Table-stakes → decision

| Capability | In competitors | Decision |
|------------|----------------|----------|
| Both directions (straight↔curly) | 2–4 offer both | **IN** — `direction` enum `to_curly` (default) / `to_straight`. |
| Opening vs closing detection | all | **IN** — core `opens_after()` picks opener from the preceding char (start/space/bracket/dash); after a letter/digit → closing/apostrophe. |
| Curl double quotes | all | **IN** — `convert_double` (default true). |
| Curl single quotes / apostrophes | all | **IN** — `convert_single` (default true). Handles `It's`, `dogs'`, elided-year `'89`. |
| Toggle double vs single independently | none surveyed expose it | **IN (edge over field)** — separate `convert_double`/`convert_single` booleans let users curl one kind only (e.g. keep `'` straight for code identifiers). |
| Feet/inches → prime marks (`6'4"`→`6′4″`) | none | **IN (edge)** — `feet_inches` boolean (default false); digit-adjacent straight quote becomes `′`/`″`. `to_straight` always folds primes back. |
| Instant, client-side, no upload | all | **IN** — pure Rust, runs in-browser / CLI / chat; 1 MB cap. |
| Also straighten em/en dashes, ellipsis, guillemets, exotic spaces | #3 only | **OUT (deferred to sibling `smart-quotes-clean`)** — that already-shipped block does full typographic→ASCII cleanup (dashes, ellipsis, primes, guillemets, zero-width/exotic spaces). Keeping this tool quote-focused avoids duplication; `to_straight` here covers the quote+prime subset only, by design. |

## Relationship to the existing `smart-quotes-clean` block (NOT a duplicate)

`smart-quotes-clean` is **one-directional** (typographic → ASCII only) and its scope is
the whole punctuation set (quotes **plus** dashes, ellipsis, guillemets, exotic/zero-width
spaces). `smart-quotes-converter` is **bidirectional**, and its headline/default direction
is the opposite one — `to_curly`, i.e. *adding* smart quotes with opening/closing detection,
which `smart-quotes-clean` cannot do at all. The overlap is only the quote-straightening
subset of `to_straight`; the educate/curl direction is the distinct capability. Analogous to
how `change-case` is a distinct superset tool rather than a duplicate of any single-mode caser.

## Out-of-model / not built

- Dash/ellipsis/guillemet/space cleanup on the straighten path — **covered by the sibling
  `smart-quotes-clean` tool**; listed, not rebuilt here.
- Locale-specific curly styles (German `„…“`, French guillemets `« … »`) — out of scope;
  the tool targets the standard EN curly set. Listed, not built.

## UX / page controls shipped

- `direction` → `<select>` with friendly labels (Straighten ↔ Curl).
- `convert_double` / `convert_single` / `feet_inches` → checkboxes (defaults on/on/off).
- `[[example]]` preset chips: a curl example and a straighten example, one-click prefilled.
- Worked example + ≥3 FAQ accordions + stated limits (1 MB cap, EN curly set, ambiguous
  leading-apostrophe cases) in `content.md`.
