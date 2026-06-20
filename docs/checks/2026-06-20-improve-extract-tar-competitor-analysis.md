# extract-tar — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/extract-tar` — list + extract the files from a tar / tar.gz /
tgz archive, returned repacked as a ZIP. Chat + CLI (no page: a ZIP output fits
neither the pure-text nor the ffmpeg media page shape — the F3 no-page
file-input pattern, like extract-pdf-images / encrypt-file).

## What competitors do

- **Online "extract tar / open tar.gz" sites** (extract.me, ezyzip, archive
  extractors, B1 online) — upload an archive, browse/download members. Strengths:
  many formats (tar, zip, rar, 7z). Weaknesses: the archive is **uploaded to a
  server** (privacy + size caps), ads, and many can't handle `.tar.gz` vs `.tgz`
  vs `.tar` transparently or choke on large member counts.
- **`tar` CLI / 7-Zip / GUI unarchivers** — fully capable but require installing
  software and a shell; not browser- or agent-friendly.
- **Language libs** (Rust `tar`, Python `tarfile`) — powerful but need code.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (tar + flate2 + zip) compiled
   to wasm, so it runs in the chat Service Worker and headless via the CLI. The
   archive never leaves the device.
2. **Auto-detects gzip.** `.tar`, `.tar.gz`, and `.tgz` all just work — the gzip
   magic (`1F 8B`) is sniffed and inflated before the tar is parsed, so the user
   doesn't pick a mode.
3. **One tidy ZIP + a full listing.** Every regular file is repacked into a
   single ZIP with original relative paths preserved; the response also lists all
   members (files and directories) with sizes, so you see the archive's shape at
   a glance. The ZIP is one downloadable file and a chainable `ref`.
4. **Security-hardened extraction.** Member paths are sanitized — leading `/`,
   `..` traversal, and `\`/drive separators are stripped — so a malicious tar
   can't smuggle absolute or escaping paths (the classic "tar-slip"). Tested.
5. **Bomb guards.** Caps on entry count (10k) and total uncompressed size
   (256 MiB), with the gzip inflate itself bounded, so a decompression bomb is
   rejected rather than exhausting memory.
6. **Zero-config & free** — no size caps beyond the safety bounds, no account.

## Honest scope / limitations

- Output is a **ZIP** repack (so it slots into the gizza single-file envelope and
  is openable everywhere), not the original tar members written to disk.
- Only **gzip**-compressed tars are auto-inflated; `.tar.bz2`, `.tar.xz`,
  `.tar.zst` are not (would need bzip2/xz/zstd decoders — could be added, those
  crates are available, but kept out of this MVP). A plain `.tar` always works.
- Symlinks/hardlinks and special files are listed but not packed (only regular
  files go into the ZIP).

## Tests

4 core unit tests on **archives built in-test with `tar::Builder`**: a plain tar
with two files + a directory → ZIP has both files with correct paths/contents and
the directory is listed; the same tar gzipped → handled identically; empty and
garbage input error; `safe_path` strips `/etc/passwd`→`etc/passwd`,
`../../secret`→`secret`, `a/./b/../c`→`a/c`, and `dir\file`→`dir/file`. Plus the
block drift-guard schema test. The `tar` crate (with `filetime`) was confirmed to
compile to `wasm32-wasip1`. CLI verified over the wire on the real
`left-pad-1.3.0.tgz` npm tarball → a valid ZIP with all 10 members, paths
(`package/…`) preserved and contents intact.
