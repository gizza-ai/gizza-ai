# file-compressor — competitor analysis (2026-07-18)

Function: compress or decompress a file with a chosen general-purpose codec
(zstd, gzip, xz, brotli), a compress/decompress direction, and a compression
level. All findings are **paraphrased** — no competitor copy, branding, or
trademarks are reproduced.

## Competitors skimmed (top real tools)

1. **A browser-local multi-codec compression utility** (Deflate / Gzip / Brotli /
   LZMA / Snappy). Both directions. Accepts pasted text, files, and raw binary.
   Codec chosen from a dropdown; runs entirely client-side.
2. **A browser-local Zstandard compressor/decompressor.** Upload a file or paste
   text, pick a level (their range 1–22), compress or decompress in-browser with
   no server round-trip. Advertises zstd's better ratio + fast decode vs gzip.
3. **An in-browser compression *comparison* tool** (gzip / Brotli / Zstandard).
   Runs each codec in a Web Worker, shows the compressed size, the percent saved
   vs the original, and per-codec timing, and round-trip-verifies every result.

(Also seen: a desktop Zstandard archive utility, and the reference `zstd` CLI
man page — both confirm the compress/decompress + level surface as table stakes.)

## Table-stakes → decision (in-model / out-of-model)

| Capability | Decision | Where |
| --- | --- | --- |
| Multiple general-purpose codecs (gzip, xz/lzma, brotli, zstd) | **in-model** for gzip/xz/brotli (pure-Rust: `flate2`, `lzma-rust2`, `brotli`); zstd **decompress** in-model (`ruzstd`) | `format` enum |
| Compress **and** decompress in one tool | **in-model** | `operation` enum |
| Compression-level selector | **in-model** (1–9, mapped per codec) | `level` param |
| Report compressed size + ratio / % saved | **in-model** | `for_llm` summary |
| File upload + download of the result | **in-model** (url⊕ref in; base64 download envelope out) | `Input::File` |
| Round-trip correctness | **in-model** | unit tests (each codec) |
| **zstd *compression*** | **OUT-OF-MODEL** — the standard zstd encoder is the C `zstd` library; it needs a wasi C toolchain (no `clang`/`WASI_SYSROOT` here) so `zstd-sys` cannot build to `wasm32-wasip1`, and the only pure-Rust encoder (`zstd-pure-rs`) is an experimental LLM-mediated port that warns of possible data loss — unsafe for a compression tool. **zstd decompress IS supported** (mature pure-Rust `ruzstd`); zstd+compress returns a clear error steering the user to gzip/xz/brotli. | listed, not built |
| zstd level 1–22 | out-of-model (follows from no zstd compress) | listed |
| Snappy / LZ4 codecs | out-of-scope (LZ4 is its own block `lz4-compress`; Snappy is a separate niche) | listed |
| Cross-codec timing/benchmark comparison | considered, rejected — that is a distinct "compare codecs" tool, not a single compress/decompress action; would bloat this descriptor | listed |
| Paste-text input | out-of-model here — gizza's file tools take a `url`/`ref` source (a data: URL or a prior tool's ref covers pasted content) | n/a |

## Notes

- Auto-detection: decompress trusts the chosen `format` (the enum is explicit and
  keeps the schema honest); the per-codec decoder emits a clear "not a valid
  <codec> stream" error on a mismatch, and decompression is bomb-guarded with an
  output-size cap (mirrors `lzma-decompress`).
- This is a *unified* multi-codec tool. It is not a semantic duplicate of the
  single-codec blocks (`gzip-compress`, `lzma-compress`, `bzip2-compress`, …):
  it adds **brotli** and **zstd** (which have no existing block) and a single
  compress/decompress + codec selector surface.
