# nacl-secretbox-encrypt — competitor analysis (2026-07-25)

**Tool:** Encrypt or decrypt data with a shared symmetric key using **NaCl secretbox**
(`crypto_secretbox`, XSalsa20-Poly1305). 32-byte key, 24-byte nonce, 16-byte Poly1305 tag,
combined output = `nonce(24) || ciphertext || tag(16)` for page/CLI convenience. Pure Rust via
RustCrypto `crypto_secretbox`, so it runs on every backend (CLI, page/WASM, chat).

## Why this is NOT a duplicate

Checked existing crypto blocks before building:

- **`chacha20-cipher`** — ChaCha20-Poly1305 **IETF/RFC 8439** AEAD: 12-byte nonce, 32-bit counter,
  MAC input includes AAD + length framing. A *different construction and wire format* from NaCl
  secretbox (XSalsa20 + 24-byte nonce, Poly1305 over the bare ciphertext, no AAD). Ciphertexts are
  not interchangeable.
- **`salsa20-cipher`** — raw Salsa20/20 stream cipher, 8-byte nonce, **unauthenticated** (no MAC).
  secretbox adds HSalsa20 nonce-extension (→ XSalsa20, 24-byte nonce) *and* Poly1305
  authentication.
- **`xsalsa20-cipher`** (raw stream, if present) — XSalsa20 keystream XOR only, **no Poly1305 tag**.
  secretbox is the authenticated construction on top.
- **`text-encrypt` / `encrypt-file` / `aes-cipher`** — passphrase + KDF (PBKDF2/Argon2) + AES-GCM,
  proprietary self-describing token. secretbox is a *raw-key* interop primitive with a fixed,
  externally-compatible NaCl/libsodium/PyNaCl/TweetNaCl wire format — a distinct interop target.

Conclusion: distinct construction, distinct wire format, distinct interop target → **viable, not a
dup**. Build-time verification covers deterministic round-trips, separate/combined nonce paths,
length validation, wrong-key rejection, and tamper rejection.

## Competitors scanned

1. **8gwifi.org — Libsodium SecretBox Online** (`/naclaead.jsp`) — message, 32-byte secret key,
   hex nonce, encrypt/decrypt toggle, show-key, **Generate random nonce** button, copy/download/
   share-URL. Also exposes an AAD field (that page conflates secretbox with the AEAD variant).
2. **libsodium documentation** (`crypto_secretbox_easy`) — the reference spec: key 32 B, nonce
   24 B, MAC 16 B, combined output prepends the tag, nonce-uniqueness warning.
3. **PyNaCl `nacl.secret.SecretBox`** — 32-byte key, 24-byte nonce, `encrypt()` returns
   `nonce || ciphertext` where ciphertext = `tag || ct`; canonical Python reference implementation.
4. **TweetNaCl / NaCl reference** — the original `crypto_secretbox`; source of the standard test
   vector (firstkey/nonce/message → tag `f3ffc7703f9400e5…`).

## Table-stakes → decision

| Capability | In competitors | Our decision |
|---|---|---|
| Message / data field | all | **in-model** — `data` |
| Encrypt / decrypt toggle | all | **in-model** — `operation` enum |
| 32-byte secret key | all | **in-model** — `key` (text or hex/base64) |
| 24-byte nonce (hex) | all | **in-model** — `nonce` (text or hex/base64) |
| Combined output | libsodium/PyNaCl | **in-model** — `nonce‖ciphertext‖tag`, with separate nonce accepted for decrypt |
| hex / base64 output encoding | typical | **in-model** — `output_encoding` enum |
| text vs encoded key/nonce/data | (implied) | **in-model** — `key_encoding`, `nonce_encoding`, `data_encoding` enums |
| Worked example / preset prefill | 8gwifi presets | **in-model** — `[[example]]` chip prefills a runnable case |
| Copy / download / share-URL | 8gwifi | **in-model (generic)** — page gives a Download link (text format) + `?param=` deep links |
| **Generate random nonce** button | 8gwifi | **out-of-model** — non-deterministic; the page's recompute-on-input model and the CLI's exact-output contract both require deterministic output. Users paste a fresh nonce; docs stress uniqueness. |
| **AAD** field | 8gwifi's `naclaead` page | **out-of-scope** — true NaCl `crypto_secretbox` has no AAD; AAD belongs to the AEAD variant, already covered by `chacha20-cipher` (ChaCha20-Poly1305 with `aad`). Documented in FAQ. |
| Detached MAC output | (libsodium `_detached`) | **out-of-model** — we ship one combined layout only; noted in limits. |
| Show/hide key | 8gwifi | **out-of-model** — a chat/CLI/one-page tool has no persistent secret store; N/A. |

Every table-stake lands in the descriptor or in the out-of-model list above — none dropped
silently. No competitor copy, branding, or trademarks were reproduced.
