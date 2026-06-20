# gunzip — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/gunzip` — decompress a gzip (.gz) file/blob back to its
original bytes, returned as a downloadable file. Chat + CLI (no page: a file→file
output fits neither the pure-text nor the ffmpeg media page shape — the F3
no-page file-input pattern, like extract-tar / gunzip's sibling extract tools).

## What competitors do

- **Online gzip/un-gzip sites** (gzip.swimburger, unzip-online, ezyzip,
  toolslick) — upload a `.gz`, download the original. Strengths: simple.
  Weaknesses: the file is **uploaded** (privacy + size caps), ads, and some
  conflate `.gz` with `.tar.gz` (extracting the tar) which surprises users who
  just want the raw decompressed member.
- **`gunzip` / `gzip -d` (gzip)** — the reference CLI, fully local, but requires
  a shell.
- **Language libs** (`zlib`, Python `gzip`) — need a runtime + code.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`flate2`/miniz_oxide)
   compiled to wasm: runs in the chat Service Worker and headless via the CLI.
   The file never leaves the device.
2. **Recovers the original filename.** gzip can embed the original name (FNAME);
   this tool reads it from the header and names the output accordingly, falling
   back to stripping the `.gz` suffix — so you get `report.csv`, not
   `report.csv.gz.out`.
3. **Clear separation from tar.** It decompresses the single gzip member (the raw
   bytes), and the description points `.tar.gz` users to the dedicated
   `extract-tar` tool — no surprising auto-untarring.
4. **Bomb-guarded.** The inflate is capped at 256 MiB of output, so a gzip bomb
   is rejected instead of exhausting memory.
5. **Chainable.** Takes a `url` or a prior tool's `ref`, and the decompressed
   output is itself a `ref` you can feed into the next tool (e.g. gunzip → then
   detect-file-type / extract-tar).

## Honest scope

- Single-member gzip streams (the common case). Multi-stream concatenated gzip
  isn't merged (the first member is returned).
- Output is delivered as `application/octet-stream` (a generic download); pair
  with `detect-file-type` if you want to identify what came out.

## Tests

4 core unit tests built with `flate2::GzBuilder`: round-trip **with** an embedded
filename (bytes match, name recovered), round-trip **without** a filename (name
None), rejects non-gzip / empty / 1-byte input, and errors on a truncated gzip
stream. Plus the block drift-guard schema test. CLI verified over the wire:
gunzip of `left-pad-1.3.0.tgz` (a 3619-byte gzip-of-tar) produced a valid
17920-byte **uncompressed tar** that Python's `tarfile` opens in `r:` mode and
reads all 10 members from.
