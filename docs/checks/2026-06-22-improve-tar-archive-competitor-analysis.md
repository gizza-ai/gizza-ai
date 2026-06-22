# tar-archive — competitor analysis (2026-06-22)

New tool built end-to-end this run, then improved against the top online
TAR/tar.gz creators. `tar-archive` bundles multiple files (URL or `ref`
attachment) into a single downloadable archive: plain `.tar` or compressed
`.tar.gz` / `.tar.bz2` / `.tar.xz`. Browser-local, pure-Rust wasm, no account,
no server. Surfaces: **chat + CLI** (no page — array input + binary output,
same shape as `create-zip`).

All competitor notes below are **paraphrased**; no copy, branding, or
trademarks reproduced.

## Competitors surveyed (6)

| Tool | Segment | Output formats | Options | Notes |
|---|---|---|---|---|
| ezyZip — Create TAR.GZ | browser-local (WASM) | tar, tar.gz, tar.bz2, tar.xz, tar.zst, tar.lz4, 7z, zip | compression level; preserves folders; password for ZIP only | segment leader: per-format SEO pages, lazy-loaded codecs, Dropbox import |
| ToolsHive — Files to TAR | browser-local | tar, tar.gz | single TAR/TAR.GZ toggle | privacy/no-account framing, ~100 MB guidance |
| FormatFuse — TAR Creator | browser-local | tar, tar.gz | archive-name field; TAR/TAR.GZ toggle; preserves folders | instant single-step download, ~2 GB memory cap |
| CloudConvert | server-side | tar, tar.bz2, 7z, rar, zip (+reverse) | per-job conversion settings | conversion-first; accounts / paid tiers / API |
| Aspose.ZIP | server-side | zip, 7z, tar, cpio | output-format selector | 250 MB cap, enterprise framing, 24 h deletion |
| online-convert (Archive) | server-side | gz/tar.gz focus | gz compressor | URL/clipboard input, multi-language, extension funnel |

## Cross-cutting findings

- Two segments: **browser-local privacy tools** (ezyZip, ToolsHive, FormatFuse)
  — our direct peers — and **server-side converters** (CloudConvert, Aspose,
  online-convert) which lean on accounts/paid tiers and contradict the
  browser-local thesis.
- Within the local segment, **only ezyZip offers the long-tail format set and a
  compression-level control**; ToolsHive/FormatFuse expose only a binary gzip
  toggle.
- No local tool offers entry-rename or tar password. Our automatic
  duplicate-name disambiguation (`name (2).ext`) is at parity / slightly ahead.

## Gap analysis vs. our tool

### Closed this run (in-model)
- **Multiple compression formats.** Replaced the initial binary `gzip` toggle
  with a `compression` enum: `none` / `gzip` / `bzip2` / `xz`, producing
  `.tar` / `.tar.gz` / `.tar.bz2` / `.tar.xz`. This closes the primary gap vs.
  ezyZip (the only local competitor offering more than gzip). Implemented by
  **reusing the existing wasm-safe encoders** from the standalone
  `bzip2-compress` (`banzai`) and `lzma-compress` (`lzma-rust2`) cores, so
  output is byte-compatible with those tools and validated cross-tool with
  `tar -tzf / -tjf / -tJf`.
- Multi-file input is already at parity (array of url/ref sources), and
  deterministic output (fixed mtime) is a correctness nicety none advertise.

### Considered, not built (out-of-model — conflicts with browser-local/no-account/no-server)
- **Accounts / paid tiers / API** (CloudConvert, online-convert) — breaks the
  no-account promise.
- **Server-side cloud batch & large-file processing** (CloudConvert, Aspose,
  online-convert) — requires server compute.
- **Password / encrypted archives** (ezyZip, ZIP only) — tar has no native
  encryption; nobody offers it for tar either.
- **Format-to-format conversion incl. RAR decode** (CloudConvert, Aspose) —
  we are a bundler, not a converter; RAR is licensing/infra-heavy.
- **Remote-file fetch by arbitrary URL** in the page/browser — needs a CORS
  proxy. (The chat/CLI surfaces do resolve URLs server-side via the runtime
  fetcher, SSRF-guarded.)
- **tar.zst / tar.lz4** (ezyZip) — deferred: no proven wasm-safe zstd/lz4
  *streaming* encoder wired in this repo yet; the three added formats cover the
  bulk of the long-tail demand. Revisit if a wasm-safe zstd core lands.

### UX notes
- No standalone page (array + binary-output tools have no page render mode in
  this stack, same as `create-zip`), so a custom archive-name field / drag-drop
  UI would have no surface to live on; we use a deterministic `archive.<ext>`
  download name. Archive-name customization is therefore not applicable here.

## Verification (this run)

- `cargo test --workspace`: 10 pass — drift-guard schema match + core
  (gzip/bzip2/xz round-trips, dedup, determinism, empty/blank-name, alias parse).
- `wafer build`: chat block validates (730 KiB) — bzip2/xz encoders instantiate
  in wasm32-wasip1.
- CLI: all four `compression` values produce archives that validate with system
  `tar` (`-tf`, `-tzf`, `-tjf`, `-tJf`); duplicate names disambiguate; invalid
  compression returns a clean error.
- Generator runs clean (195 tools). No page surface (stated above).
