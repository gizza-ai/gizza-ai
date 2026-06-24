# scrypt-derive — competitor analysis (2026-06-22)

Snapshot of the top scrypt / KDF tools to rank capability gaps. No competitor copy,
branding, or trademarks were reused. Out-of-model features are listed but not built.

## Surfaces verified (Phase 1)
- **Chat block:** `wafer build` validates+instantiates `target/block.wasm` (scrypt 0.11
  is wasm-safe in wafer; 330.8 KiB).
- **CLI:** `gizza tool scrypt-derive password= salt= n=16 r=1 p=1 length=64 encoding=hex`
  reproduces RFC 7914 §12 vector 1; `mode=verify` returns `{"match":true}`; bad N gives a
  clear error.
- **Page:** `/tools/scrypt-derive/` — 3 Playwright tests pass (derive vector 1, verify,
  query-param deep link).

## Competitors surveyed
1. **OpenSSL `kdf`/`enc -scrypt` (CLI):** exposes N, r, p and key length; reference
   implementation. No salt-encoding helpers, no in-browser UI.
2. **Node `crypto.scryptSync` / `crypto.scrypt`:** params `N`, `r`, `p`, `maxmem`,
   `keylen`; returns a Buffer (hex/base64 at the call site). No verify helper.
3. **Python `hashlib.scrypt`:** `n`, `r`, `p`, `dklen`, `maxmem`; raw bytes out.
4. **Go `golang.org/x/crypto/scrypt`:** `Key(password, salt, N, r, p, keyLen)`; raw bytes.
5. **Browser "scrypt online" calculators (various):** password + salt + N/r/p + dkLen,
   hex output; most run JS scrypt client-side. Some also emit the PHC `$scrypt$` string.

## Capability diff + ranking (fit-to-model)

| Capability | Competitors | scrypt-derive | Status |
|---|---|---|---|
| N / r / p cost params | all | yes (N power-of-two, r, p) | matched |
| Output key length | all | yes (1–1024 bytes) | **ahead** |
| Salt as text **and** raw bytes (hex/base64) | rare (most assume text) | yes (utf8/hex/base64) | **ahead** |
| Output encoding hex / base64 | mixed | yes | matched |
| Verify a password reproduces a key | none | yes (mode=verify) | **ahead** |
| Runs locally, nothing uploaded | OpenSSL/Node/Python/Go (local); web calcs vary | yes (in-browser wasm) | matched |
| RFC 7914 test-vector interoperability | OpenSSL/Node/Python/Go | yes (vectors 1 & 3 as unit tests) | matched |
| Clear param-validation errors (power-of-two N, log2(N)<r*16, r*p<2^30, mem cap) | partial | yes | **ahead** |

## Gaps closed this build
- Salt accepts utf8 / hex / base64 (raw-byte salts), matching what library users need to
  reproduce keys, which most web calculators lack.
- Verify mode (no surveyed competitor offers it) — compares a re-derived key against an
  expected hex/base64 value with a constant-time-style byte check.
- Helpful, specific validation errors for every scrypt parameter constraint.
- Memory guard: caps `128*N*r` at 1 GiB so a too-large N can't OOM the browser tab.

## Out-of-model / intentionally not built
- **PHC `$scrypt$...$` modular-crypt string output/parsing.** A distinct format
  (and the existing crypto blocks focus on raw key bytes); would warrant its own tool.
  Listed, not built.
- **`maxmem` knob (Node/Python).** Superseded here by the fixed 1 GiB safety cap; an
  adjustable memory ceiling adds little for an in-browser tool. Not built.
## Notes
- **Output key length:** the `scrypt` 0.11 `Params::len` field is only validated to be
  10..=64, but `scrypt()` actually derives `output.len()` bytes and ignores `Params::len`.
  The tool passes a fixed valid `Params::len` and lets the output buffer size (1–1024 bytes)
  govern the real length — verified producing a 200-byte key. This matches the spec's PBKDF2
  finalization step (output can exceed 64 bytes).
- Defaults (N=16384, r=8, p=1, length=32, hex) match the RFC 7914 interactive-login example.
- Deterministic and standards-interoperable: output matches OpenSSL `-scrypt`, Node
  `crypto.scrypt`, Python `hashlib.scrypt`, and Go `x/crypto/scrypt` for the same inputs.
