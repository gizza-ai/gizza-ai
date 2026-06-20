# encrypt-file — competitor analysis (2026-06-20)

Eighth `/create-next-tool` backlog pick (csv-to-json was skiplisted before it as a
dup of csv-json-convert). Pure-Rust crypto tool (aes-gcm + PBKDF2) — like the
other pure tools it runs on ALL backends incl. the chat Service Worker. Surfaces:
**chat + CLI** (file in → file out; no page mode for pure-Rust file bytes).
Defensive/legitimate use (passphrase file encryption). Research via `WebSearch`.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| 8gwifi | AES-256 + PBKDF2, 100% in-browser (WebCrypto), .enc output, drag-drop | capabilities |
| encrypt-online.com | any file type, local-only, encrypt + decrypt | capabilities |
| cryptfile.online | AES-256-GCM or ChaCha20-Poly1305; random key / password / own key | capabilities |
| StatiCrypt | AES-256 self-decrypting HTML (niche) | capabilities |

## Gap diff vs our tool
Our tool: AES-256-GCM with a PBKDF2-HMAC-SHA256-derived key (200k iters), a fresh
random salt + nonce per encryption, a self-describing blob
(`GZAE1 | salt | nonce | ciphertext+tag`), encrypt + decrypt modes, any file type.
This matches the in-browser AES-256-GCM + PBKDF2 baseline competitors advertise,
and the GCM tag gives authenticated decryption (wrong passphrase / tampering fail
cleanly).

**In-model gaps considered, deferred (fit the model; minor):**
- **ChaCha20-Poly1305 alternative cipher** — a `cipher` enum (the chacha20poly1305
  crate is pure-Rust); AES-256-GCM is the recommended default, so this is a small
  optional add.
- **Random-key / keyfile mode** (no passphrase) — a different input mode; niche.
- Argon2 KDF instead of PBKDF2 — stronger but heavier in wasm; PBKDF2 200k is a
  reasonable default.

**Out-of-model:** drag-drop multi-file UI, self-decrypting HTML output (a
different artifact), browser WebCrypto (we use pure-Rust so it also works in the
chat SW + CLI).

## Tested
unit (6: roundtrip, wrong-passphrase fails, tampered-blob fails, fresh salt/nonce
per encrypt, bad-header rejected, empty-passphrase rejected) + drift-guard ·
`wafer build` validates the block (aes-gcm + getrandom compile to wasm32-wasip1;
pure-Rust so it also works in the chat SW) · CLI encrypts a real public file into
a GZAE1 blob + decrypt-mode rejects a non-blob cleanly. No page surface.

> Original work only — no competitor copy, branding, or trademarks copied.
