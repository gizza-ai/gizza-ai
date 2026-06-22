# bzip2-compress — competitor analysis (2026-06-22)

## What we built
`bzip2-compress` compresses a file (or any bytes from a `url`/`ref` source) into a
single bzip2 (`.bz2`) stream using the Burrows–Wheeler transform, returned as a
downloadable file named `<input>.bz2`. `level` (1-9) sets the BWT block size
(100 KB units; 9 = best ratio, default). Pure-Rust `banzai` encoder → runs on all
backends including the chat Service Worker.

Surfaces: **chat + CLI**. No standalone page — this is the no-page file-input
pattern (file → binary file output), exactly like `gzip-compress` / `lzma-compress`.
Output validated end-to-end: `bzip2 -t` accepts it and `bunzip2` recovers the
original bytes byte-for-byte.

## Competitors surveyed

| Tool | Model | Notable features | Gap vs us |
|------|-------|------------------|-----------|
| [bzip2.utils.com](https://bzip2.utils.com/) | Client-side, browser | Drag-drop, client-side privacy | UI niceties only; same core output |
| [thetoolapp.com bzip2](https://thetoolapp.com/utilities/bzip2-compress-file/) | Client-side, browser | Drop → Go → download .bz2 | No level control exposed; we expose level 1-9 |
| [CloudConvert BZ2](https://cloudconvert.com/bz2-converter) | Server upload | Format conversion suite, batch | Uploads to their servers (privacy); ours is local/agentic |
| [AnyConv BZ2](https://anyconv.com/bz2-converter/) | Server upload | Bulk convert, many formats | Server-side upload required |
| [PeaZip bzip2](https://peazip.github.io/bzip2-utility.html) | Desktop app | Full archive manager, extract too | Native install; not chat/CLI/agent-callable |

## Gap analysis (fit-to-model)

- **Compression level control** — several web tools fix the level; we expose
  `level` 1-9 mapped to the bzip2 block size. ✅ covered.
- **Privacy / local processing** — the privacy-focused competitors run client-side;
  ours runs in-browser (chat SW) or locally via CLI, no server upload. ✅ on par.
- **Standard-compatible output** — verified the stream passes system `bzip2 -t` and
  round-trips through `bunzip2`, so it interoperates with every bzip2 toolchain. ✅.
- **Stronger ratio than gzip on text** — the headline bzip2 selling point; the tool
  copy states this, and the BWT pipeline delivers it. ✅.
- **Decompression (.bz2 → original)** — competitors that bundle bunzip2 do both
  directions. *Out of this tool's scope by design* (compress-only, like
  `gzip-compress`); a dedicated `bunzip2` tool would mirror the existing `gunzip`.
  Not a copy/UX gap — a separate tool.
- **Batch / multi-file** — server converters offer bulk upload. Out of model: the
  single-source descriptor + page driver take one input (same constraint as every
  other gizza compression tool). Not built.

No competitor copy, branding, or trademarks were used. No in-model
capability/copy/UX gaps remain; the one missing direction (decompress) is a
distinct tool, not a gap in this one.

## Sources
- [BZIP2 Compressor — utils.com](https://bzip2.utils.com/)
- [Bzip2 Compress File — thetoolapp.com](https://thetoolapp.com/utilities/bzip2-compress-file/)
- [BZ2 Converter — CloudConvert](https://cloudconvert.com/bz2-converter)
- [BZ2 Converter — AnyConv](https://anyconv.com/bz2-converter/)
- [bzip2 utility — PeaZip](https://peazip.github.io/bzip2-utility.html)
- [bzip2 — sourceware (reference implementation)](https://sourceware.org/bzip2/)
