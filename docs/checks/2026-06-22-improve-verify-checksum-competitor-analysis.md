# verify-checksum — competitor analysis (2026-06-22)

Tool: **Checksum Verifier** — paste data + an expected checksum, get MATCH /
MISMATCH. Algorithm auto-detected from the expected digest's byte length, or
chosen explicitly. Pure Rust (RustCrypto + blake3) → runs on all surfaces (chat,
CLI, standalone page).

## Surfaces verified

- **chat block** — `wafer build` validates `target/block.wasm` instantiates
  (369 KiB). Schema drift guard passes.
- **CLI** — `gizza tool verify-checksum text=abc expected=<sha256>` → MATCH
  (auto-detect sha256); `algorithm=sha256` with wrong input → MISMATCH; md5
  digest auto-detected. All correct.
- **page** — 5 Playwright tests pass (MATCH auto, MISMATCH, md5 auto-detect, hex
  input encoding, query-param deep-link).

## Top competitors surveyed

1. **OnlineHashCrack / "checksum verifier" web tools** — upload/paste a file,
   paste an expected hash, get match/no-match. Usually fixed to MD5+SHA family.
2. **emn178 hash pages / md5file.com** — compute then eyeball-compare manually
   (no built-in comparison step).
3. **Get-FileHash (PowerShell) / `sha256sum -c`** — CLI verification against a
   `.sha256`/`.md5` sidecar; algorithm is fixed per command.
4. **VirusTotal / file-info "checksum" panels** — show MD5/SHA1/SHA256 of an
   uploaded file; comparison is manual.
5. **Browser-based "verify file integrity" widgets** — drag a file, paste a
   hash, match/no-match; some auto-pick by length.

## Gap diff and ranking (fit-to-model)

| Capability | Competitors | This tool | Action |
|---|---|---|---|
| Explicit match/mismatch verdict | partial (many only compute) | yes | covered |
| Auto-detect algorithm by digest length | rare | yes (tries whole width-family) | **differentiator, covered** |
| Algorithm breadth (MD5…BLAKE3) | MD5/SHA only is common | 11 algorithms incl. SHA-3, BLAKE2/3 | **covered, ahead** |
| Accept expected as hex **or** base64 | hex-only is common | both, with `0x`/case/whitespace tolerance | **covered, ahead** |
| Constant-time-ish comparison | n/a | yes (no early-out) | covered |
| Show expected vs actual side by side | sometimes | yes (report lists both) | covered |
| Decode input from hex/base64 before hashing | rare | yes (`input_encoding`) | covered |
| Local / no-upload privacy | mixed | yes (in-browser wasm) | covered |
| Deep-link via query params | rare | yes (page query-prefill) | covered |

### Out-of-model (NOT built — recorded, not implemented)

- **File upload as the verified input.** The page input is a single text field;
  binary file-input → JSON tools exist (`file-hash`) but a *file*-input verify
  surface needs `AssetKind` page wiring not part of this pure tool. For files,
  users hash with `file-hash` then paste the digest here, or use the CLI. Noted
  as a deliberate scope boundary, consistent with `hash-text` (text-input).
- **Sidecar `.sha256` / `CHECKSUMS` file parsing** (extract `hash  filename`
  lines): a separate parsing tool, out of scope for a single-value verifier.

## Copy / UX / visual

- Title/description/tags written for SEO ("checksum verifier", "verify hash",
  per-algorithm keywords); no competitor copy/branding copied.
- Content explains auto-detection length table, hex/base64 acceptance, and the
  MD5/SHA-1 "broken — checksum only" caveat. Cross-links to hash-identifier and
  hash-text.
- Multiline fields for data + expected so pasted multi-line/whitespace content
  is preserved.

## Not a duplicate

`hash-text` and `file-hash` *compute* a digest; `hash-identifier` *classifies* a
hash's format. None *verify* input against an expected value (compute + compare
+ verdict). Confirmed distinct before building.
