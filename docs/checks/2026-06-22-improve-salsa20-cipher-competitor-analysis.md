# salsa20-cipher — competitor analysis & verification (2026-06-22)

## What it is

`salsa20-cipher` encrypts/decrypts text with the Salsa20/20 stream cipher (DJB's
eSTREAM cipher). Inputs: `data`, `operation` (encrypt|decrypt), `key` (16 or 32
bytes), `nonce` (8 bytes), `key_format` (text|encoded), `counter` (initial 64-bit
block counter), `format` (hex|base64). Pure Rust core implemented from the spec
(quarter-round / double-round, sigma/tau constants), no crypto crate dependency —
so it instantiates and runs on all three surfaces (chat block, CLI, page).

## Surfaces verified

- **Chat block:** `wafer build` validates `target/block.wasm` (319 KiB) — instantiates clean.
- **CLI:** `gizza tool salsa20-cipher` — encrypt("attack at dawn", key/nonce) →
  `0d065913366f2bbf510a3551aac8`; decrypt of that recovers the plaintext;
  encoded-key round-trip of 64 bytes recovers the input; wrong-length key errors
  (exit 1, "must be 16 or 32 bytes").
- **Page:** Playwright `tool-page-salsa20-cipher.spec.ts` — 3/3 pass (round-trip,
  deterministic ciphertext, wrong-length-key error message).
- **Correctness:** core unit tests assert the official ECRYPT eSTREAM Set 1 vector 0
  keystreams for both the **256-bit** key (`E3BE8FDD8BECA2E3…`) and the **128-bit**
  key (`4DFA5E481DA23EA0…`), plus multi-block counter continuity. 9 unit tests pass.

## Competitor scan (top tools for "salsa20 encrypt online")

1. **CyberChef (gchq.github.io/CyberChef)** — has a "Salsa20" recipe operation.
   Inputs: key (hex/utf8/base64/latin1), nonce (hex/...), counter, rounds (8/12/20),
   input/output type. Outputs raw/hex. The reference implementation.
2. **cryptii.com** — modular cipher playground; has ChaCha/stream-cipher style
   blocks but Salsa20 coverage is thin; key/nonce as text or bytes.
3. **devglan / various "online Salsa20" form tools** — key + nonce + plaintext,
   hex/base64 output; mostly thin wrappers, no counter, no 128-bit option exposed.
4. **Language libs (libsodium `crypto_stream_salsa20`, Rust `salsa20` crate,
   Python `pycryptodome` Salsa20)** — the API benchmarks: 16/32-byte key, 8-byte
   nonce, optional seek/counter.
5. **emn178-style hash/cipher utility sites** — generally cover RC4/AES, Salsa20
   rarely; when present, key+nonce+hex only.

## Gap diff & decisions (fit-to-model)

| Capability | Competitors | Ours | Decision |
|---|---|---|---|
| 256-bit key | all | yes (32-byte) | covered |
| 128-bit key | CyberChef, libsodium | yes (16-byte, tau constants) | covered — better than most form tools |
| 8-byte nonce | all | yes (required, validated) | covered |
| hex / base64 I/O | most | yes (both, for data + key + nonce) | covered |
| text (UTF-8) key/nonce | CyberChef | yes (key_format=text) | covered |
| initial block counter / seek | CyberChef, libsodium | yes (`counter` param) | covered |
| encrypt + decrypt (symmetric) | all | yes | covered |
| selectable rounds (8/12/20) | CyberChef only | **no — fixed at 20** | OUT OF SCOPE: Salsa20/8 and /12 are reduced-round variants used almost exclusively for cryptanalysis benchmarking; shipping them invites misuse. Standard "Salsa20" = 20 rounds, which is what the tool name implies. Documented, not built. |
| XSalsa20 (24-byte nonce) | libsodium | **no** | OUT OF SCOPE: XSalsa20 is a distinct construction (HSalsa20 nonce extension); would be its own tool. Noted. |
| authenticated (AEAD / Poly1305) | libsodium secretbox | **no** | OUT OF SCOPE by design: this is the raw cipher. Page + skill copy steer users to `aes-cipher`/`text-encrypt` for authenticated, password-based encryption. |
| password-based key derivation | aes-cipher (ours) | no (raw key) | intentional — keeps the tool a faithful raw-cipher primitive; cross-references the password tools. |

## Copy / UX gaps closed

- Page and skill descriptions spell out the exact key (16/32-byte) and nonce
  (8-byte) length requirements and the hex/base64-char equivalents, matching the
  precision of CyberChef's field hints.
- Security note added (no authentication, never reuse a key+nonce pair, raw cipher)
  and cross-links to the authenticated `aes-cipher`/`text-encrypt` tools — a safety
  affordance most thin competitor form-tools lack.
- All copy original; no competitor branding/trademark/text copied.

## Conclusion

Feature-complete versus mainstream online Salsa20 tools for the standard
(20-round) cipher, and ahead of the typical form tools on 128-bit support, base64
I/O, an explicit block counter, and safety guidance. Reduced-round / XSalsa20 /
AEAD are deliberately out of scope and documented above.
