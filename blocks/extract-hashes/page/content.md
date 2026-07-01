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

## FAQ

<details>
<summary>Why wasn't my hash picked up?</summary>

Only hex runs of exactly 32, 40, 64, or 128 characters are extracted — the
standard MD5 / SHA-1 / SHA-256 / SHA-512 digest lengths. CRC32 values (8 hex
chars), truncated digests, and arbitrary hex blobs are deliberately skipped so
random hex in your text doesn't produce false positives.

</details>

<details>
<summary>Is a 64-character match definitely SHA-256?</summary>

Not necessarily. Grouping is by **length**, and several algorithms share a
length — SHA3-256, BLAKE2s and Keccak-256 also produce 64 hex characters. The
group label is the most common algorithm for that length, and the length is
shown alongside so you can verify against your source.

</details>

<details>
<summary>How are duplicates and letter case handled?</summary>

Each hash appears once per group: de-duplication is case-insensitive, so
`ABC123…` and `abc123…` count as the same digest, and results keep first-seen
order. Output is normalized to lowercase by default — untick "Normalize to
lowercase" to preserve the casing from your text.

</details>
