# lzma-decompress — competitor analysis (2026-06-21)

Tool: **gizza-ai/lzma-decompress** — decompress an LZMA `.xz` (or legacy `.lzma`)
file back to its original bytes, returned as a downloadable file. Pure-Rust
(`lzma-rust2`), surfaces: **chat + CLI** (no standalone page — file→file output
fits the no-page file-input pattern, like gunzip / lzma-compress).

## Surfaces verified

- **Chat block**: `wafer build` OK (validates, 559 KiB block.wasm).
- **CLI**: `gizza tool lzma-decompress url=…` decompresses a real `.xz`
  (tukaani test vector `good-1-check-crc32.xz`) → `Hello\nWorld!\n`, byte-for-byte
  equal to `xz -d`; filename correctly stripped of the `.xz` suffix. Non-xz input
  returns a clean error (no panic). Both `.xz` and legacy `.lzma` decode natively
  (unit-tested + verified against `xz` / `xz --format=lzma` output).
- **Page**: none (no-page file tool, by design).

## Top-5 competitors

| Tool | Formats | Max size | Output | Notable UX |
|------|---------|----------|--------|-----------|
| extract.me | 70+ (7z, rar, xz, lzma, gz, tar, iso, zip…) | ~10 GB (premium) | per-file / save-all-as-zip | drag-drop, password archives, multi-part, cloud import |
| ezyZip | 130+ (tar.xz, .txz, tar.lzma, 7z, rar…) | 2 GB free | per-file / save-all (keeps tree) | client-side WASM, in-archive search, media preview |
| unziper | zip, rar, 7z, tar, gzip, lzma, xz… | 4 GB (RAM-bound) | individual / download-all | drag-drop, password field, file-list preview |
| utils.com (unxz) | .xz, .tar.xz | n/a | limited | drag-drop; admits JS LZMA is slow/incomplete |
| FormatFuse | .xz (single-purpose) | n/a | minimal | clean single-purpose page, privacy messaging |

All are client-side/privacy-first except extract.me (server-side, richest breadth).
Multi-format archive handling (.7z/.tar/.rar) and "download all as zip" are table
stakes for the broad tools; the dedicated `.xz` tools (FormatFuse, utils.com) are thin.

## Gap analysis

### In-model gaps — addressed in this tool
- **Legacy `.lzma` ("alone") support** alongside `.xz` — auto-detected from magic
  bytes; verified against `xz --format=lzma` output. (closes the gap vs utils.com /
  FormatFuse which are `.xz`-only.)
- **Concatenated multi-stream `.xz`** — decoded as one (`allow_multiple_streams = true`).
- **Correct output filename derivation** — strips `.xz` / `.lzma`, and `.txz` → `.tar`.
- **Robust error messaging** for truncated / wrong-magic / corrupt input (clean
  `Err`, never a panic).
- **Decompressed-size readout** — `for_llm` reports in→out bytes and detected format.

### In-model, intentionally NOT added
- **Plain `.gz` decompress** — already covered by the existing `gunzip` tool; adding
  it here would duplicate that block. (Kept this tool focused on the LZMA family.)
- **`.lz` (lzip)** — distinct container; out of scope for an "LZMA/.xz" tool, and a
  separate low-demand format.

### Out-of-model (not for a single-file, no-container pure tool)
- `.tar.xz` / `.tar.lzma` **extraction** (tar parsing → many files); this tool
  returns the inner `.tar` as a single download (correct), but does not unpack it —
  pair with `extract-tar`.
- `.7z`, `.rar`, `.zip`, `.iso` container browsing/extraction.
- Multi-file output / "Download All as ZIP", folder-tree preservation.
- Multi-part / split archives, password-protected archives, batch queues,
  cloud import/export, drag-drop UI flourishes (no page surface here).

## Known limitation (honest)

The chat/CLI runtime runs in a constrained wasm linear-memory sandbox. Very large
**decompressed outputs** (~10 MiB and up — e.g. a full `Python-x.y.z.tar.xz` that
expands to ~100 MiB) can exhaust the wasm heap (graceful "out of memory" error, or a
runtime abort on the largest). The core decodes these correctly natively; the limit
is the sandbox heap, not the codec. Typical small/medium `.xz` / `.lzma` files (the
overwhelming majority of single-file use) decompress fine on both surfaces. The CLI
fetcher also caps remote downloads at 50 MiB (compressed) and does not follow
redirects, so use a direct, non-redirecting URL. No competitor copy/branding was used.
