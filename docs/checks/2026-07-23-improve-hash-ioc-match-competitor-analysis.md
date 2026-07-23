# hash-ioc-match — competitor analysis (2026-07-23)

**Tool function:** hash a file (its bytes) and flag it if any of its computed
digests appears in a pasted blocklist of known-bad MD5 / SHA-1 / SHA-256 /
SHA-512 values. Offline IOC matching — no live threat-feed lookup.

All notes below are paraphrased from public tool descriptions. No competitor
copy, branding, or trademarks are reproduced.

## Competitors scanned (top real tools)

1. **InventiveHQ — File Hash Checker / Malicious Hash Lookup.** Drag-drop a file
   to hash it locally in the browser, or bulk-paste MD5/SHA-1/SHA-256 hashes and
   check them against malware databases. Emphasises "hashing happens in your
   browser."
2. **Ciphers Security — Hash Reputation Checker.** Paste an MD5/SHA-1/SHA-256
   file hash; checks it against MalwareBazaar and VirusTotal feeds; hashing done
   in-browser, file never uploaded.
3. **Team Cymru — Malware Hash Registry (hash.cymru.com).** Manual interface to
   check hashes against a malware dataset; supports MD5/SHA-1/SHA-256; caps a
   submission at ~1000 hashes.
4. **isMalicious — File Hash Reputation.** MD5/SHA1/SHA256 reputation lookups
   enriched with malware family, related domains/IPs, threat-intel evidence.
5. **CISA ioc-scanner (reference impl).** Scans a filesystem for indicators of
   compromise; accepts a supplied list of bad hashes and reports matches — the
   offline "match my file against a known-bad list" model this tool mirrors.

## Table-stakes → decision (every item lands in the descriptor or is listed here)

| Capability | In-model? | Where |
| --- | --- | --- |
| Compute MD5, SHA-1, SHA-256 of the input | yes | `input` + computed digests |
| Compute SHA-512 too | yes (dep already used by file-hash) | computed digests |
| Match against a user-supplied blocklist | yes | `blocklist` param |
| Tolerate labelled / CSV / separator-laden blocklist lines (`MD5: …`, `hash,name`, `#`-comments) | yes | core normalizer extracts hex tokens of digest widths |
| Case-insensitive matching | yes | digests + blocklist lowercased |
| Hash raw file bytes, not just text | yes | `input_encoding = hex \| base64` decodes bytes first |
| Report which algorithm(s)/hash(es) matched | yes | structured `matches` + text report |
| Clean "not flagged" result | yes | `flagged=false`, CLEAN status |
| Runs locally, nothing uploaded | yes (inherent — pure wasm, in-browser) | — |

## Out-of-model (listed, not built)

- **Live threat-feed lookup** (VirusTotal / MalwareBazaar / Malware Hash
  Registry / reputation APIs): needs network egress + API keys + a hosted
  proxy. This tool is deliberately offline — the user supplies the blocklist,
  so the file and its hashes never leave the browser. This is a different,
  privacy-first product shape, not a missing feature.
- **Threat-intel enrichment** (malware family, related domains/IPs): needs a
  curated intelligence feed — out of model.
- **Drag-drop binary file upload on the standalone page:** the generic page
  runtime only wires file upload for the ffmpeg family; a pure-Rust file-upload
  page has no runtime here. The page therefore accepts the file *content* as
  text or, for true binaries, as pasted hex/base64 (`input_encoding`) — the same
  approach the sibling `hash-all` / `verify-checksum` pages use. The chat/CLI
  surfaces likewise take file bytes via hex/base64. Distinct from `file-hash`
  (chat/CLI file url/ref → digests, no blocklist, no page) and `verify-checksum`
  (matches ONE expected checksum, not a many-entry IOC blocklist).

## Distinctness vs existing blocks

- `file-hash`: computes digests of a file (url/ref) — no blocklist, no flagging,
  no page.
- `hash-all`: computes every digest of text — no matching.
- `verify-checksum`: MATCH/MISMATCH against **one** expected checksum (integrity
  check). hash-ioc-match matches against a **many-entry** pasted blocklist and
  reports a security FLAGGED/CLEAN verdict with which entry hit — an IOC-triage
  workflow, not integrity verification.
- `extract-hashes` / `ioc-extract`: pull hashes *out of* text; they do not hash
  an input or match it against a blocklist.

Conclusion: distinct, in-model, buildable. Proceed.
