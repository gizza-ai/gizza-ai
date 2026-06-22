# carve-files — competitor analysis snapshot (2026-06-22)

## Tool
`gizza-ai/carve-files` — scan a binary blob (disk image, memory dump, corrupted
container, or any concatenation of files) for embedded files by their **magic-byte
signatures** and extract the recovered files ("file carving"). Returns each recovered
file with its byte **offset**, **size**, detected type (extension / human format name /
MIME), an `exact_end` flag (true when the end was found by a trailer or a declared length,
false when it is a best-effort run to the next signature/blob end), and its content inline
as **base64** up to an internal budget (larger files are listed without content). Recognises
PNG, JPEG, GIF, BMP, PDF, ZIP, gzip, bzip2, XZ, 7-Zip, RAR, Ogg, FLAC, MP3 (ID3), RIFF
(WAV/AVI/WEBP), and SQLite. Pure-Rust (only `base64`), runs on ALL backends including the
chat Service Worker. Surfaces: chat + CLI (no page — binary file→JSON, the no-page
file-input pattern, like `unzip` / `extract-tar` / `detect-file-type`).

## Top competitors surveyed

1. **Foremost** (https://foremost.sourceforge.net/) — the classic signature-based carver.
   Header/footer rules in `/etc/foremost.conf`; recovers JPEG/GIF/PNG/BMP/PDF/Office/ZIP/
   etc. Uses both a header magic and an optional footer (trailer) plus a max size — the same
   header+trailer model this tool uses. Writes recovered files to an output dir + an audit
   log; no inline content, no byte offsets in the primary output.
2. **Scalpel** (https://github.com/sleuthkit/scalpel) — a foremost rewrite with a faster
   two-pass engine and the same `scalpel.conf` header/footer/max-size rule format. Same
   recovery niche; tuned for large media. File-system output, no JSON, no inline content.
3. **binwalk** (https://github.com/ReFirmLabs/binwalk) — firmware-oriented: magic-signature
   scan **plus entropy analysis** and **recursive/nested extraction** (carves an archive,
   then re-scans what it extracted). Reports byte offsets in its scan table. The richest of
   the surveyed tools; entropy + recursion are its differentiators.
4. **PhotoRec** (https://www.cgsecurity.org/wiki/PhotoRec) — recovery-focused carver over
   480+ formats, reads partitions/whole disks directly, understands some filesystem hints.
   GUI/TUI desktop app; writes files out, no programmatic JSON.
5. **bulk_extractor** (https://github.com/simsong/bulk_extractor) — scans for features
   (emails, URLs, ccn) and can carve, with offset reporting; oriented at feature extraction
   over whole-file recovery. Desktop/CLI, no inline content.

## Gap diff & ranking (fit-to-model)

- **Distinct from existing gizza blocks (no dup):** `unzip` / `extract-tar` decode a *known*
  container's directory; `detect-file-type` identifies the *single* type of one whole blob.
  `carve-files` is the only block that scans an *arbitrary* blob for *multiple* embedded
  files at unknown offsets and cuts each out. Confirmed against `ls blocks/ | grep -iE
  'carve|extract|detect|recover'` — no overlap. Kept as its own tool.
- **Byte offsets in output:** present (`offset` per file) — matches binwalk's offset table and
  exceeds foremost/scalpel/photorec, which only write files out. In model, done.
- **Header + footer (trailer) carving:** present — PNG (IEND+CRC), JPEG (FFD9 EOI),
  GIF (00 3B), PDF (`%%EOF`), ZIP (EOCD) carve to an exact end (`exact_end: true`), mirroring
  foremost/scalpel's header/footer rules. Declared-length formats (BMP, RIFF) read the
  in-band size. In model, done.
- **Inline recovered content (base64):** present and a genuine advantage for an LLM/CLI
  surface — competitors write files to disk; here each carved file's bytes come back inline
  (within a budget) so a follow-up tool (`detect-file-type`, `unzip`, image tools) can act on
  a `ref` directly. In model, done.
- **Format coverage:** covers the common image/doc/archive/audio/video/db signatures that
  foremost/scalpel ship by default. PhotoRec's 480+ and binwalk's firmware-specific signatures
  are far larger; expanding the table is incremental and **in model** — added the highest-value
  common set; more signatures can be appended later without an architecture change.
- **Entropy analysis (binwalk):** flagging encrypted/compressed regions by Shannon entropy is a
  distinct analysis feature. `gizza-ai/byte-entropy` already covers entropy as its own block, so
  this is **out of scope for carve-files** (would duplicate that tool) — noted, not built here.
- **Recursive / nested extraction (binwalk):** carving an archive then re-scanning its contents
  is multi-pass and better composed at the chat/agent layer (carve → `unzip` a recovered `ref` →
  carve again) than baked into one block; the single-pass carve here is the right unit. Listed
  as a known limitation, not built. No competitor copy/branding/trademarks were used.

## Verification (this snapshot)
- Unit tests (6): two concatenated images carved at correct offsets with exact trailers; PDF
  carved to `%%EOF`; BMP via declared LE size; budget omits data but still lists the file; empty
  / no-signature blobs return zero; gzip (heuristic end) runs exactly to the following PNG.
- Chat: `wafer build` validates the block instantiates (491 KiB) in wasm32-wasip1; schema-drift
  test asserts the LLM-facing schema is the exact `url`⊕`ref` `oneOf`.
- CLI: `gizza tool carve-files url=https://cdn.openai.com/papers/whisper.pdf` → one PDF,
  `offset 0`, `exact_end: true`, `mime application/pdf`, base64 content present (exit 0).
- No page surface (binary file→JSON; the no-page file-input pattern), stated explicitly.
