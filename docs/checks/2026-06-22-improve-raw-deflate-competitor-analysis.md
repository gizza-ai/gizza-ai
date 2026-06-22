# raw-deflate — competitor analysis snapshot (2026-06-22)

## Tool
`gizza-ai/raw-deflate` — compress a file (or any bytes) with **headerless raw DEFLATE**
(RFC 1951): no gzip (RFC 1952) and no zlib (RFC 1950) wrapper, just the bare deflate bit
stream. Output is returned as a downloadable `<input>.deflate`. Pure-Rust `flate2`
(miniz_oxide), runs on all backends including the chat Service Worker. `level` param 1-9
(default 6). Surfaces: chat + CLI (no page — file→file output, the no-page file-input
pattern, like `gzip-compress` / `gunzip`).

## Top competitors surveyed

1. **dcode.fr — Deflate Compression** (https://www.dcode.fr/deflate-compression)
   Online compress/decompress of Deflate streams. Both directions, copy/paste + file. No
   explicit level control surfaced; general-purpose puzzle/encoding site.
2. **Perl `IO::Compress::RawDeflate`** (https://perldoc.perl.org/IO::Compress::RawDeflate)
   Library, writes RFC 1951 files/buffers. Exposes a `Level` option and explicitly produces
   the headerless raw form — the canonical reference for what "raw deflate" means.
3. **beatgammit/deflate-js** (https://github.com/beatgammit/deflate-js) — JS lib,
   RFC 1951 raw deflate/inflate. Library, not an end-user tool; no UI.
4. **nayuki.io — Simple DEFLATE decompressor** (https://www.nayuki.io/page/simple-deflate-decompressor)
   Educational decompressor only (inflate), no compress path.
5. **DevToys / DevToys Web Pro** (https://devtoys.pro/blog/gzip-deflate-zlib-formats)
   Documents the gzip/deflate/zlib framing distinction; the toolbox offers gzip-family
   compress/decompress but treats raw deflate mostly as an explainer, not a first-class
   compressor.

## Gap diff & ranking (fit-to-model)

- **Distinct from existing gizza blocks (no dup):** `gzip-compress` wraps output in the gzip
  container (1F 8B magic + FNAME header + CRC-32); `raw-deflate` emits the bare RFC 1951
  stream (no magic, no checksum, no filename) — verified the output's first bytes are neither
  `1f8b` (gzip) nor `789c` (zlib) and that it round-trips with zlib `wbits=-15`. Same niche as
  the existing `lz4-compress` vs `gzip-compress` coexistence. Kept as its own tool.
- **Level control (1-9):** present (matches Perl `Level`, exceeds dcode.fr which hides it). In model, done.
- **Both directions:** competitors (dcode.fr, nayuki) offer inflate too. A raw-INFLATE tool is
  a separate backlog item (cf. `lz4-decompress` is its own block); **out of scope for this
  single tool** — noted, not built here. No copy/branding borrowed.
- **No-header guarantee / framing clarity:** addressed in the skill description and the
  `<input>.deflate` naming + the for_llm note ("no header"). In model, done.
- **Tiny/incompressible inputs grow:** handled — the size note reports "~N% larger
  (input too small/incompressible)" instead of underflowing, since raw deflate adds a few
  bytes of block overhead on inputs too small to compress.

## In-model capabilities closed
Level 1-9, headerless RFC-1951 output, size-change reporting (smaller/larger), file or `ref`
input, runs on all backends. Drift-guard schema test passes; core unit tests cover round-trip,
no-gzip-header, repetitive-data compression, level clamping, monotonic level, empty input.

## Out-of-model / not built (no copy taken)
- Raw **inflate** (decompress) — separate tool, like `lz4-decompress`.
- Stream/dictionary tuning (zlib custom dictionaries) — not exposed by flate2's simple API; niche.

## Sources
- [RFC 1951 - DEFLATE Compressed Data Format Specification v1.3](https://www.ietf.org/rfc/rfc1951.txt)
- [dcode.fr — Deflate Compression](https://www.dcode.fr/deflate-compression)
- [Perl IO::Compress::RawDeflate](https://perldoc.perl.org/IO::Compress::RawDeflate)
- [beatgammit/deflate-js](https://github.com/beatgammit/deflate-js)
- [nayuki.io — Simple DEFLATE decompressor](https://www.nayuki.io/page/simple-deflate-decompressor)
- [DevToys — GZip vs Deflate vs Zlib](https://devtoys.pro/blog/gzip-deflate-zlib-formats)
