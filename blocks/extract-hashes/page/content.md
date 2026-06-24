## About this tool

**Extract Hashes** scans a block of text and pulls out every hexadecimal
**hash / digest** string it contains, grouping each by the algorithm implied by
its length:

- **MD5** — 32 hex characters
- **SHA-1** — 40 hex characters
- **SHA-256** — 64 hex characters
- **SHA-512** — 128 hex characters

Hashes are **deduplicated** case-insensitively and listed in first-seen order
within each group. By default they are normalized to **lowercase**; untick
**Normalize to lowercase** to keep the original casing.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Handy for

- Pulling file-hash indicators of compromise (IOCs) out of a malware or
  threat-intel report.
- Mining MD5/SHA checksums from build logs or release notes.
- Collecting and deduplicating digests scattered across a manifest or chat log.

### Notes

- Only the four standard digest lengths above are recognized; other hex runs
  (CRC32, truncated digests, hex blobs) are ignored to avoid false positives.
- The length is reported alongside each group so you can confirm the algorithm.
