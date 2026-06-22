# lz4-compress — competitor analysis (2026-06-22)

Tool: `blocks/lz4-compress` — compress a file (or any bytes) into the standard
LZ4 **frame** format (`.lz4`, magic `0x184D2204`), returned for download.
Pure-Rust (`lz4_flex`), so it runs on every backend including the chat Service
Worker. Surfaces: chat + CLI. No standalone page (file→file output, the no-page
file-input pattern, same as `gzip-compress` / `bzip2-compress`).

## Surfaces verified (Phase 1)

- **Chat block**: `wafer build` validates the `block.wasm` instantiates (508.7 KiB) —
  `lz4_flex` (+`twox-hash` for the frame xxhash checksum) instantiates clean in
  wafer (wasm32-wasip1). Pure Rust, so the chat SW path is functional (unlike
  ffmpeg tools).
- **CLI**: `gizza tool lz4-compress url=…/COPYING` →
  `compressed COPYING (496 bytes) → COPYING.lz4 (443 bytes LZ4, ~11% smaller)`.
  Output verified to start with the standard LZ4 frame magic `04 22 4d 18`
  (little-endian `0x184D2204`), so it is interoperable with the reference
  `lz4 -d` / `unlz4`.
- **Page**: none. A file→file (opaque-bytes) tool has no page render mode, matching
  the established no-page file-input pattern of the sibling compressors.
- Unit tests: 4 core round-trip tests (incl. empty + binary input) + 1 chat-schema
  drift-guard. All pass.

## Competitors surveyed (top 5)

1. **lz4 CLI / liblz4** (lz4.org, github.com/lz4/lz4) — the reference
   implementation. Default output is the LZ4 frame format (`.lz4`). Offers a speed
   knob (`-1`..`-12`, `--fast`), an HC (high-compression) variant, multi-frame
   concatenation, content/block checksums, and a legacy frame mode.
2. **IO Tools — LZ4 Compression Encoder/Decoder** (iotools.cloud) — browser tool:
   paste text or upload a file; compress **or** decompress mode toggle.
3. **Beautify Code — LZ4 Compression Online** (beautifycode.net) — paste text →
   LZ4; copy result.
4. **Convert.Guru — .LZ4 Converter** (convert.guru) — detects the LZ4 format and
   reads/extracts the contained data (decompress + convert to zip/tar/gz).
5. **Generic "online file compressor" sites** — bundle gzip/zip/bzip2/lz4; upload a
   file, pick the codec, download the result, and show the before/after size +
   savings %.

## Capability diff + gap ranking (fit-to-model)

| Capability | Competitors | lz4-compress | Verdict |
|---|---|---|---|
| Standard LZ4 **frame** output (`.lz4`, interoperable with `lz4 -d`) | lz4 CLI, most | yes (verified magic) | matched |
| File input (any bytes) | most | yes (`url`⊕`ref`, up to 64 MB) | matched |
| Before/after size + savings % | generic compressors | yes (`for_llm` reports in/out bytes + %, incl. "larger" on tiny inputs where the ~15 B frame header dominates) | matched |
| Frame checksums (xxhash) | lz4 CLI | yes (`lz4_flex` frame default) | matched |
| **Decompress mode** | IO Tools, Convert.Guru | no | OUT OF SCOPE — this is a *compress* tool; a separate `lz4-decompress` / `unlz4` tool would mirror the `gunzip`/`lzma-decompress` split already in the catalog. Not built here. |
| Paste-**text** input | IO Tools, Beautify Code | no (file/url/ref only) | minor; matches the catalog's other file-compressors (gzip/bzip2). A paste-text path is the chat message itself; the file-input surface is the right fit for this pattern. |
| Compression-**level** / HC knob (`-1..-12`) | lz4 CLI | no | OUT OF MODEL for `lz4_flex`: the crate's frame encoder exposes the single default fast mode and **no** level/HC selection, so there is no knob to expose. Documented in the skill copy ("LZ4 has a single fast mode, so there is no level/quality option") so the LLM never offers a non-existent option — strictly honest vs. competitors that imply a level. |
| Legacy frame mode | lz4 CLI | no | deliberately omitted — the modern frame format is the default everywhere; legacy is a compatibility wart. |

## Copy / UX changes made

- Skill `description` rewritten to be accurate and LLM-actionable: states the LZ4
  frame format + `.lz4` extension, the speed-over-ratio trade-off and typical use
  cases (logs/streaming/real-time), the absence of a level knob (so the model
  doesn't hallucinate one), the `url`⊕`ref` input, and the exact inverse
  (`lz4 -d` / `unlz4`).
- `for_llm` size report fixed to use a signed delta so tiny inputs report
  "~N% larger (frame overhead on tiny input)" instead of an unsigned-subtraction
  underflow (caught during Phase-1 CLI verification on a 13-byte input).
- Output named `<input>.lz4` with mime `application/x-lz4`, consistent with the
  sibling compressors' `<input>.bz2` / `<input>.gz`.

## Out-of-model (not built — would need work outside the current model)

- **Decompress** (`lz4-decompress` / `unlz4`): a separate file-input tool; in
  scope for the catalog but a distinct tool, not this one.
- **Compression level / HC mode**: not exposed by `lz4_flex`'s frame encoder.
- **Standalone page**: file→file opaque-bytes output has no page render mode.
