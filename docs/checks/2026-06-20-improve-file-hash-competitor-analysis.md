# file-hash — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/file-hash` — compute MD5, SHA-1, SHA-256, SHA-512 and CRC-32
checksums of any file. Chat + CLI (no page: a file→digests report fits neither
the pure-text nor the ffmpeg media page shape — the F3 no-page file-input
pattern, like detect-file-type).

## What competitors do

- **Online hash/checksum sites** (emn178 online tools, defuse.ca, md5file.com,
  online-convert, gchq CyberChef) — upload/paste a file, get one or several
  digests. Strengths: many algorithms. Weaknesses: many **upload the file** to a
  server (privacy), several only do one algorithm at a time, and few report
  CRC-32 alongside the crypto hashes.
- **System CLIs** (`md5sum`, `sha256sum`, `sha512sum`, `crc32`, PowerShell
  `Get-FileHash`) — the reference, but require a shell and one command per
  algorithm.
- **VirusTotal / threat intel** — take a SHA-256 (or MD5/SHA-1) to look up a
  file's reputation; this tool produces exactly those digests for that workflow.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (RustCrypto) compiled to wasm,
   so it runs in the chat Service Worker and headless via the CLI. The file
   never leaves the device.
2. **All digests at once.** One call returns MD5, SHA-1, SHA-256, SHA-512 **and**
   CRC-32 as lowercase hex — no re-running per algorithm. SHA-512 and CRC-32 go
   beyond the requested MD5/SHA-1/SHA-256 set (integrity + zip/gzip checks).
3. **Threat-intel ready.** The SHA-256/MD5/SHA-1 are precisely what VirusTotal
   and most malware databases key on, so the output drops straight into a lookup.
4. **Chainable.** Takes a `url` or a `ref` from a prior tool, so you can hash the
   output of another tool (e.g. an extracted/encrypted file) without a round-trip.
5. **Correct & verified.** Digests match the system `sha256sum`/`md5sum`
   byte-for-byte (see tests).

## Honest scope

- Hashes the file as-is (no normalization). For text with differing line endings,
  CRLF vs LF will (correctly) hash differently.
- CRC-32 is the IEEE/zip variant (reflected, poly 0xEDB88320); other CRC variants
  aren't offered.

## Tests

4 core unit tests against **published known-answer vectors**: empty string and
`"abc"` for MD5/SHA-1/SHA-256/SHA-512 (FIPS/RFC test vectors) plus CRC-32 of
`"abc"` = `352441c2`; hex is lowercase/zero-padded; digest lengths are
32/40/64/128/8. Plus the block drift-guard schema test. CLI verified over the
wire on `tux.png` — the tool's MD5, SHA-256 and byte size match the system
`md5sum`/`sha256sum`/`stat` exactly.
