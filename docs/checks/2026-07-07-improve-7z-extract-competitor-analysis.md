# 7z-extract — competitor analysis (2026-07-07)

Tool: `gizza-ai/7z-extract` — extract a `.7z` (7-Zip) archive, including
AES-256-encrypted ones, and return every file repacked into a single ZIP.
Surfaces: chat + CLI, **no page** (a ZIP/binary output has no page render mode —
same no-page file-input pattern as `blocks/archive-extractor`, `unzip`,
`zip-inspect`, `extract-tar`). All UX below is judged against that model.

## Competitors scanned (top 3 real, in-browser)

1. **ezyZip — 7z extractor** (paraphrased): client-side extraction, no upload;
   prompts for a password on encrypted archives; shows names + a
   folder/full-list toggle; per-file and "extract all" downloads; convert-out to
   zip/tar/tar.\* variants; preview some file types; free size cap ~2 GB.
2. **FormatFuse — 7z extractor** (paraphrased): WebAssembly client-side, no
   upload, works offline after load; password prompt; browse contents and pick
   which files to extract; notes LZMA/LZMA2 plus PPMD/BCJ/BCJ2/BZip2/Deflate
   codecs; supports split `.001/.002` multi-part archives; ~2 GB cap.
3. **ZIP Extractor (Google Workspace)** (paraphrased): opens zip/rar/7z/tar and
   password-protected archives from Drive/Gmail/local; browser-based.

(Descriptions paraphrased from public feature pages — no competitor copy,
branding, or trademarks reproduced.)

## Table-stakes → fit-to-model decisions

| Capability | In/out of model | Where it lands |
|---|---|---|
| Extract a `.7z` archive | **in** | core `extract_to_zip` (sevenz-rust2 `ArchiveReader`, in-memory) |
| AES-256 password-protected archives | **in** | `password` descriptor param + `aes256` feature; header-encrypted (`-mhe=on`) supported |
| LZMA + LZMA2 codecs (7z defaults) | **in** | always-on sevenz-rust2 decoders; verified against real files |
| BCJ / Delta filters | **in** | built into sevenz-rust2; verified (`lzma2delta_1.7z`) |
| List members (names, sizes, dirs) | **in** | `Extracted.entries` → member listing in the response |
| Download extracted output | **in** | repacked into one ZIP `data:` envelope (chat download / CLI writes file) |
| Client-side / no upload / private | **in** | pure Rust; runs in the chat Service Worker and locally in the CLI — no server round-trip |
| Path-traversal safety | **in** | `safe_path` normalizes to relative paths (drops `..`, leading `/`, drive prefixes) |
| BZip2 / Zstd 7z codecs | **out** | the `bzip2`/`zstd` sevenz-rust2 features pull **C-binding** crates that do not instantiate under wasm32-wasip1 — excluded; graceful "unsupported 7z compression method" error |
| PPMd / Brotli / Deflate 7z codecs | **out** | pure-Rust but disabled to keep the wasm lean and instantiation-safe; rare in real `.7z` (which defaults to LZMA2). Same graceful error path |
| Split / multi-part archives (`.001`, `.002`) | **out** | the chat/CLI input model is a single `url`/`ref` source; multi-part needs multi-file upload (cf. the multi-input-ffmpeg limitation) |
| Selective / interactive "pick which files" | **out** | non-interactive tool: extracts all members into one ZIP (the caller unzips the parts they want) |
| In-browser preview of images/audio/docs | **out** | no interactive page for a ZIP output; preview is a page-only UX |
| Convert-out to tar/tar.gz/… variants | **out** | one canonical output (ZIP); the existing `archive-extractor` + compressor tools cover re-packing |
| ~2 GB size cap | **partial** | ours caps at 64 MiB compressed input / 256 MiB uncompressed (decompression-bomb guard); stated in the tool description |

### UX control patterns (competitor pages) vs. our surfaces

Competitors ship drag-drop upload, a password prompt, per-file "Save"/"Save All"
buttons, and a search box — all **page-interactive** controls. For a no-page
chat+CLI tool these map to: the `url`/`ref` source (the archive) and the
optional `password` parameter. No sliders / color pickers / preset chips are
relevant (no page). Every table-stake above is therefore either realized in the
descriptor or explicitly listed out-of-model — none dropped silently.

## Verification (what was actually run)

- **Unit (core):** plain LZMA2 multi-file + subdir extract; AES-256 extract with
  the correct password (full decrypt round-trip); wrong password errors; missing
  password on a header-encrypted archive asks for one; non-7z rejected; empty
  rejected; signature detection; path-traversal sanitization; output-name mapping.
- **wafer build:** instantiates clean under wasm32-wasip1 (1097 KiB) — dead-code
  GC drops sevenz-rust2's std::fs `open`/`decompress` helpers (we only use the
  in-memory `ArchiveReader::new`), so no missing-WASI-import failure.
- **CLI (real public `.7z` URLs, py7zr test data):** LZMA (`lzma_1.7z` →
  `test1.txt`), LZMA2+Delta (`lzma2delta_1.7z` → `src/bra.txt`), header-encrypted
  no-password → password hint, header-encrypted wrong-password → "incorrect
  password", non-7z (a zip) → "not a .7z archive". The AES path runs through
  wasmi in the CLI.
- **Hygiene gate:** `check-tool-hygiene.py 7z-extract` exits 0.

No page → no Playwright / generator page (the generator skips blocks without
`page/meta.toml`; confirmed at `tools/generator/src/main.rs:133`).
