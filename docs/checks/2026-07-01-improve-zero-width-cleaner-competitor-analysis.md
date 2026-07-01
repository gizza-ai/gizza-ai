# zero-width-cleaner competitor analysis (2026-07-01)

## Scope

Tool: `zero-width-cleaner`

Goal: detect and strip invisible Unicode formatting characters from pasted text while preserving ordinary visible text. Important cases include zero-width space/non-joiner/joiner, byte-order mark, word joiner, bidirectional controls, soft hyphen, and non-breaking/odd Unicode spaces.

## Competitors reviewed

1. MiniWebtool — Invisible Character Remover
   - Describes detecting/removing zero-width spaces, soft hyphens, and other invisible Unicode characters, with visual detection emphasis.
   - In-model gap: broad invisible-character coverage and optional visible replacement.
   - Shipped: zero-width, bidi, soft-hyphen removal plus a replacement field to show where removals happened.

2. FreeToolsCorner — Zero-Width Character Remover
   - Focuses on ZWSP, ZWNJ, BOM, and automated Unicode cleaning.
   - In-model gap: handle common zero-width family plus BOM explicitly.
   - Shipped: U+200B/U+200C/U+200D, U+2060, U+2061–U+2064, U+180E, and U+FEFF are covered.

3. Leap AI — Invisible Character Remover
   - Emphasizes browser/no-signup text cleanup and hidden Unicode from pasted text.
   - In-model gap: privacy-focused page copy and clear pasted-text use cases.
   - Shipped: browser-local WebAssembly page with privacy note and paste-first UX.

4. Elysia Tools — Zero Width Character Remover
   - Positions the task around hidden formatting/control-code cleanup and data integrity.
   - In-model gap: expose toggles so users can preserve characters like emoji joiners when needed.
   - Shipped: independent toggles for zero-width, bidi controls, soft hyphen, and odd-space replacement.

5. Unformat.online — Remove Zero-Width Spaces
   - Highlights practical failures: broken comparisons, regex/search failures, and code bugs.
   - In-model gap: explain why invisible characters matter and include Trojan-Source/bidi controls.
   - Shipped: page copy and default removal include bidi controls with a Trojan Source note.

## In-model improvements shipped

- Removes zero-width characters, joiners, word joiner, invisible math operators, Mongolian vowel separator, and BOM.
- Removes bidirectional formatting controls by default.
- Removes soft hyphens by default.
- Optional conversion of non-breaking/odd Unicode spaces to ordinary ASCII spaces.
- Optional visible replacement string for each removed invisible character.
- Page content documents emoji/ZWJ trade-off and privacy behavior.
- Unit tests, descriptor drift guard, wafer fixture, CLI smoke, and Playwright page coverage.

## Out-of-model / not built

- Highlighted per-character visualization map in the page output.
- File upload/batch cleaning.
- Server-side APIs, saved history, or account/workspace features.

## Verification notes

The final verification matrix includes block cargo tests, wafer build, wasm-pack web build, generator, CLI smoke, and Playwright page tests for default cleaning, replacement/NBSP behavior, and preserving ZWJ when zero-width removal is disabled.
