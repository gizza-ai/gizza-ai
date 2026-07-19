# diceware-passphrase — competitor analysis (2026-07-20)

Scanned before implementing (WebSearch: "diceware passphrase generator online EFF wordlist tool"),
top-3 real tools skimmed. Paraphrased observations only — no copy, branding, or trademarks reused.

## Competitors reviewed

1. **diceware.dmuth.org** — button row for the number of dice rolls (2–8, default 4), EFF long
   wordlist only, shows the rolled words and the joined passphrase separately, dice-roll
   animation, copy-to-clipboard, "# of possible passwords" figure. No separator, capitalization,
   digit/symbol, or crack-time controls.
2. **diceware.rempe.us** — word-count buttons (5–9, +1), wordlist dropdown with many languages
   plus the original Diceware list, join-style buttons (spaces, hyphens, PascalCase, camelCase,
   snake_case, dots, random caps), append-symbol and shuffle actions, entropy readout and a
   crack-time table across attacker speeds, copy button, and manual physical-dice input (type
   digits 1–6, 5 per word).
3. **wutools.com diceware generator** — word-count slider (default 6) with per-count entropy
   hints, security-goal preset dropdown (~60/80/100/128-bit targets) that auto-sizes the word
   count, separator dropdown (dash/underscore/dot/space/none/random symbol per gap),
   capitalization checkbox, trailing random digit (~3.3 bits) and trailing random symbol
   (~3.6 bits) checkboxes, dice-roll display checkbox (5-digit blocks next to each word), batch
   count, entropy + guesses + online/offline attack-time estimates, copy/download, long FAQ.

## Table stakes → disposition

| Capability | Tag | Where it landed |
| --- | --- | --- |
| Word count control (slider/buttons, default ~6) | in-model | `words` integer 2–20, default 6, page `kind = "slider"` |
| EFF long wordlist (7,776 words, 5 dice) | in-model | `wordlist = eff-long` (default), list embedded (CC-BY 3.0, attributed in core) |
| EFF short wordlist (1,296 words, 4 dice) | in-model | `wordlist = eff-short` |
| Separator choice incl. none + random symbol per gap | in-model | `separator` enum hyphen/space/underscore/dot/none/random-symbol |
| Capitalize words (readability, PascalCase-style with `none`) | in-model | `capitalize` boolean |
| Trailing random digit / symbol (+~3.3 / ~3.6 bits) | in-model | `add_number` / `add_symbol` booleans |
| Entropy bits + strength label | in-model | in every output (`Entropy: … — strength: …`) |
| Crack-time estimate | in-model | offline 10^10 guesses/sec line; humanized duration |
| Batch generation | in-model | `count` 1–20, one per line; page text-download link is generator-standard |
| Show dice rolls per word | in-model | `show_rolls` boolean (`62315  tiger` lines) |
| Manual physical-dice input (digits 1–6) | in-model | `rolls` string param — deterministic lookup, also the exact-output test path |
| Security-goal presets (bit targets → word count) | in-model | `[[example]]` chips: Recommended 6 / Vault 8 + symbol / short-list / physical-dice |
| Copy result button | in-model | generator-standard Copy button on every field/text tool |

## Out-of-model (listed, not built)

- **Non-English / original-Diceware wordlists** (rempe ships many languages): data-size and
  curation cost; English EFF lists only. Could be a future `wordlist` enum extension.
- **Dice-roll animation** (dmuth): visual gimmick, not expressible in the declarative page
  runtime and adds nothing to correctness.
- **Shuffle / re-roll a single word** (rempe): stateful per-word UI; re-running generates a
  fresh phrase, which covers the need.
- **Random-caps join style** (rempe): nonstandard; `capitalize` covers title-casing. A camelCase
  variant (first word lowercase) is likewise omitted — `capitalize` + `separator=none` gives the
  PascalCase form.
- **Auto-sizing word count from a named bit target** (wutools dropdown): covered statically by
  the preset chips + per-count bit figures in the copy/FAQ; a live auto-sizer would need
  page-side logic the declarative controls don't express.
- **Attacker-speed table / online-vs-offline matrix** (rempe, wutools): single honest offline
  figure (10^10 guesses/sec) shown instead; FAQ explains the assumption.

## Design decisions

- Deterministic `rolls` path doubles as the exact-output verification surface (CLI + Playwright)
  — a generator is otherwise untestable for exact values.
- Entropy math: log2(7776) ≈ 12.9 bits/word (long), log2(1296) ≈ 10.3 (short); random-symbol
  separator adds log2(12) per gap; trailing digit/symbol add log2(10)/log2(12). Capitalization
  adds 0 and the copy says so.
- Strength bands: <45 weak, <70 fair, <100 strong, ≥100 very strong (6 long-list words = 77.5
  bits = strong; 8 = 103.4 = very strong — matches the vault-preset narrative).
- Existing `password-generator` block keeps its toy ~120-word passphrase mode; this tool is the
  real diceware surface (7,776-word EFF list, dice-roll semantics). Not a duplicate: different
  wordlists, entropy class, and workflow (physical dice, roll display).
