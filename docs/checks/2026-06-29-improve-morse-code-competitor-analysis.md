# morse-code — competitor analysis & surface checks (2026-06-29)

**Tool:** `morse-code` — translate text to International Morse code and decode Morse back to text.

## Surface checks

| Surface | Check | Result |
| --- | --- | --- |
| Core/workspace tests | `cd blocks/morse-code && CARGO_BUILD_JOBS=1 cargo test --workspace` | ✅ 19 tests passed (descriptor drift guard + encode/decode/core cases) |
| Chat block | `cd blocks/morse-code && CARGO_BUILD_JOBS=1 wafer build` | ✅ produced and validated `target/block.wasm` |
| Web wasm | `CARGO_BUILD_JOBS=1 wasm-pack build blocks/morse-code/web --target web --release --out-dir pkg` | ✅ built `web/pkg` |
| Generator | `CARGO_BUILD_JOBS=1 cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `/tools/morse-code/` |
| CLI | `gizza tool morse-code text='SOS'` and `gizza tool morse-code direction=decode text='... --- ...'` | ✅ returned `"... --- ..."` and `"SOS"` |
| Page | `cd tests && xvfb-run npx playwright test tool-page-morse-code.spec.ts` | ✅ 4 passed (encode, decode, custom separators, query-param deep-link) |

## Competitor scan

Searches reviewed:
- `online morse code translator decoder encoder competitors`
- `morse code translator text to morse online decoder`

Representative competitors and references:

1. **Morse Code World Translator** — mature translator with Latin/Hebrew/Arabic/Cyrillic, audio playback, flashing/vibration, speed/Farnsworth/frequency controls, and share links.
2. **dCode Morse Code** — encryption/decryption-style Morse tool with explanatory copy and alphabet support.
3. **MorseDecoder.com** — simple two-box text↔Morse translator.
4. **DNSChecker Morse Code Translator** — encoder/decoder with advanced options and playback.
5. **Online Text Tools text-to-Morse** and similar utilities — quick browser text-to-Morse conversion.

## Gap / fit analysis

| Capability | Competitors | gizza `morse-code` | Decision |
| --- | --- | --- | --- |
| Text → Morse | Universal baseline | ✅ default `encode` mode | Built |
| Morse → text | Most dedicated translators support decoding | ✅ `direction=decode` | Built |
| International Morse table | Competitors cover A-Z/0-9/common punctuation | ✅ A-Z, 0-9, and common punctuation including `. , ? ' ! / ( ) & : ; = + - _ " $ @` | Built |
| Separator controls | Some tools assume spaces/slashes; advanced tools expose formatting | ✅ configurable `letter_sep` and `word_sep`; blanks use common defaults | Built |
| Dash alias | Morse Code World notes `_` as a dash alternative | ✅ decode normalizes `_` to `-` | Built |
| Audio / flashing / vibration | Morse Code World and DNSChecker offer playback and signals | ❌ out-of-model for this pure text tool; would require audio/timing UI | Not built |
| Speed/Farnsworth/frequency controls | Advanced practice tools support CW timing | ❌ out-of-model without audio generation | Not built |
| Multi-script alphabets | Some competitors support non-Latin alphabets | ❌ out-of-model for International Morse text table in this loop | Not built |
| Privacy/offline | Some browser tools are client-side | ✅ pure wasm/page + CLI/chat, no network required | Built |

## Improvements made from analysis

- Added both directions in one tool with explicit `direction` enum.
- Included configurable letter and word separators so users can match common `/` style or custom delimiters.
- Added punctuation and digit round-trip coverage, not just A-Z.
- Added page tests for encode, decode, custom separator behavior, and deep links.
