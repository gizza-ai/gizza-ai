# multi-encoder — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/multi-encoder` — encode/decode text across Base64, hex,
binary, URL percent-encoding, ROT13, and Morse, in one tool, both directions.
Chat + CLI + page (base64 + percent-encoding crates).

## Relationship to existing tools

The existing `url-encode` covers only URL percent-encoding; `multi-encoder` adds
**five** schemes that have no tool yet (base64, hex, binary, ROT13, Morse) plus
URL, in a single encode/decode interface — so it's additive, not a dup.

## What competitors do

- **CyberChef / dcode / rapidtables / "X to Y" sites** — paste, pick a recipe,
  get output. CyberChef is extremely powerful but heavyweight; the single-purpose
  sites are one-codec-per-page, often ad-heavy, and several **upload** the input.
- **CLI** (`base64`, `xxd`, `tr`) — local but per-codec and shell-bound.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm: page
   in-browser, CLI headless, chat Service Worker.
2. **Six codecs, both directions, one tool.** base64, hex, binary, url, rot13,
   morse — no hunting across six different pages.
3. **Round-trip correct & UTF-8 aware.** Each scheme round-trips (verified); hex
   decode tolerates whitespace; binary is 8-bit/byte; decodes that don't yield
   valid UTF-8 error clearly rather than emitting mojibake.
4. **Real Morse.** A–Z, 0–9 and ~25 punctuation marks, with letter spaces and
   ` / ` word separators (both encode and decode).
5. **Sensible ROT13** (symmetric — direction ignored, as it should be).
6. **Three surfaces + deep-links.**

## Honest scope

- Text in / text out (UTF-8). Binary blobs that aren't valid UTF-8 after decode
  are rejected (use the dedicated file tools — file-to-data-uri / file-hash — for
  raw bytes).
- Base64 is standard (not URL-safe) alphabet; could add a variant later.

## Tests

7 core unit tests: round-trips for base64 (`hi`↔`aGk=`), hex (`hi`↔`6869`, decode
tolerates spaces), binary (`A`↔`01000001`, multi-byte), URL (`a b&c`↔`a%20b%26c`),
ROT13 (symmetric), and Morse (`SOS`→`... --- ...`, words with ` / ` round-trip),
plus error cases (bad base64/hex/binary, non-Morse char, bad scheme/direction).
Plus the block drift-guard schema test. CLI + Playwright (base64 via fill; Morse
decode via deep-link) verified — see commit.
