# Competitor analysis — pronounceable-password-generator (2026-07-18)

WebSearch: "pronounceable password generator tool options length syllables digits symbols".
Skimmed the top real competitor tools below. All findings paraphrased.

## Tools scanned

1. **texttoolbox.net / texttools.org — Pronounceable Password Generator** — length
   control, toggles for numbers and symbols, letter-casing option, "generate
   multiple at once". Frames output as fake-but-speakable syllables not in any
   dictionary.
2. **warpconduit.net — Readable Pronounceable Password Generator** — adjustable
   length, optional numbers/symbols, copy button; emphasizes readability +
   easy-to-type.
3. **RandomKeygen — Pronounceable Passwords** — serves a batch of ready-made
   pronounceable passwords of a fixed-ish length; minimal controls, copy-to-use.
4. **online-password-generator.com — Speakable Passwords** — length, casing, and
   digit/symbol injection between/after syllables; stresses "say it aloud / share
   verbally".

## Table-stakes params, defaults, patterns

| Capability | Competitor norm | Our decision | In/out of model |
|---|---|---|---|
| Length control | ~8–16 chars typical, recommend 12+/16+ | `length` 4–64, default 12 (letters) | in-model |
| Numbers | two-digit suffix common | `digits` 0–12, default 2 | in-model |
| Symbols | one symbol satisfies most rules | `symbols` 0–12, default 1 | in-model |
| Letter casing / capitalize | on by default | `capitalize` bool, default true | in-model |
| Cryptographic RNG | claimed by better tools | getrandom CSPRNG + rejection sampling | in-model |
| Entropy readout | some show strength/bits | entropy in bits reported | in-model |
| Example/preset chips | a few offer quick presets | 3 `[[example]]` chips | in-model |
| Generate N at once | common | out of scope — re-run for a fresh value | out-of-model |
| Copy button | common | generic page Download/copy affordance | platform |
| Digit/symbol placed *between* syllables | a couple offer it | appended-suffix only (keeps a stable, testable shape) | out-of-model (deferred) |

## Worked examples

- Default (12 letters, cap, 2 digits, 1 symbol) → e.g. `Bofuka92!`, ≈ 49 bits.
- 20 letters + 3 digits + 2 symbols → longer, ≈ 80+ bits.
- Letters-only lowercase (16, no digits/symbols) → pure pronounceable string.

## UX controls adopted

Sliders for length/digits/symbols (mirrored onto number boxes), a capitalize
checkbox, and three preset chips (Memorable / Long & strong / Letters only).
No competitor copy or branding reused.
