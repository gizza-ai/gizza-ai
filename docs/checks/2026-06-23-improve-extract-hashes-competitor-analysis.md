# extract-hashes — competitor analysis (2026-06-23)

## What our tool does
Scans arbitrary text and pulls out every hexadecimal hash/digest string, grouping
each by the algorithm implied by its hex length:

- MD5 — 32 hex chars
- SHA-1 — 40 hex chars
- SHA-256 — 64 hex chars
- SHA-512 — 128 hex chars

De-duplicates case-insensitively, keeps first-seen order within each group, and can
normalize to lowercase (default) or preserve casing. Runs fully client-side (wasm) on
chat, CLI, and the standalone page.

## Top competitors

1. **APIVoid — Extract MD5 Hashes** (apivoid.com/tools/extract-md5-hashes/) and the
   sibling **Extract SHA1 Hashes** (apivoid.com/tools/extract-sha1-hashes/). Targeted at
   security analysts: paste logs / CSV / MySQL dumps / reports, find and copy hash strings.
   **Split into one tool per algorithm** — MD5 and SHA-1 are separate pages; no single
   tool extracts all hash types at once, and no SHA-256/512 extractor.
2. **hashes.com / CrackStation** — hash *lookup/cracking* services (hash → plaintext), not
   extractors. Different job; out of scope.
3. **Browserling / TestingBot / emn178 / Inventive HQ / Text Tool Suite** — hash
   *generators* (text → hash), not extractors. Adjacent but a different task.
4. **md5file.com WASM calculator / Apify hash-generator API** — file-hash *calculators*,
   not text extractors.

## Capability diff (vs. APIVoid extractors, the only true peers)

| Capability | APIVoid | extract-hashes | Notes |
|---|---|---|---|
| Extract MD5 | yes (own page) | yes | |
| Extract SHA-1 | yes (own page) | yes | |
| Extract SHA-256 | no | **yes** | gap closed in our favor |
| Extract SHA-512 | no | **yes** | gap closed in our favor |
| All algorithms in one pass, grouped | no (per-algo pages) | **yes** | our key differentiator |
| Deduplicate | unclear / per-tool | yes (case-insensitive) | |
| Case normalization toggle | not offered | yes (`lowercase`) | |
| Client-side / private | yes | yes | parity |
| Count + per-group length reported | minimal | yes | |
| API + CLI access | API product | CLI + chat + page | |

## Gaps considered and decisions

- **Other digest lengths (SHA-224=56, SHA-384=96, NTLM=32, etc.).** SHA-224/384 share no
  ambiguity and could be added, but NTLM collides with MD5 at 32 chars and would force
  guessing. Kept the four standard, unambiguous lengths to avoid false positives; this is
  a deliberate scope choice, documented in the page copy. In-model but intentionally
  deferred to keep precision high. Could revisit SHA-224/384 as a follow-up.
- **CSV / file upload of dumps.** Out of model for a pure text-in tool here; the textarea
  already accepts pasted dumps, which covers the common case.
- **Hash cracking / lookup.** Different product category (online DB lookup); explicitly out
  of scope and not something a local pure tool should do.

## Conclusion
No in-model capability/copy gap remains versus the true peers — we already exceed the
APIVoid extractors by covering all four common algorithms in a single grouped pass plus a
casing toggle and dedupe. Copy emphasizes the multi-algorithm grouping and privacy
(client-side wasm). No competitor copy/branding was reused.

## Verification (all surfaces, 2026-06-23)
- `cargo test --workspace` in blocks/extract-hashes — 7 tests pass (6 core + 1 drift-guard).
- `wafer build` — block.wasm validates OK (gizza-ai/extract-hashes v0.1.0).
- `wasm-pack build …/web` — page wasm built.
- `gizza tool extract-hashes text=…` — groups md5/sha1/sha256, dedupes a repeated MD5.
- Playwright `tool-page-extract-hashes.spec.ts` — 2 tests pass (grouping + casing toggle).
