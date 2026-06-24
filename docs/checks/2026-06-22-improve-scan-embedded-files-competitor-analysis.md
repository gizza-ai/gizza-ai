# scan-embedded-files — competitor analysis (2026-06-22)

Tool: `gizza-ai/scan-embedded-files` — scan a file for embedded/appended file
signatures (magic numbers) anywhere in the bytes, not just at offset 0, to
surface hidden / appended / concatenated payloads (polyglots, carved EXEs,
ZIP-in-PNG, PDF-in-image, steganographic carriers).

Surfaces: chat (LLM tool) + CLI. **No standalone page** — a file→JSON report is
the F3 "no-page file-input" shape (same as `detect-file-type`,
`pdf-extract-text`): it fits neither the pure-text page nor the ffmpeg
file→media page driver.

## Competitors surveyed (top 5)

1. **binwalk / binwalk.sh** — the de-facto firmware/binary signature scanner.
   Scans for hundreds of signatures (filesystems, bootloaders, compression,
   certificates, ELF/PE), maps where each region sits, draws a Shannon-entropy
   curve, and can **carve/extract** the embedded files to a ZIP. Online version
   gates extraction + entropy behind a paid tier; identification is free.
2. **`file -k` / libmagic (`file --keep-going`)** — the canonical CLI. With `-k`
   it keeps going past the first match and lists *multiple* file types for a
   polyglot. Huge curated magic database.
3. **magic-bytes.js (WuTools / Just File Tools "File Type Detector")** — browser,
   no-upload magic-byte detectors. Identify the *leading* type only; they do not
   scan the interior for appended payloads.
4. **Foremost / Scalpel** — classic file-carving forensics tools: scan raw data
   for signatures and **extract** complete files (data recovery / carving).
5. **Polydet (Polyglot detector)** — focuses on the formats common in polyglots
   (HTML/JS/PDF/ZIP + PNG/JPG/MP3) and flags abnormal/extra data chunks.

## Capability diff (us vs them)

| Capability | scan-embedded-files | binwalk | file -k | magic-bytes.js | foremost |
|---|---|---|---|---|---|
| Scan interior (not just offset 0) | yes | yes | partial (-k) | no | yes |
| Per-hit byte offset | yes | yes | no | n/a | yes |
| Leading/declared type reported | yes | yes | yes | yes | partial |
| "appended past logical end" flag | **yes** (PNG/GIF/JPEG-anchored) | implicit (offset map) | no | no | no |
| MIME + format-name + extension per hit | yes | partial | partial | yes | no |
| Polyglot / hidden-payload focus | yes | yes | partial | no | partial |
| Runs fully local / private | yes (browser-local, no upload to a server) | sandbox upload | local | local | local |
| Archive/exe/font/db signatures | yes (zip/rar/7z/gz/xz/zstd/ELF/Mach-O/Wasm/Java/SQLite/woff…) | yes (more) | yes (most) | many | some |

## Gaps + decisions (fit-to-model)

In-model gaps we addressed / already cover:
- **Interior scan with byte offsets** — core scans the whole buffer and reports
  every hit with its offset (the key delta over the existing `detect-file-type`,
  which only reads offset 0).
- **Appended-payload flagging** — `leading_logical_end()` computes the host
  format's structural end for PNG (after `IEND`+CRC), GIF (trailer `0x3B`), and
  JPEG (`FFD9` EOI); any hit at/after that end is flagged `appended` (the strong
  polyglot signal — e.g. ZIP appended to a PNG). Verified by unit test and the
  `octocat/Hello-World` ZIP / live QR PNG CLI runs.
- **Rich per-hit metadata** — extension + human kind + MIME for each hit, plus a
  top-level `leading_type`, `embedded_count`, `has_appended_payload` summary so
  the LLM/CLI gets an at-a-glance verdict.
- **Low false-positive signature set** — only distinctive 3+ byte (or anchored)
  magics, longest-match-wins at each offset; a `random_binary_has_few_false_hits`
  test guards against noise on arbitrary data.

Out-of-model features we deliberately do NOT build (stated, not copied):
- **Carving/extraction to files** (binwalk/foremost) — extracting a sub-file
  needs the host format's length parsing per format and a multi-file media
  output the gizza page/envelope model doesn't support; identification +
  offsets is the in-model scope. A user can carve with the reported offset.
- **Shannon-entropy curve** (binwalk) — a visualization output; the page driver
  renders single media/text, not a chart, for a file-input tool. (Entropy *as a
  number* could be a future minor add, but it's not what distinguishes this
  tool.)
- **Filesystem / firmware signatures** (SquashFS/JFFS2/UBI, bootloaders) —
  niche firmware-forensics scope; the general formats above cover the common
  hidden-payload cases. Easy to extend the `SIGS` table later if demanded.

No competitor copy, branding, or trademarks were reused — descriptions are
original and describe gizza's own behavior.

## Verification

- `cargo test` (block schema drift-guard) + `cargo test -p …-core` (8 unit
  tests incl. zip-appended-to-png, embedded-pdf-in-jpeg, multi-signature order,
  false-positive guard) — all pass.
- `wafer build` — block.wasm instantiates (486 KiB).
- `cargo install --path cli` + generator — tool registered (`gizza list` shows
  it; landing page rendered with 183 tools).
- CLI live runs: `octocat/Hello-World` ZIP → leading ZIP + 2 interior PK hits at
  offsets 58/289, no appended payload; live QR PNG → clean leading PNG, 0
  embedded.
- No page surface (F3 file→JSON pattern) → no Playwright spec, by design.
