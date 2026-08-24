# zstd-decompress — competitor analysis (2026-08-22)

Scan run **before** implementation, so the descriptor could include the table-stakes from the
start. All competitor observations are **paraphrased** — no competitor copy, branding, or
trademarks are reproduced anywhere in this repo.

## Search

One web search: *"online zstd decompress tool paste base64 zstandard decoder"*. The result set is
dominated by one operator (jsontotable.org) running several near-identical single-purpose pages,
plus a handful of independent dev-tool sites. Competitors skimmed:

1. **jsontotable.org — Base64→Zstandard decompressor** (reachable)
2. **jsontotable.org — Zstandard decompressor** (reachable; distinct page, file-oriented)
3. **onlinedevtools.dev — zstd compress/decompress** (reachable)

A fourth candidate, **hexmos.com freedevtools zstd-decompress**, returned only navigation chrome —
the tool interface never rendered for the fetcher. Per the scan rule it was **replaced** rather
than run with fewer, by pulling in the second jsontotable page as a distinct third profile.

## What competitors actually ship

| Capability observed | Where seen | Verdict |
|---|---|---|
| Paste Base64-encoded zstd payload | 1, 2, 3 | **in-model** → `data` + `encoding` |
| Decode + decompress in one step (RFC 4648 → RFC 8878) | 1, 2 | **in-model** → core behaviour |
| Size statistics: compressed, decompressed, expansion ratio | 1, 2 | **in-model** → `stats` |
| Copy result to clipboard | 1, 2, 3 | **in-model** — platform-provided (generator adds a Copy button to every page) |
| Download result as a file | 1, 2, 3 | **in-model** — platform-provided (`format = "text"` pages get a Download link) |
| Sample / "load sample" data buttons, several sizes | 1, 2, 3 | **in-model** → `[[example]]` preset chips |
| Client-side only, no upload, no account | 3 (explicit), others implied | **in-model** — already how every block works |
| Error handling for invalid/corrupt Base64 | 1 | **in-model** → typed errors naming what was expected |
| Stated soft size guidance (">5 MB slow", "10 MB may take longer") | 2, 3 | **in-model**, improved on → hard, *stated* caps with an explicit error |
| Upload a `.zst` file / drag-and-drop | 1, 2, 3 | **out-of-model for this page** (see below) |
| Zstd **compression** (encode side) | 3 and sibling pages of 1 | **out-of-model** (see below) |

Nothing else material was on offer. Every competitor is a two-box paste→result page; none exposes
any zstd-specific structure.

## Table-stakes → where each one landed

Every table-stake below ends in the descriptor or in the out-of-model list — none was dropped.

- Base64 input → `data` + `encoding = "base64"`
- Auto-detection of the input transport → `encoding = "auto"` (default)
- Size/ratio statistics → `stats = true`
- Sample data → five `[[example]]` chips on the page
- Copy / Download / Reset → provided by the shared page generator, not per-tool code
- Clear errors on bad input → typed, "expected X, got Y" messages throughout
- Size limits → 8 MiB compressed in / 16 MiB decompressed out, stated on the page **and** in the
  descriptor text, not only surfaced as an error

## Out-of-model (listed, deliberately not built)

- **`.zst` file upload / drag-and-drop.** This page is the paste-in, read-out half of the zstd
  story, matching the `brotli-decompress` precedent. The file half already exists in this repo:
  `file-compressor` takes a real `.zst` by URL or ref (`operation=decompress`, `format=zstd`) and
  returns a download. Duplicating a file picker here would fork one story across two pages.
- **Zstd compression.** The reference encoder is the C `zstd` library; `zstd-sys` needs a wasi C
  toolchain that isn't available for `wasm32-wasip1`, and the pure-Rust encoders available today
  warn of possible data loss — unacceptable for a compression tool. This is the same conclusion
  `file-compressor` already reached and documents. Decode-only is honest here.
- **Dictionary-compressed frames.** Decoding one requires the *user's* dictionary file as a second
  input, which the paste-in model has nowhere to put. Handled instead by **detecting** the case and
  erroring with the exact `Dictionary_ID` from the frame header, so the user knows precisely what is
  missing rather than seeing a generic decode failure.
- Accounts, cloud batch, API keys, paid tiers — none apply to a browser-local wasm tool.

## Where this tool goes beyond the field

These are not gaps being closed; they are capabilities no competitor scanned offers. All three are
consequences of driving `ruzstd`'s `FrameDecoder` frame-by-frame rather than calling a one-shot
streaming helper.

1. **Concatenated multi-frame streams decode in full.** A zstd stream is legally a *sequence* of
   frames (`zstd -c a b > out.zst`, and the output of parallel compressors, both produce these).
   `ruzstd`'s `StreamingDecoder` reports `is_finished()` at the end of the **first** frame, so the
   obvious implementation silently truncates such a stream — it returns partial data with no error,
   which is the worst failure mode a decompressor can have. This block loops over frames and
   concatenates them, and reports the frame count.
2. **Skippable frames are skipped, not fatal.** Frames with magic `0x184D2A50`–`0x5F` carry
   application metadata and are legal anywhere in a stream. They are stepped over and reported.
3. **Frame-header inspection + checksum verification** (`frame_info = true`): per frame, the
   window size, the declared content size (and whether it was declared at all), the dictionary ID,
   and whether the trailing xxHash-32 content checksum is present *and* whether it matched what was
   recomputed during decoding. A mismatch is a hard error, not a silent pass — no competitor here
   verifies the checksum at all.

Additionally, hex input (`encoding = "hex"`, `0x` prefix and whitespace tolerated) and hex/Base64
**output** rendering are supported, so a payload that decompresses to binary is still inspectable
instead of failing on UTF-8 — competitors only offer text out.

## Design decisions recorded

- **Auto-detect is verified, not guessed.** Unlike Brotli, zstd *has* a magic number
  (`28 B5 2F FD`), so `encoding = "auto"` decodes the paste as both hex and Base64 and keeps
  whichever yields bytes that actually start with a zstd (or skippable) frame magic — a short
  Base64 string can consist entirely of hex characters, so picking by shape would be wrong.
- **Wrong-codec blobs are diagnosed up front.** Because the magic number exists, a gzip/xz/LZ4/ZIP/
  bzip2/7-Zip/tar/zlib/Brotli-shaped payload is named — along with the sibling tool that handles
  it — *before* any decode is attempted, rather than after a failure as `brotli-decompress` must do.
- **`stats` and `frame_info` are separate booleans.** `stats` matches what competitors show (sizes
  + ratio, the common case); `frame_info` is the structural view. Folding them together would force
  users who want one to read the other.
