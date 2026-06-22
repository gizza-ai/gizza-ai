# zlib-compress — competitor analysis snapshot (2026-06-22)

## Tool
`gizza-ai/zlib-compress` — compress a file (or any bytes) into a **zlib** stream
(RFC 1950): a 2-byte zlib header (CMF/FLG), the raw DEFLATE body (RFC 1951), and a
trailing 4-byte big-endian **Adler-32** checksum of the uncompressed data. Output is
returned as a downloadable `<input>.zz`. Pure-Rust `flate2` (miniz_oxide), runs on all
backends including the chat Service Worker. `level` param 1-9 (default 6). Surfaces:
chat + CLI (no page — file→file output, the no-page file-input pattern, like
`gzip-compress` / `raw-deflate`).

## Top competitors surveyed

1. **dcode.fr — Zlib Compression** (https://www.dcode.fr/zlib-compression)
   Online zlib compress/decompress, copy/paste + file. General-purpose encoding site; no
   explicit level control surfaced.
2. **Python `zlib.compress`** (https://docs.python.org/3/library/zlib.html)
   Stdlib reference for the RFC-1950 form: `zlib.compress(data, level)` produces the
   `78 xx` header + DEFLATE + Adler-32 we emit; the canonical interop target. Used here to
   cross-verify the output (`zlib.decompress` round-trips our bytes).
3. **DevToys / DevToys Web Pro** (https://devtoys.pro/blog/gzip-deflate-zlib-formats)
   Documents the gzip vs deflate vs zlib framing distinction; offers gzip-family
   compress/decompress, treats zlib mainly as an explainer of the three framings.
4. **zlib reference library** (https://zlib.net/) — the C library defining the format;
   `compress2()`/`deflate()` with `Z_DEFAULT_COMPRESSION`; library, not an end-user tool.
5. **nodejs `zlib.deflateSync`** (https://nodejs.org/api/zlib.html) — Node's built-in zlib
   binding; `deflateSync(buf, { level })` yields the zlib-framed form. Library/runtime API,
   no UI.

## Gap diff & ranking (fit-to-model)

- **Distinct from existing gizza blocks (no dup):** `gzip-compress` wraps output in the
  gzip container (`1F 8B` magic + optional FNAME header + CRC-32); `raw-deflate` emits the
  bare RFC-1951 stream (no header, no checksum). `zlib-compress` is the middle framing
  (RFC 1950): 2-byte header + Adler-32, no filename/timestamp. Verified the output's first
  bytes are `78 da` (level-9 zlib), not `1f8b` (gzip) and not headerless, and that it
  round-trips with Python `zlib.decompress`. Same niche-coexistence pattern as
  `lz4-compress` vs `gzip-compress`. Kept as its own tool.
- **Level control (1-9):** present (matches zlib/Python/Node `level`, exceeds dcode.fr which
  hides it). In model, done.
- **Both directions:** competitors (dcode.fr, Python, Node) also inflate. A zlib-INFLATE /
  decompress tool is a separate backlog item (cf. `lz4-decompress` is its own block);
  **out of scope for this single tool** — noted, not built here. No copy/branding borrowed.
- **Framing clarity / interop:** addressed in the skill description (calls out the 2-byte
  header + Adler-32 and the PNG IDAT / `Content-Encoding: deflate` use cases) and the
  `<input>.zz` naming + the for_llm note. In model, done.
- **Tiny/incompressible inputs grow:** handled — the size note reports "~N% larger
  (input too small/incompressible)" instead of underflowing, since zlib adds the 6-byte
  header+checksum plus a few bytes of DEFLATE block overhead on inputs too small to compress.

## In-model capabilities closed
Level 1-9, RFC-1950 zlib output (header + DEFLATE body + Adler-32), size-change reporting
(smaller/larger), file or `ref` input, runs on all backends. Drift-guard schema test passes;
core unit tests cover round-trip, valid zlib header (CM=8, FCHECK %31 == 0, not gzip),
repetitive-data compression, level clamping, monotonic level, and empty input (still framed).
Cross-verified the CLI output against Python `zlib.decompress` (`78 da` header, exact bytes).

## Out-of-model / not built (no copy taken)
- zlib **inflate** (decompress) — separate tool, like `lz4-decompress`.
- Custom zlib dictionaries / stream-level tuning — not exposed by flate2's simple
  `ZlibEncoder` API; niche.

## Sources
- [RFC 1950 - ZLIB Compressed Data Format Specification v3.3](https://www.ietf.org/rfc/rfc1950.txt)
- [RFC 1951 - DEFLATE Compressed Data Format Specification v1.3](https://www.ietf.org/rfc/rfc1951.txt)
- [Python zlib module](https://docs.python.org/3/library/zlib.html)
- [dcode.fr — Zlib Compression](https://www.dcode.fr/zlib-compression)
- [DevToys — GZip vs Deflate vs Zlib](https://devtoys.pro/blog/gzip-deflate-zlib-formats)
- [zlib reference library](https://zlib.net/)
- [Node.js zlib API](https://nodejs.org/api/zlib.html)
