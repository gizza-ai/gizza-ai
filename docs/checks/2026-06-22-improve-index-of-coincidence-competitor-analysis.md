# Competitor analysis — index-of-coincidence (2026-06-22)

Tool: **Index of Coincidence Calculator** — computes the IC of a text to gauge
whether it is monoalphabetic, polyalphabetic, or random, and to estimate a
Vigenère key length. Pure-compute block; surfaces = chat / CLI / page.

## Competitors surveyed

| # | Tool | Notable features |
| - | --- | --- |
| 1 | dCode — Index of Coincidence (dcode.fr/index-coincidence) | IC value; three char-handling modes (A-Z only / all chars / all-but-space); probable key-length; probable number of alphabets; interpretive guidance; per-language reference IC (EN/FR/DE/ES/IT/RU); CSV/TXT export |
| 2 | PlanetCalc — Index of Coincidence (planetcalc.com/7944) | IC value for given text; simple single-number output |
| 3 | PlanetCalc — Vigenère Cipher Breaker (planetcalc.com/7956) | Per-column IC over candidate key lengths to find the most likely key length (then full break) |
| 4 | Toolkk — Index of Coincidence Calculator (toolkk.com) | IC value; framed for polyalphabetic / Vigenère key-length analysis |
| 5 | CryptoWeb / Univ. of Portsmouth (crypto.soc.port.ac.uk) + Southampton MATH1001 notes | IC for working out Vigenère key length; teaching context |

## Capability diff (theirs → ours)

| Capability | Competitors | gizza index-of-coincidence | Status |
| --- | --- | --- | --- |
| IC value (raw probability) | yes (≈0.0667 EN) | yes — `raw`, 5 dp | covered |
| Normalized IC (×26, ≈1.73 EN) | dCode/most | yes — `normalized`, 4 dp | covered |
| Plain-language interpretation | dCode | yes — 3-bucket interpret() | covered |
| Per-letter frequency table | partial | yes — `show_counts` (count + %) | covered |
| Key-length / period estimation | planetcalc breaker, dCode | yes — per-column avg IC for `1..=max_period`, picks the highest (the planetcalc breaker method); also a single-number **Friedman estimate** (added this pass) | covered + |
| Case-insensitive, A-Z-only counting | dCode (a mode) | yes (the default + only mode) | covered |
| Counts modes (all chars / all-but-space) | dCode | A-Z-only only | out of scope (classical convention; flagged below) |
| Per-language reference IC table | dCode | English reference baked into copy/interpretation | partial (copy) |
| CSV/TXT export | dCode | page output is selectable plain text; CLI returns text | covered (effectively) |
| Full Vigenère break (recover key+plaintext) | planetcalc breaker | no | OUT OF MODEL — separate tool; IC only classifies + estimates length |

## Gaps closed this pass

- **Friedman key-length estimate** added to the core/report (`friedman_key_length`)
  — the closed-form `K ≈ 0.0265·n / ((0.0665 − IC) + n·(IC − 0.0385))` that dCode
  exposes as "probable number of alphabets". Shown when the IC is below
  plaintext level (estimate ≥ 1.5). Surfaces: chat, CLI, page.
- Sharpened copy/interpretation: normalized vs raw table, the period-analysis
  explanation (companion to Kasiski), and the English reference values, so the
  page communicates the same guidance dCode/Toolkk provide.

## Out-of-model / deliberately not built

- **Full Vigenère decryption** (recover key + plaintext): a different, larger tool
  (the planetcalc *breaker*). This tool is the IC statistic + length estimate only.
- **Multi-language reference IC selection / non-Latin alphabets**: the core counts
  the 26 Latin letters per the classical convention; alternate alphabets and the
  per-language reference table beyond English are not implemented. English
  baseline is documented in the page copy.
- **All-characters / all-but-space counting modes** (dCode): out of scope —
  classical cryptanalytic IC is computed over A-Z; counting punctuation/spaces
  changes the statistic's meaning.

No competitor copy, branding, or trademarks were reproduced.

## Verification

- `cargo test --workspace` in `blocks/index-of-coincidence/` — 11 tests pass
  (core 10 incl. English≈1.73, uniform formula, all-same max, period recovery,
  Friedman; block 1 schema drift-guard).
- `wafer build` — chat block instantiates and validates (297 KiB).
- `wasm-pack build` web + generator — page renders at `/tools/index-of-coincidence/`.
- CLI (`gizza tool index-of-coincidence …`) — basic, period+counts, Friedman, and
  no-letters error all behave correctly.
- Playwright (`tool-page-index-of-coincidence.spec.ts`) — 3 tests pass (basic IC,
  period+counts, query-param deep-link).

## Sources

- [dCode — Index of Coincidence](https://www.dcode.fr/index-coincidence)
- [PlanetCalc — Index of Coincidence](https://planetcalc.com/7944/)
- [PlanetCalc — Vigenère Cipher Breaker](https://planetcalc.com/7956/)
- [Toolkk — Index of Coincidence Calculator](https://www.toolkk.com/en/tools/index-of-coincidence-calculator)
- [CryptoWeb — Index of Coincidence](https://crypto.soc.port.ac.uk/crypto/cryptoweb/ioc.html)
