# readability-score — competitor analysis (2026-06-21)

Tool: `blocks/readability-score` — scores English text for readability and returns the
classic indices plus the underlying counts. Three surfaces verified: chat block
(`wafer build` validated), CLI (`gizza tool readability-score text=…`), and the standalone
page (`/tools/readability-score/`, Playwright-driven).

## Top competitors surveyed

1. **WebFX Readability Test** (webfx.com/tools/read-able) — Flesch Reading Ease,
   Flesch-Kincaid Grade, Gunning-Fog, Coleman-Liau, ARI, SMOG. URL or paste input.
2. **Readable** (readable.com) — paid SaaS; "every notable readability algorithm,"
   keyword density, tone, scoring history (account-gated).
3. **ToolsFYI Flesch-Kincaid Score Checker** — Flesch-Kincaid, Gunning-Fog, SMOG, ARI,
   plus an overall complexity label; no signup.
4. **MiniWebtool Readability Score Calculator** — multiple indices + the reading grade
   needed to understand the text.
5. **Authorlytica Readability Checker** — live Flesch Reading Ease, Flesch-Kincaid,
   Gunning-Fog, SMOG, Coleman-Liau, ARI on paste; no signup.

## Gap analysis (fit to gizza's pure-Rust, browser-local model)

| Capability | Competitors | gizza before | Action |
|---|---|---|---|
| Flesch Reading Ease | all | yes | kept |
| Flesch-Kincaid Grade | all | yes | kept |
| Gunning-Fog Index | most | yes | kept |
| SMOG Index | most | yes | kept |
| Coleman-Liau Index | WebFX, Authorlytica | **no** | **added** (letters/word, sentences/word) |
| Automated Readability Index (ARI) | WebFX, ToolsFYI, Authorlytica | **no** | **added** (chars/word, words/sentence) |
| Underlying counts (words/sentences/syllables/complex words) | some | yes | kept |
| Plain-language reading level + grade band labels | some | yes | kept |
| Privacy: 100% client-side, no upload, no signup | a few (most are server-side) | yes | kept (a differentiator) |

### Closed in this pass

- Added the **Coleman-Liau Index** and **Automated Readability Index** so the six indices
  match the broadest competitors (WebFX/Authorlytica). The plain-language `average_grade`
  now averages all five grade-level indices for a more stable signal. Surfaced on all three
  surfaces (JSON keys `coleman_liau` / `automated_readability`; page summary lines).

### Out of model / deliberately not built (no copying)

- **URL fetching of a target page** — gizza's page surface is offline/local; a network
  fetch would make it a chat-only `network` tool. Out of scope for this pure tool; users
  paste text.
- **Keyword density, tone, sentiment, history dashboards** (Readable) — these are separate
  tools / require accounts and analytics; not a readability-grade capability.
- **Dictionary-exact syllable counts** — gizza uses a vowel-group heuristic with
  silent-`e`/`-le` adjustments (documented on the page). Competitors that bundle a
  pronunciation dictionary get marginally more precise syllable counts; the heuristic is
  accurate enough to compare drafts and hit a target band, and Coleman-Liau/ARI are
  syllable-free cross-checks.

## Tests run (all pass)

- `cargo test --workspace` in `blocks/readability-score` — 9 unit tests (core 8 + drift-guard 1).
- `wafer build` — chat block validates OK (297.6 KiB).
- `gizza tool readability-score text=…` — simple + complex inputs return all six indices.
- Playwright `tool-page-readability-score.spec.ts` — page renders the six indices.

No competitor copy, branding, or trademarks were reproduced.
