# Improve text-to-unicode — competitor analysis (2026-06-30)

## Scope

Tool: `text-to-unicode`

Goal: list every character in user-provided text with Unicode code point notation, decimal value, escape sequence, UTF-8 bytes, and official Unicode name. Runs locally in chat, CLI, and the browser page.

## Competitor scan

1. Compart Unicode Utilities / code point tools
   - Strengths: direct U+ notation, character names, broad Unicode database coverage.
   - Gaps closed here: local deterministic output with JSON and table modes, UTF-8 byte display, and gizza CLI/chat/page surfaces.

2. CyberChef Unicode text tools
   - Strengths: many encoding transformations and chained recipes.
   - Gaps closed here: focused per-character inspection, compact table output, and no recipe setup for a simple code-point audit.

3. Unicode Explorer / Unicode Lookup sites
   - Strengths: rich single-character metadata and search.
   - Gaps closed here: batch inspection of every character in a pasted string, including invisible/control characters.

4. Online Unicode Converter / escape tools
   - Strengths: quick escape-sequence conversion.
   - Gaps closed here: names, decimal values, UTF-8 bytes, and JSON export instead of only escaped text.

5. Browser console / language REPL snippets
   - Strengths: programmable and flexible.
   - Gaps closed here: no code required, consistent schema, and shareable page parameters.

## In-model improvements included

- Aligned table output for human inspection.
- JSON output for programmatic use.
- Official Unicode character names via a wasm-safe Unicode names crate.
- Visible placeholders for spaces and control characters so invisible input can be spotted.
- BMP `\uXXXX` and astral `\u{XXXX}` escape notation.
- UTF-8 hex byte display.
- Browser page copy, textarea input, format selector, and query-param deep-link test.

## Out-of-model / not built

- Full Unicode property browser, confusables/security skeletons, normalization diffing, or bidirectional text analysis. Those are larger tools and should be separate focused blocks if needed.
- Remote lookup services or live Unicode database updates. This block uses the bundled crate data so it works offline and inside wasm.

## Verification checklist

- Core unit tests cover ASCII, emoji, JSON output, control characters, empty input, and format parsing.
- Drift-guard schema test covers the chat/LLM descriptor.
- Web wrapper exposes `run(text, format)` for the generated page.
- Playwright tests cover table output, JSON output, and query-param deep-link behavior.
