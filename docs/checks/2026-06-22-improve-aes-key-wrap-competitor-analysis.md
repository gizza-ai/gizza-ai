# aes-key-wrap — competitor analysis (2026-06-22)

New tool built this run; this snapshot records the competitor landscape and the
gap analysis used to scope the initial feature set. (Paraphrased only — no
competitor copy/branding reproduced.)

## Function

Wrap or unwrap cryptographic key material with the AES Key Wrap algorithm under a
key-encryption key (KEK). Supports KW (RFC 3394 / NIST SP 800-38F) and KWP
(RFC 5649, "with padding"). Pure-Rust (RustCrypto `aes-kw`), runs fully client-side.

## Competitor landscape

The space is dominated by **libraries**, not interactive web tools — there is no
single well-known "wrap a key online" page. The closest comparables:

1. **asecuritysite.com — Hazmat key wrapping (RFC 3394 & 5649).** The only notable
   interactive web demo. Lets you enter a KEK and key material and shows the wrapped
   result for both RFC 3394 and 5649. Educational framing; server-side Python.
   Output is hex. No unwrap-only mode separated from wrap.
2. **Python `aes_keywrap` (kurtbrose).** RFC 3394 + RFC 5649 (alternative IV). Library
   API only — `aes_wrap_key` / `aes_unwrap_key`, raw bytes in/out.
3. **PHP `spomky-labs/aes-key-wrap`.** RFC 3394 + RFC 5649; used inside JOSE/JWE
   implementations. Library only.
4. **Python `cryptography` (pyca) hazmat keywrap.** `aes_key_wrap` /
   `aes_key_wrap_with_padding` + unwrap. Raises on integrity failure. Library only.
5. **C++ `ikluft/AESKeyWrap` (Crypto++).** RFC 3394 + RFC 5649. Library only.

(Fewer than 5 *interactive* competitors exist — most are code libraries. Reported
honestly rather than padded.)

## Gap analysis vs. our tool

| Capability / dimension                | Competitors                          | gizza aes-key-wrap | Status |
|---------------------------------------|--------------------------------------|--------------------|--------|
| RFC 3394 (KW) wrap + unwrap           | all                                  | yes                | parity |
| RFC 5649 (KWP) wrap + unwrap          | most (some KW-only)                  | yes                | parity |
| AES-128 / 192 / 256 (auto from KEK)   | varies                               | yes (16/24/32 B)   | parity / ahead |
| hex **and** base64 I/O                | mostly hex-only or bytes-only        | yes (both)         | ahead |
| Integrity-check failure surfaced      | libraries raise; web demo unclear    | yes (clear error)  | parity |
| Runs client-side, nothing uploaded    | web demo is server-side              | yes (wasm, local)  | ahead |
| No account / no API key               | n/a                                  | yes                | ahead |
| Deep-link via query params            | none                                 | yes                | ahead |

All in-model gaps were closed in the initial build: both algorithms, all three key
sizes, both encodings, explicit wrap/unwrap operation, clear integrity-failure error,
and a query-param-prefillable page.

## Out-of-model (considered, not built)

- **Random KEK / key generation** — out of scope for a deterministic wrap tool; a
  separate generator tool fits the model better (we already ship key generators).
- **JWE/JOSE envelope packaging** (wrap a CEK and emit a full JWE) — a distinct,
  larger tool; the primitive here is the building block.
- **Arbitrary alternative IVs / non-standard AIV** — deliberately omitted; the
  standard RFC 3394/5649 IVs are the safe, interoperable defaults.

## Verification (this run)

- Unit: 11 core tests incl. the **RFC 3394 §4.1 test vector** (exact match), KW/KWP
  round-trips, base64 round-trip, and error paths (bad KEK length, non-8-multiple KW,
  too-short KW, wrong-KEK integrity failure). Plus the drift-guard schema test.
- `wafer build`: chat block instantiates in wasm32-wasip1 (356 KiB).
- CLI: `gizza tool aes-key-wrap` wrap matches the RFC vector, unwrap round-trips,
  KWP wraps a 7-byte key, wrong KEK errors.
- Page: Playwright 3/3 — wrap→unwrap round-trip, bad-KEK error, query-param deep-link.
